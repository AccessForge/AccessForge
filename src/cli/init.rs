use anyhow::{Context, Result, bail};
use std::io::{self, Write};
use std::path::Path;

use crate::manifest::{Manifest, LoaderKind, Source, slugify};
use crate::steam;

/// Handle `init` command.
pub fn dev_init(dir: &Path) -> Result<()> {
    let manifest_path = dir.join("accessforge.yml");
    if manifest_path.exists() {
        bail!("accessforge.yml already exists in {}", dir.display());
    }

    println!("Welcome to AccessForge mod setup.");
    println!("This will create an accessforge.yml manifest in {}.", dir.display());
    println!("Press Enter to accept the default shown in [brackets].\n");

    // --- Game info (ask first to derive good defaults) ---
    let store = prompt_validated("Store (steam/none)", Some("none"), |v| {
        if v == "steam" || v == "none" {
            Ok(())
        } else {
            Err("Must be 'steam' or 'none'.".to_string())
        }
    });

    let (game_name, steam_id) = match store.as_str() {
        "steam" => prompt_steam_game()?,
        _ => prompt_manual_game(),
    };

    let loader = prompt_validated(
        "Loader (ue4ss/bepinex/melonloader/none)",
        Some("none"),
        |v| {
            LoaderKind::from_str(v)
                .map(|_| ())
                .map_err(|_| "Must be one of: ue4ss, bepinex, melonloader, none".to_string())
        },
    );

    // --- Mod info (defaults derived from game name) ---
    let default_mod_name = format!("{game_name} Access");
    let mod_name = prompt_validated("Mod name", Some(&default_mod_name), |v| {
        if v.is_empty() {
            Err("Mod name cannot be empty.".to_string())
        } else {
            Ok(())
        }
    });

    let default_desc = format!("Screen reader accessibility mod for {game_name}");
    let description = prompt_validated("Description", Some(&default_desc), |v| {
        if v.is_empty() {
            Err("Description cannot be empty.".to_string())
        } else {
            Ok(())
        }
    });

    // --- Author and source from git ---
    let (git_owner, git_repo) = detect_git_remote(dir);
    let git_user = detect_git_user();

    if git_owner.is_some() {
        println!("Detected git remote.");
    }

    let default_author = git_user.as_deref().or(git_owner.as_deref());
    let author = prompt_validated("Author", default_author, |v| {
        if v.is_empty() {
            Err("Author cannot be empty.".to_string())
        } else {
            Ok(())
        }
    });

    let source = if let (Some(owner), Some(repo)) = (&git_owner, &git_repo) {
        let default_source = format!("github:{owner}/{repo}");
        prompt_validated("Source", Some(&default_source), validate_source)
    } else {
        prompt_validated(
            "Source (github:owner/repo or url:https://...)",
            None,
            validate_source,
        )
    };

    // --- Derived fields ---
    let id = slugify(&mod_name);
    let version = "0.1.0";
    let asset = format!("{id}-{{version}}.zip");

    // --- Build YAML ---
    let mut yaml = format!(
        r#"spec: 1

id: {id}
name: {mod_name}
description: {description}
author: {author}
version: "{version}"
source: {source}

game:
  name: {game_name}
"#,
        mod_name = yaml_quote(&mod_name),
        description = yaml_quote(&description),
        author = yaml_quote(&author),
        game_name = yaml_quote(&game_name),
    );

    if let Some(steam_id) = steam_id {
        yaml.push_str(&format!(
            r#"  store:
    steam: {steam_id}
"#
        ));
    }

    yaml.push_str(&format!(
        r#"
loader:
  name: {loader}

release:
  asset: "{asset}"
"#
    ));

    // --- Validate before writing ---
    if let Err(e) = Manifest::from_yaml(&yaml) {
        println!("\nWarning: generated manifest has validation errors: {e}");
        println!("Writing anyway — you can fix these manually.");
    }

    std::fs::write(&manifest_path, &yaml)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    println!("\nCreated: {}", manifest_path.display());
    println!("Next: run `accessforge install` to test the install flow.");

    Ok(())
}

