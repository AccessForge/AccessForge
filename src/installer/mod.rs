pub mod loaders;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::manifest::{LoaderKind, Manifest};
use crate::steam;
use loaders::{get_loader_def, resolve_loader_version};

/// Download a file from a URL into memory.
/// Limit is set to 500MB to handle large mod loaders and mod files.
pub(crate) fn download(url: &str) -> Result<Vec<u8>> {
    let data = ureq::get(url)
        .header("User-Agent", "AccessForge")
        .call()
        .with_context(|| format!("failed to download {url}"))?
        .body_mut()
        .with_config()
        .limit(500 * 1024 * 1024)
        .read_to_vec()
        .context("failed to read download body")?;
    Ok(data)
}

/// Extract a zip archive to a target directory.
pub(crate) fn extract_zip(data: &[u8], target: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).context("failed to open zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("failed to read zip entry")?;
        let name = file.name().to_string();

        // Build a proper path from zip entry name (which uses '/' separators).
        // Reject any entry that contains '..' components to prevent path traversal
        // (a malicious zip could otherwise write files outside the target directory).
        let components: Vec<&str> = name.split('/').collect();
        if components.iter().any(|c| *c == "..") {
            anyhow::bail!("zip entry '{}' contains '..' and would escape the target directory", name);
        }
        let rel_path: PathBuf = components.iter().collect();

        if name.ends_with('/') {
            let dir = target.join(&rel_path);
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create directory {}", dir.display()))?;
        } else {
            let out_path = target.join(&rel_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)
                .with_context(|| format!("failed to create {}", out_path.display()))?;
            std::io::copy(&mut file, &mut out)?;
        }
    }

    Ok(())
}

/// Resolve the game install path: check state, try Steam, or return None (caller asks user).
pub fn resolve_game_path(
    manifest: &Manifest,
    saved_path: Option<&str>,
) -> Result<Option<PathBuf>> {
    if let Some(p) = saved_path {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return Ok(Some(path));
        }
    }

    if let Some(steam_id) = manifest.steam_id() {
        if let Some(path) = steam::find_game_path(steam_id as u32)? {
            if path.is_dir() {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

/// Resolve the loader install directory (where the loader itself gets extracted).
pub fn loader_install_path(loader: LoaderKind, game_root: &Path) -> Result<PathBuf> {
    match loader {
        LoaderKind::Ue4ss => {
            let bin = steam::find_ue_binaries(game_root)?
                .context("could not find Binaries/Win64 in game directory")?;
            Ok(bin)
        }
        _ => Ok(game_root.to_path_buf()),
    }
}

/// Determine where mod files should be extracted based on loader type.
pub fn mod_install_path(loader: LoaderKind, game_root: &Path) -> Result<PathBuf> {
    let def = get_loader_def(loader);
    let base = loader_install_path(loader, game_root)?;
    if def.mod_dir.is_empty() {
        Ok(base)
    } else {
        Ok(base.join(def.mod_dir))
    }
}

/// Determine where patches/exe-level deps should go.
pub fn patch_install_path(loader: LoaderKind, game_root: &Path) -> Result<PathBuf> {
    loader_install_path(loader, game_root)
}

/// Install a mod loader to the game directory.
pub fn install_loader(
    loader: LoaderKind,
    version: Option<&str>,
    arch: &str,
    game_root: &Path,
) -> Result<()> {
    let def = get_loader_def(loader);

    if !def.needs_download() {
        return Ok(());
    }

    let (tag, _version) = resolve_loader_version(def, version)?;
    let asset_name = (def.asset_fn)(&tag, arch);

    // Find the asset in the release
    let asset = crate::registry::fetch_github_release_by_tag(
        def.github_owner,
        def.github_repo,
        &asset_name,
        &_version,
    )?
    .with_context(|| {
        format!(
            "no release asset '{}' found for {}/{}",
            asset_name, def.github_owner, def.github_repo
        )
    })?;

    let data = download(&asset.download_url)?;
    let install_path = loader_install_path(loader, game_root)?;
    extract_zip(&data, &install_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn extract_zip_creates_flat_file() {
        let data = make_zip(&[("hello.txt", b"world")]);
        let dir = tempfile::tempdir().unwrap();
        extract_zip(&data, dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert_eq!(content, "world");
    }

    #[test]
    fn extract_zip_creates_nested_file() {
        let data = make_zip(&[("a/b/c.txt", b"nested")]);
        let dir = tempfile::tempdir().unwrap();
        extract_zip(&data, dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join("a/b/c.txt")).unwrap();
        assert_eq!(content, "nested");
    }

    #[test]
    fn extract_zip_multiple_files() {
        let data = make_zip(&[("a.txt", b"aaa"), ("b.txt", b"bbb")]);
        let dir = tempfile::tempdir().unwrap();
        extract_zip(&data, dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "aaa");
        assert_eq!(std::fs::read_to_string(dir.path().join("b.txt")).unwrap(), "bbb");
    }

    #[test]
    fn extract_zip_rejects_path_traversal() {
        let data = make_zip(&[("../escape.txt", b"evil")]);
        let dir = tempfile::tempdir().unwrap();
        let result = extract_zip(&data, dir.path());
        assert!(result.is_err(), "expected error for path traversal entry");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains(".."), "error should mention the offending pattern, got: {msg}");
    }

    #[test]
    fn extract_zip_rejects_nested_path_traversal() {
        let data = make_zip(&[("safe/../../escape.txt", b"evil")]);
        let dir = tempfile::tempdir().unwrap();
        let result = extract_zip(&data, dir.path());
        assert!(result.is_err(), "expected error for nested path traversal");
    }

    #[test]
    fn resolve_game_path_uses_saved_path_when_dir_exists() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = crate::manifest::Manifest {
            spec: 1,
            id: "test".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            author: String::new(),
            version: "1.0.0".to_string(),
            source: "github:owner/repo".to_string(),
            license: None,
            game: crate::manifest::Game { name: "TestGame".to_string(), store: None },
            loader: crate::manifest::LoaderField { name: "none".to_string(), version: None, arch: None },
            release: crate::manifest::Release { asset: "mod.zip".to_string() },
            dependencies: vec![],
        };
        let result = resolve_game_path(&manifest, Some(dir.path().to_str().unwrap())).unwrap();
        assert_eq!(result.unwrap(), dir.path());
    }

    #[test]
    fn resolve_game_path_skips_saved_path_when_missing() {
        let manifest = crate::manifest::Manifest {
            spec: 1,
            id: "test".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            author: String::new(),
            version: "1.0.0".to_string(),
            source: "github:owner/repo".to_string(),
            license: None,
            game: crate::manifest::Game { name: "TestGame".to_string(), store: None },
            loader: crate::manifest::LoaderField { name: "none".to_string(), version: None, arch: None },
            release: crate::manifest::Release { asset: "mod.zip".to_string() },
            dependencies: vec![],
        };
        let result = resolve_game_path(&manifest, Some("/nonexistent/path/xyz")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_game_path_returns_none_with_no_steam_id() {
        let manifest = crate::manifest::Manifest {
            spec: 1,
            id: "test".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            author: String::new(),
            version: "1.0.0".to_string(),
            source: "github:owner/repo".to_string(),
            license: None,
            game: crate::manifest::Game { name: "TestGame".to_string(), store: None },
            loader: crate::manifest::LoaderField { name: "none".to_string(), version: None, arch: None },
            release: crate::manifest::Release { asset: "mod.zip".to_string() },
            dependencies: vec![],
        };
        let result = resolve_game_path(&manifest, None).unwrap();
        assert!(result.is_none());
    }
}
