use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::installer;
use crate::manifest::{Manifest, slugify};
use crate::state::{AppState, ModState};

/// Find accessforge.yml in the given directory or any parent directory.
/// Returns (manifest, manifest_dir).
fn find_manifest(from: &Path) -> Result<(Manifest, PathBuf)> {
    let mut dir = from.to_path_buf();
    loop {
        let candidate = dir.join("accessforge.yml");
        if candidate.exists() {
            let manifest = Manifest::from_file(&candidate)?;
            return Ok((manifest, dir));
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!(
        "no accessforge.yml found in {} or any parent directory",
        from.display()
    );
}

/// Handle `install --from <path>` command.
pub fn dev_install(from: &Path) -> Result<()> {
    let (manifest, manifest_dir) = find_manifest(from)?;

    if manifest_dir != from {
        println!(
            "Found manifest in {}",
            manifest_dir.display()
        );
        println!("Copying files from {}", from.display());
    }

    println!("Mod: {}", manifest.name);
    println!("Game: {}", manifest.game.name);

    let state = AppState::load().unwrap_or_else(|_| AppState::new());
    let game_slug = manifest.game_slug();
    let saved_path = state.games.get(&game_slug).map(|g| g.path.clone());

    let game_root = installer::resolve_game_path(&manifest, saved_path.as_deref())?
        .context("could not find game directory — please specify manually")?;

    println!("Game found at: {}", game_root.display());

    let loader = manifest.loader_kind()?;

    // Install loader
    println!("Installing {}...", manifest.loader.name());
    installer::install_loader(
        loader,
        manifest.loader.version(),
        manifest.loader.arch(),
        &game_root,
    )?;

    // Install dependencies from remote
    for dep in &manifest.dependencies {
        println!("Installing dependency: {}...", dep.name);
        let source = crate::manifest::Source::parse(&dep.source)?;
        let asset =
            crate::registry::fetch_release_for_source(&source, &dep.asset, &dep.version)?
                .with_context(|| format!("no release found for dependency '{}'", dep.name))?;

        let data = installer::download(&asset.download_url)?;
        let dep_type = crate::manifest::DepType::from_str(&dep.dep_type)?;
        let target = match dep_type {
            crate::manifest::DepType::Patch => installer::patch_install_path(loader, &game_root)?,
            crate::manifest::DepType::Mod => installer::mod_install_path(loader, &game_root)?,
        };
        installer::extract_zip(&data, &target)?;
        println!("  Installed: {}", dep.name);
    }

    // Copy local mod files
    println!("Copying local mod files...");
    let mod_path = installer::mod_install_path(loader, &game_root)?;

    if loader == crate::manifest::LoaderKind::Ue4ss {
        // UE4SS: files go into Mods/<slug>/Scripts/
        let mod_dir = mod_path.join(manifest.slug()).join("Scripts");
        std::fs::create_dir_all(&mod_dir)?;
        super::copy_dir_contents(from, &mod_dir)?;
    } else {
        std::fs::create_dir_all(&mod_path)?;
        super::copy_dir_contents(from, &mod_path)?;
    }

    // Run post-install hook (e.g. UE4SS mods.txt)
    let def = crate::installer::loaders::get_loader_def(loader);
    if let Some(post_install) = def.post_install {
        println!("Running post-install...");
        post_install(manifest.slug(), &mod_path)?;
    }

    // Save state
    save_install_state(&manifest, &game_root, Some(from))?;

    println!("Done! {} installed from local source.", manifest.name);
    Ok(())
}

/// Handle `install --from <url>` command (HTTP source).
pub fn dev_install_url(url: &str) -> Result<()> {
    println!("Fetching manifest from {url}...");

    let source = crate::manifest::Source::parse(&format!("url:{url}"))?;
    let yaml = crate::registry::fetch_manifest_for_source(&source)?;
    let manifest = crate::manifest::Manifest::from_yaml(&yaml)?;

    println!("Mod: {} ({})", manifest.name, manifest.id);
    println!("Game: {}", manifest.game.name);

    let state = AppState::load().unwrap_or_else(|_| AppState::new());
    let game_slug = manifest.game_slug();
    let saved_path = state.games.get(&game_slug).map(|g| g.path.clone());

    let game_root = installer::resolve_game_path(&manifest, saved_path.as_deref())?
        .context("could not find game directory — please specify manually")?;

    println!("Game found at: {}", game_root.display());

    let loader = manifest.loader_kind()?;

    // Install loader
    println!("Installing {}...", manifest.loader.name());
    installer::install_loader(
        loader,
        manifest.loader.version(),
        manifest.loader.arch(),
        &game_root,
    )?;

    // Install dependencies
    for dep in &manifest.dependencies {
        println!("Installing dependency: {}...", dep.name);
        let dep_source = crate::manifest::Source::parse(&dep.source)?;
        let asset =
            crate::registry::fetch_release_for_source(&dep_source, &dep.asset, &dep.version)?
                .with_context(|| format!("no release found for dependency '{}'", dep.name))?;

        let data = installer::download(&asset.download_url)?;
        let dep_type = crate::manifest::DepType::from_str(&dep.dep_type)?;
        let target = match dep_type {
            crate::manifest::DepType::Patch => installer::patch_install_path(loader, &game_root)?,
            crate::manifest::DepType::Mod => installer::mod_install_path(loader, &game_root)?,
        };
        installer::extract_zip(&data, &target)?;
        println!("  Installed: {}", dep.name);
    }

    // Install the mod itself from the URL source
    println!("Installing {}...", manifest.name);
    let mod_source = manifest.parsed_source()?;
    let asset = crate::registry::fetch_release_for_source(
        &mod_source,
        &manifest.release.asset,
        &manifest.version,
    )?
    .with_context(|| format!("no release found for '{}'", manifest.name))?;

    let data = installer::download(&asset.download_url)?;
    let mod_path = installer::mod_install_path(loader, &game_root)?;

    if loader == crate::manifest::LoaderKind::Ue4ss {
        let mod_dir = mod_path.join(manifest.slug()).join("Scripts");
        std::fs::create_dir_all(&mod_dir)?;
        installer::extract_zip(&data, &mod_dir)?;
    } else {
        std::fs::create_dir_all(&mod_path)?;
        installer::extract_zip(&data, &mod_path)?;
    }

    // Run post-install hook (e.g. UE4SS mods.txt)
    let def = crate::installer::loaders::get_loader_def(loader);
    if let Some(post_install) = def.post_install {
        post_install(manifest.slug(), &mod_path)?;
    }

    // Save state
    save_install_state(&manifest, &game_root, None)?;

    println!("Done! {} v{} installed from {}.", manifest.name, manifest.version, url);
    Ok(())
}

/// Save install state so the GUI knows about this mod.
fn save_install_state(
    manifest: &Manifest,
    game_root: &Path,
    local_path: Option<&Path>,
) -> Result<()> {
    let mut state = AppState::load().unwrap_or_else(|_| AppState::new());
    let game_slug = manifest.game_slug();
    let game = state.get_or_create_game(
        &game_slug,
        &manifest.game.name,
        &game_root.to_string_lossy(),
    );

    let deps: HashMap<String, String> = manifest
        .dependencies
        .iter()
        .map(|d| (slugify(&d.name), d.version.clone()))
        .collect();

    game.mods.insert(
        manifest.slug().to_string(),
        ModState {
            name: manifest.name.clone(),
            source: manifest.source.clone(),
            version: manifest.version.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            loader: manifest.loader.name().to_string(),
            local_path: local_path.map(|p| p.to_string_lossy().to_string()),
            dependencies: deps,
        },
    );

    state.save().context("failed to save state")?;
    println!("State saved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_manifest(dir: &Path) {
        let yaml = r#"spec: 1
id: test-mod
name: Test Mod
description: A test
author: tester
version: "1.0.0"
source: github:tester/test-mod
game:
  name: Test Game
loader:
  name: none
release:
  asset: "test-{version}.zip"
"#;
        std::fs::write(dir.join("accessforge.yml"), yaml).unwrap();
    }

    #[test]
    fn test_find_manifest_in_current_dir() {
        let dir = std::env::temp_dir().join("accessforge_test_find_manifest_current");
        let _ = std::fs::create_dir_all(&dir);
        write_test_manifest(&dir);

        let (manifest, found_dir) = find_manifest(&dir).unwrap();
        assert_eq!(manifest.id, "test-mod");
        assert_eq!(found_dir, dir);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_manifest_in_parent_dir() {
        let parent = std::env::temp_dir().join("accessforge_test_find_manifest_parent");
        let child = parent.join("subdir");
        let _ = std::fs::create_dir_all(&child);
        write_test_manifest(&parent);

        let (manifest, found_dir) = find_manifest(&child).unwrap();
        assert_eq!(manifest.id, "test-mod");
        assert_eq!(found_dir, parent);

        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn test_find_manifest_not_found() {
        let dir = std::env::temp_dir().join("accessforge_test_find_manifest_none");
        let _ = std::fs::create_dir_all(&dir);
        // No manifest written

        let result = find_manifest(&dir);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