/// Prompt for a Steam game: ask for app ID, look it up, confirm.
fn prompt_steam_game() -> Result<(String, Option<u32>)> {
    loop {
        let id_str = prompt_validated("Steam app ID", None, |v| {
            v.parse::<u32>()
                .map(|_| ())
                .map_err(|_| "Must be a number.".to_string())
        });
        let id: u32 = id_str.parse().context("invalid Steam app ID")?;

        match steam::find_game_info(id) {
            Ok(Some((name, path))) => {
                println!("Found: {} at {}", name, path.display());
                let confirm = prompt_validated(
                    &format!("Is this correct? (yes/no)"),
                    Some("yes"),
                    |v| {
                        if v == "yes" || v == "no" {
                            Ok(())
                        } else {
                            Err("Please enter 'yes' or 'no'.".to_string())
                        }
                    },
                );
                if confirm == "yes" {
                    let game_name = prompt_validated("Game name", Some(&name), |v| {
                        if v.is_empty() {
                            Err("Game name cannot be empty.".to_string())
                        } else {
                            Ok(())
                        }
                    });
                    return Ok((game_name, Some(id)));
                }
                println!("Let's try again.");
            }
            Ok(None) => {
                println!("Game with app ID {id} not found in Steam. Is it installed?");
                let retry = prompt_validated(
                    "Try another ID? (yes/no)",
                    Some("yes"),
                    |v| {
                        if v == "yes" || v == "no" {
                            Ok(())
                        } else {
                            Err("Please enter 'yes' or 'no'.".to_string())
                        }
                    },
                );
                if retry == "no" {
                    let game_name = prompt_validated("Game name", None, |v| {
                        if v.is_empty() {
                            Err("Game name cannot be empty.".to_string())
                        } else {
                            Ok(())
                        }
                    });
                    return Ok((game_name, Some(id)));
                }
            }
            Err(e) => {
                println!("Could not search Steam: {e}");
                let game_name = prompt_validated("Game name", None, |v| {
                    if v.is_empty() {
                        Err("Game name cannot be empty.".to_string())
                    } else {
                        Ok(())
                    }
                });
                return Ok((game_name, Some(id)));
            }
        }
    }
}

/// Prompt for a game without a store: ask for folder path, derive name.
fn prompt_manual_game() -> (String, Option<u32>) {
    let game_path = prompt_validated("Game folder path", None, |v| {
        if v.is_empty() {
            Err("Path cannot be empty.".to_string())
        } else if !Path::new(v).is_dir() {
            Err(format!("'{}' is not a valid directory.", v))
        } else {
            Ok(())
        }
    });

    let default_name = Path::new(&game_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string());

    let game_name = prompt_validated("Game name", default_name.as_deref(), |v| {
        if v.is_empty() {
            Err("Game name cannot be empty.".to_string())
        } else {
            Ok(())
        }
    });

    (game_name, None)
}

fn validate_source(v: &str) -> std::result::Result<(), String> {
    Source::parse(v)
        .map(|_| ())
        .map_err(|e| format!("{e}"))
}

/// Read a line from stdin with an optional default value.
fn prompt(label: &str, default: Option<&str>) -> String {
    if let Some(def) = default {
        print!("{label} [{def}]: ");
    } else {
        print!("{label}: ");
    }
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let input = input.trim().to_string();

    if input.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        input
    }
}

/// Prompt with validation — re-asks until the validator passes.
fn prompt_validated(
    label: &str,
    default: Option<&str>,
    validate: impl Fn(&str) -> std::result::Result<(), String>,
) -> String {
    loop {
        let value = prompt(label, default);
        match validate(&value) {
            Ok(()) => return value,
            Err(msg) => println!("{msg}"),
        }
    }
}

