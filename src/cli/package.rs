use anyhow::{Context, Result};
use std::path::Path;

use crate::manifest::Manifest;

/// Handle `package --from <path>` command.
pub fn dev_package(from: &Path) -> Result<()> {
    let manifest_path = from.join("accessforge.yml");
    let manifest = Manifest::from_file(&manifest_path)?;

    let zip_name = crate::manifest::resolve_asset(&manifest.release.asset, &manifest.version);
    let zip_path = from.join(&zip_name);

    // Determine source directory: dist/ if it exists, otherwise project root
    let dist_dir = from.join("dist");
    let source_dir = if dist_dir.is_dir() {
        println!("Packaging from dist/ folder.");
        &dist_dir
    } else {
        println!("No dist/ folder found, packaging from project root.");
        from
    };

    println!("Mod: {} ({})", manifest.name, manifest.id);
    println!("Version: {}", manifest.version);

    let loader = manifest.loader_kind()?;
    let has_scripts_dir = source_dir.join("Scripts").is_dir();
    let zip_prefix = if loader == crate::manifest::LoaderKind::Ue4ss && !has_scripts_dir {
        "Scripts/"
    } else {
        ""
    };

    let file = std::fs::File::create(&zip_path)
        .with_context(|| format!("failed to create {}", zip_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    if !zip_prefix.is_empty() {
        zip.add_directory(zip_prefix, options)?;
    }
    add_dir_to_zip(&mut zip, source_dir, source_dir, zip_prefix, &options)?;

    zip.finish().context("failed to finalize zip")?;

    println!("Created: {}", zip_path.display());

    // Validate the zip structure
    validate_zip(&zip_path, loader)?;

    Ok(())
}

/// Validate that the zip structure looks correct for the given loader.
fn validate_zip(zip_path: &Path, loader: crate::manifest::LoaderKind) -> Result<()> {
    use crate::manifest::LoaderKind;

    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("failed to open {}", zip_path.display()))?;
    let archive = zip::ZipArchive::new(file)
        .context("failed to read zip for validation")?;

    let files: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.name_for_index(i).map(|n| n.to_string()))
        .filter(|n| !n.ends_with('/'))
        .collect();

    if files.is_empty() {
        println!("Warning: zip is empty — no files were packaged.");
        return Ok(());
    }

    println!("{} file(s) packaged.", files.len());

    match loader {
        LoaderKind::Ue4ss => {
            let has_scripts = files.iter().any(|f| f.starts_with("Scripts/"));
            if !has_scripts {
                println!("Warning: UE4SS mod but no files under Scripts/ in the zip. The mod may not load.");
            }
        }
        LoaderKind::BepInEx | LoaderKind::MelonLoader => {
            let has_dll = files.iter().any(|f| f.ends_with(".dll"));
            if !has_dll {
                println!("Warning: {} mod but no .dll files in the zip. The mod may not load.",
                    if loader == LoaderKind::BepInEx { "BepInEx" } else { "MelonLoader" });
            }
        }
        LoaderKind::None => {
            if files.len() == 1 {
                println!("Warning: only 1 file in the zip. Mods without a loader typically contain multiple files.");
            }
        }
    }

    Ok(())
}

/// Recursively add directory contents to a zip archive.
/// `prefix` is prepended to all paths in the zip (e.g. "Scripts/" for UE4SS).
fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    prefix: &str,
    options: &zip::write::SimpleFileOptions,
) -> Result<()> {
    use std::io::Read;

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip manifest, dotfiles, .git, zip files, dist folder
        if name_str == "accessforge.yml"
            || name_str.starts_with('.')
            || name_str == "dist"
            || name_str.ends_with(".zip")
        {
            continue;
        }

        let relative = path.strip_prefix(base)
            .context("failed to compute relative path")?;
        let zip_path = format!("{prefix}{}", relative.to_string_lossy().replace('\\', "/"));

        if path.is_dir() {
            zip.add_directory(&format!("{zip_path}/"), *options)?;
            add_dir_to_zip(zip, base, &path, prefix, options)?;
        } else {
            zip.start_file(&zip_path, *options)?;
            let mut f = std::fs::File::open(&path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            std::io::Write::write_all(zip, &buf)?;
        }
    }
    Ok(())
}
