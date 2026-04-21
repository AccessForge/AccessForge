use anyhow::{Context, Result, bail};
use crate::manifest::{LoaderKind, normalize_version};
use crate::registry;
use std::path::Path;
use versions::Versioning;

/// Static definition of a mod loader — how to download it, where mods go.
pub struct LoaderDef {
    pub github_owner: &'static str,
    pub github_repo: &'static str,
    /// Construct the asset filename from (tag, arch).
    pub asset_fn: fn(tag: &str, arch: &str) -> String,
    /// Relative mod directory within the loader install path.
    /// Empty string means game root.
    pub mod_dir: &'static str,
    /// Optional post-install step after mod files are extracted.
    /// Receives (mod_slug, mod_install_path) where mod_install_path is the
    /// top-level mod directory (e.g. `Mods/` for UE4SS).
    pub post_install: Option<fn(mod_slug: &str, mod_dir_path: &Path) -> Result<()>>,
}

impl LoaderDef {
    /// Whether this loader requires a download (false for `none`).
    pub fn needs_download(&self) -> bool {
        !self.github_owner.is_empty()
    }
}

static UE4SS: LoaderDef = LoaderDef {
    github_owner: "UE4SS-RE",
    github_repo: "RE-UE4SS",
    asset_fn: |tag, _arch| format!("UE4SS_{tag}.zip"),
    mod_dir: "Mods",
    post_install: Some(enable_ue4ss_mod),
};

static BEPINEX: LoaderDef = LoaderDef {
    github_owner: "BepInEx",
    github_repo: "BepInEx",
    asset_fn: |_tag, arch| {
        let version = normalize_version(_tag);
        format!("BepInEx_win_{arch}_{version}.zip")
    },
    mod_dir: "BepInEx/plugins",
    post_install: None,
};

static MELONLOADER: LoaderDef = LoaderDef {
    github_owner: "LavaGang",
    github_repo: "MelonLoader",
    asset_fn: |_tag, arch| format!("MelonLoader.{arch}.zip"),
    mod_dir: "Mods",
    post_install: None,
};

static NONE: LoaderDef = LoaderDef {
    github_owner: "",
    github_repo: "",
    asset_fn: |_, _| String::new(),
    mod_dir: "",
    post_install: None,
};

/// Look up the loader definition by kind.
pub fn get_loader_def(kind: LoaderKind) -> &'static LoaderDef {
    match kind {
        LoaderKind::Ue4ss => &UE4SS,
        LoaderKind::BepInEx => &BEPINEX,
        LoaderKind::MelonLoader => &MELONLOADER,
        LoaderKind::None => &NONE,
    }
}

/// Resolve a loader version against GitHub releases.
/// - If `requested` is None, fetches the latest release tag.
/// - If `requested` is a partial version (e.g. "5.4"), finds the latest matching tag.
/// - If `requested` is an exact version that matches a tag, uses it directly.
///
/// Returns `(tag, version)` where tag is e.g. "v3.0.1" and version is "3.0.1".
pub fn resolve_loader_version(
    def: &LoaderDef,
    requested: Option<&str>,
) -> Result<(String, String)> {
    if !def.needs_download() {
        return Ok((String::new(), String::new()));
    }

    match requested {
        None => {
            // Fetch latest release
            let tags = registry::list_release_tags(def.github_owner, def.github_repo)?;
            let tag = find_latest_tag(&tags)?;
            let version = normalize_version(&tag).to_string();
            Ok((tag, version))
        }
        Some(req) => {
            let tags = registry::list_release_tags(def.github_owner, def.github_repo)?;

            // Try exact match first (with and without v prefix)
            let exact_v = format!("v{req}");
            if tags.contains(&exact_v) {
                return Ok((exact_v, req.to_string()));
            }
            if tags.contains(&req.to_string()) {
                let version = normalize_version(req).to_string();
                return Ok((req.to_string(), version));
            }

            // Partial match: find all tags that start with the requested prefix
            let matching = find_best_partial_match(&tags, req)?;
            let version = normalize_version(&matching).to_string();
            Ok((matching, version))
        }
    }
}

/// Find the latest tag by version ordering.
fn find_latest_tag(tags: &[String]) -> Result<String> {
    if tags.is_empty() {
        bail!("no releases found");
    }

    tags.iter()
        .max_by(|a, b| {
            let va = Versioning::new(normalize_version(a));
            let vb = Versioning::new(normalize_version(b));
            match (va, vb) {
                (Some(va), Some(vb)) => va.cmp(&vb),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => a.cmp(b),
            }
        })
        .cloned()
        .context("no releases found")
}

