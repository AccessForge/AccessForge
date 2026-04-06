use anyhow::{Context, Result};
use std::path::PathBuf;
use steamlocate::SteamDir;

/// Find a game's install path by Steam app ID.
pub fn find_game_path(app_id: u32) -> Result<Option<PathBuf>> {
    Ok(find_game_info(app_id)?.map(|(_, path)| path))
}

/// Find a game's name and install path by Steam app ID.
pub fn find_game_info(app_id: u32) -> Result<Option<(String, PathBuf)>> {
    let steam_dir = SteamDir::locate().context("failed to locate Steam installation")?;

    match steam_dir.find_app(app_id) {
        Ok(Some((app, library))) => {
            let path = library.resolve_app_dir(&app);
            let name = app.name.unwrap_or_default();
            if name.is_empty() {
                Ok(Some((format!("App {app_id}"), path)))
            } else {
                Ok(Some((name, path)))
            }
        }
        Ok(None) => Ok(None),
        Err(e) => {
            eprintln!("Warning: error searching Steam for app {app_id}: {e}");
            Ok(None)
        }
    }
}

/// Find the Binaries/Win64 directory for UE games.
/// Scans one level deep inside game root for `*/Binaries/Win64/`.
pub fn find_ue_binaries(game_root: &std::path::Path) -> Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(game_root)
        .with_context(|| format!("failed to read game directory: {}", game_root.display()))?;

    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        // Skip the Engine directory — that's UE's own binaries, not the game's
        let name = entry.file_name();
        if name.eq_ignore_ascii_case("engine") {
            continue;
        }
        let bin_path = entry.path().join("Binaries").join("Win64");
        if bin_path.is_dir() {
            return Ok(Some(bin_path));
        }
    }

    Ok(None)
}
