use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the path to the current executable's directory.
pub fn exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to locate executable")?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .context("executable has no parent directory")
}

/// Check if AccessForge's directory is on the user's PATH.
pub fn is_on_path() -> Result<bool> {
    let dir = exe_dir()?;
    let dir_str = dir.to_string_lossy().to_lowercase();

    if let Ok(path) = std::env::var("PATH") {
        for entry in path.split(';') {
            if entry.to_lowercase().trim_end_matches('\\') == dir_str.trim_end_matches('\\') {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Add AccessForge's directory to the user's PATH (HKCU registry, no admin needed).
pub fn add_to_path() -> Result<()> {
    let dir = exe_dir()?;
    let dir_str = dir.to_string_lossy().to_string();

    // Read current user PATH from registry
    let output = std::process::Command::new("reg")
        .args(["query", "HKCU\\Environment", "/v", "Path"])
        .output()
        .context("failed to read registry")?;

    let current = if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .find(|l| l.contains("REG_") && l.contains("Path"))
            .and_then(|line| {
                let parts: Vec<&str> = line.splitn(3, "    ").collect();
                parts.get(2).map(|v| v.trim().to_string())
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already present
    let already = current
        .split(';')
        .any(|e| e.to_lowercase().trim_end_matches('\\') == dir_str.to_lowercase().trim_end_matches('\\'));

    if already {
        return Ok(());
    }

    // Append our directory
    let new_path = if current.is_empty() {
        dir_str
    } else {
        format!("{};{}", current.trim_end_matches(';'), dir_str)
    };

    let status = std::process::Command::new("reg")
        .args(["add", "HKCU\\Environment", "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &new_path, "/f"])
        .status()
        .context("failed to write registry")?;

    if !status.success() {
        anyhow::bail!("failed to update PATH in registry");
    }

    // Broadcast WM_SETTINGCHANGE so other programs pick up the change
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "[Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"])
        .status();

    Ok(())
}