/// Find the best matching tag for a partial version like "5.4".
/// Matches all tags whose version starts with the requested prefix,
/// then returns the latest one by version ordering.
fn find_best_partial_match(tags: &[String], prefix: &str) -> Result<String> {
    let matching: Vec<&String> = tags
        .iter()
        .filter(|tag| {
            let ver = normalize_version(tag);
            ver.starts_with(prefix)
                && (ver.len() == prefix.len() || ver.as_bytes().get(prefix.len()) == Some(&b'.'))
        })
        .collect();

    if matching.is_empty() {
        bail!("no release matching version '{prefix}' found");
    }

    matching
        .iter()
        .max_by(|a, b| {
            let va = Versioning::new(normalize_version(a));
            let vb = Versioning::new(normalize_version(b));
            match (va, vb) {
                (Some(va), Some(vb)) => va.cmp(&vb),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => a.cmp(b),
            }
        })
        .map(|t| (*t).clone())
        .context("no matching release found")
}

/// Enable a UE4SS mod by adding/updating its entry in `mods.txt`.
/// `mod_slug` is the mod folder name, `mod_dir_path` is the `Mods/` directory.
fn enable_ue4ss_mod(mod_slug: &str, mod_dir_path: &Path) -> Result<()> {
    let mods_txt = mod_dir_path.join("mods.txt");

    let mut lines: Vec<String> = if mods_txt.exists() {
        std::fs::read_to_string(&mods_txt)
            .with_context(|| format!("failed to read {}", mods_txt.display()))?
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        Vec::new()
    };

    // Check if the mod is already listed
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        // Match "ModName : 0" or "ModName : 1" (with flexible whitespace)
        if let Some(name) = trimmed.split(':').next() {
            if name.trim() == mod_slug {
                *line = format!("{mod_slug} : 1");
                found = true;
                break;
            }
        }
    }

    if !found {
        lines.insert(0, format!("{mod_slug} : 1"));
    }

    std::fs::write(&mods_txt, lines.join("\n") + "\n")
        .with_context(|| format!("failed to write {}", mods_txt.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_latest_tag() {
        let tags = vec![
            "v1.0.0".to_string(),
            "v2.0.0".to_string(),
            "v1.5.0".to_string(),
        ];
        assert_eq!(find_latest_tag(&tags).unwrap(), "v2.0.0");
    }

    #[test]
    fn test_find_best_partial_match() {
        let tags = vec![
            "v5.4.21".to_string(),
            "v5.4.22".to_string(),
            "v5.4.23.5".to_string(),
            "v5.3.0".to_string(),
            "v6.0.0-pre.1".to_string(),
        ];
        assert_eq!(find_best_partial_match(&tags, "5.4").unwrap(), "v5.4.23.5");
        assert_eq!(find_best_partial_match(&tags, "5.3").unwrap(), "v5.3.0");
        assert!(find_best_partial_match(&tags, "7.0").is_err());
    }

    #[test]
    fn test_partial_match_no_false_prefix() {
        let tags = vec![
            "v5.4.0".to_string(),
            "v5.40.0".to_string(),
        ];
        // "5.4" should match "5.4.0" but NOT "5.40.0"
        assert_eq!(find_best_partial_match(&tags, "5.4").unwrap(), "v5.4.0");
    }

    #[test]
    fn test_enable_ue4ss_mod_new_file() {
        let dir = std::env::temp_dir().join("accessforge_test_mods_txt_new");
        let _ = std::fs::create_dir_all(&dir);
        let mods_txt = dir.join("mods.txt");
        let _ = std::fs::remove_file(&mods_txt);

        enable_ue4ss_mod("my-mod", &dir).unwrap();

        let content = std::fs::read_to_string(&mods_txt).unwrap();
        assert!(content.contains("my-mod : 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_enable_ue4ss_mod_existing_disabled() {
        let dir = std::env::temp_dir().join("accessforge_test_mods_txt_disabled");
        let _ = std::fs::create_dir_all(&dir);
        let mods_txt = dir.join("mods.txt");
        std::fs::write(&mods_txt, "other-mod : 1\nmy-mod : 0\n").unwrap();

        enable_ue4ss_mod("my-mod", &dir).unwrap();

        let content = std::fs::read_to_string(&mods_txt).unwrap();
        assert!(content.contains("my-mod : 1"));
        assert!(content.contains("other-mod : 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_enable_ue4ss_mod_already_enabled() {
        let dir = std::env::temp_dir().join("accessforge_test_mods_txt_enabled");
        let _ = std::fs::create_dir_all(&dir);
        let mods_txt = dir.join("mods.txt");
        std::fs::write(&mods_txt, "my-mod : 1\n").unwrap();

        enable_ue4ss_mod("my-mod", &dir).unwrap();

        let content = std::fs::read_to_string(&mods_txt).unwrap();
        // Should still have exactly one entry
        assert_eq!(content.matches("my-mod").count(), 1);
        assert!(content.contains("my-mod : 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
