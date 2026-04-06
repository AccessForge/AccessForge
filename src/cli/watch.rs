use anyhow::{Context, Result};
use std::path::Path;

use crate::installer;
use crate::manifest::Manifest;

/// Handle `dev watch --from <path>` command.
pub fn dev_watch(from: &Path) -> Result<()> {
    let manifest_path = from.join("accessforge.yml");
    let manifest = Manifest::from_file(&manifest_path)?;
    let loader = manifest.loader_kind()?;

    let game_root = installer::resolve_game_path(&manifest, None)?
        .context("could not find game directory")?;

    let mod_path = installer::mod_install_path(loader, &game_root)?;

    let target = if loader == crate::manifest::LoaderKind::Ue4ss {
        mod_path.join(manifest.slug()).join("Scripts")
    } else {
        mod_path
    };

    std::fs::create_dir_all(&target)?;

    println!("Watching: {}", from.display());
    println!("Target:   {}", target.display());
    println!("Press Ctrl+C to stop.");

    loop {
        super::copy_dir_contents(from, &target)?;
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