/// Quote a string for YAML if it contains special characters.
fn yaml_quote(s: &str) -> String {
    if s.contains(':') || s.contains('#') || s.contains('\'') || s.contains('"')
        || s.contains('\n') || s.starts_with(' ') || s.ends_with(' ')
        || s.starts_with('{') || s.starts_with('[')
    {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// Try to detect the git user name from global config.
fn detect_git_user() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if name.is_empty() { None } else { Some(name) }
        }
        _ => None,
    }
}

/// Try to detect GitHub owner and repo from git remote origin.
fn detect_git_remote(dir: &Path) -> (Option<String>, Option<String>) {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output();

    let url = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return (None, None),
    };

    parse_github_url(&url)
}

/// Parse a GitHub URL into (owner, repo).
fn parse_github_url(url: &str) -> (Option<String>, Option<String>) {
    // https://github.com/owner/repo.git or https://github.com/owner/repo
    if let Some(path) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let path = path.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = path.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return (Some(parts[0].to_string()), Some(parts[1].to_string()));
        }
    }

    // git@github.com:owner/repo.git
    if let Some(path) = url.strip_prefix("git@github.com:") {
        let path = path.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return (Some(parts[0].to_string()), Some(parts[1].to_string()));
        }
    }

    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_url_https() {
        let (owner, repo) = parse_github_url("https://github.com/jessica/sparking-zero-access");
        assert_eq!(owner.as_deref(), Some("jessica"));
        assert_eq!(repo.as_deref(), Some("sparking-zero-access"));
    }

    #[test]
    fn test_parse_github_url_https_git_suffix() {
        let (owner, repo) = parse_github_url("https://github.com/jessica/sparking-zero-access.git");
        assert_eq!(owner.as_deref(), Some("jessica"));
        assert_eq!(repo.as_deref(), Some("sparking-zero-access"));
    }

    #[test]
    fn test_parse_github_url_ssh() {
        let (owner, repo) = parse_github_url("git@github.com:jessica/sparking-zero-access.git");
        assert_eq!(owner.as_deref(), Some("jessica"));
        assert_eq!(repo.as_deref(), Some("sparking-zero-access"));
    }

    #[test]
    fn test_parse_github_url_ssh_no_suffix() {
        let (owner, repo) = parse_github_url("git@github.com:jessica/sparking-zero-access");
        assert_eq!(owner.as_deref(), Some("jessica"));
        assert_eq!(repo.as_deref(), Some("sparking-zero-access"));
    }

    #[test]
    fn test_parse_github_url_invalid() {
        let (owner, repo) = parse_github_url("https://gitlab.com/owner/repo");
        assert!(owner.is_none());
        assert!(repo.is_none());
    }

    #[test]
    fn test_parse_github_url_empty() {
        let (owner, repo) = parse_github_url("");
        assert!(owner.is_none());
        assert!(repo.is_none());
    }

    #[test]
    fn test_yaml_quote_plain() {
        assert_eq!(yaml_quote("simple name"), "simple name");
    }

    #[test]
    fn test_yaml_quote_colon() {
        assert_eq!(yaml_quote("DRAGON BALL: Sparking! ZERO"), "\"DRAGON BALL: Sparking! ZERO\"");
    }

    #[test]
    fn test_yaml_quote_hash() {
        assert_eq!(yaml_quote("mod #1"), "\"mod #1\"");
    }

    #[test]
    fn test_yaml_quote_inner_quotes() {
        assert_eq!(yaml_quote("it's a \"test\""), "\"it's a \\\"test\\\"\"");
    }

    #[test]
    fn test_yaml_quote_leading_bracket() {
        assert_eq!(yaml_quote("[array]"), "\"[array]\"");
        assert_eq!(yaml_quote("{object}"), "\"{object}\"");
    }

    #[test]
    fn test_yaml_quote_leading_trailing_space() {
        assert_eq!(yaml_quote(" leading"), "\" leading\"");
        assert_eq!(yaml_quote("trailing "), "\"trailing \"");
    }
}
