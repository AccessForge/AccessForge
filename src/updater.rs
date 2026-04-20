use anyhow::{Context, Result};
use versions::Versioning;

use crate::installer;
use crate::registry;
use crate::state::AppState;
use crate::worker::{InstallStep, ProgressMsg, ProgressTx, TaskResult, report};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_OWNER: &str = "AccessForge";
const UPDATE_REPO: &str = "AccessForge";
const UPDATE_ASSET: &str = "AccessForge.exe";

/// Information about an available update.
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

/// Return the current app version.
pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

/// Check GitHub for a newer version of AccessForge.
/// Returns `Some(UpdateInfo)` if an update is available.
pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let asset = registry::fetch_latest_release_asset(UPDATE_OWNER, UPDATE_REPO, UPDATE_ASSET)?;

    let Some(asset) = asset else {
        return Ok(None);
    };

    let latest_ver = asset.tag.strip_prefix('v').unwrap_or(&asset.tag);

    let current = Versioning::new(CURRENT_VERSION);
    let latest = Versioning::new(latest_ver);

    match (current, latest) {
        (Some(c), Some(l)) if l > c => Ok(Some(UpdateInfo {
            version: latest_ver.to_string(),
            download_url: asset.download_url,
        })),
        _ => Ok(None),
    }
}

/// Check for updates and save the check timestamp to state.
pub fn check_and_record() -> Result<Option<UpdateInfo>> {
    let result = check_for_update();

    // Record the check regardless of outcome
    if let Ok(mut state) = AppState::load() {
        state.mark_update_checked();
        if let Err(e) = state.save() {
            eprintln!("warning: failed to save update check timestamp: {e:#}");
        }
    }

    result
}

/// Download the update and perform the rename-swap.
/// Sends progress via the channel. Does NOT restart — caller handles that.
pub fn apply_update(info: &UpdateInfo, tx: &ProgressTx) -> Result<()> {
    let exe_path = std::env::current_exe().context("failed to locate current executable")?;
    let exe_dir = exe_path.parent().context("executable has no parent directory")?;
    let new_path = exe_dir.join(format!("{}.new", UPDATE_ASSET));
    let old_path = exe_dir.join(format!("{}.old", UPDATE_ASSET));

    // Download
    report(tx, ProgressMsg::InstallProgress {
        step: InstallStep::InstallingMod,
        detail: format!("AccessForge {}", info.version),
    });

    let data = installer::download(&info.download_url)
        .context("failed to download update")?;

    report(tx, ProgressMsg::Status("Writing update...".to_string()));

    std::fs::write(&new_path, &data)
        .with_context(|| format!("failed to write {}", new_path.display()))?;

    // Rename swap
    report(tx, ProgressMsg::InstallProgress {
        step: InstallStep::SavingState,
        detail: "Applying update".to_string(),
    });

    // Remove stale .old if it exists
    if old_path.exists() {
        let _ = std::fs::remove_file(&old_path);
    }

    // Current → .old
    std::fs::rename(&exe_path, &old_path)
        .with_context(|| format!("failed to rename current exe to {}", old_path.display()))?;

    // .new → current
    if let Err(e) = std::fs::rename(&new_path, &exe_path) {
        // Rollback: .old → current
        let _ = std::fs::rename(&old_path, &exe_path);
        return Err(e).with_context(|| "failed to rename new exe into place");
    }

    report(tx, ProgressMsg::Done(TaskResult::Install {
        mod_name: "AccessForge".to_string(),
        version: info.version.clone(),
    }));

    Ok(())
}

/// Spawn the updated executable and exit the current process.
pub fn restart() -> ! {
    let exe = std::env::current_exe().expect("failed to locate executable");
    let args: Vec<String> = std::env::args().skip(1).collect();

    let _ = std::process::Command::new(&exe)
        .args(&args)
        .spawn();

    std::process::exit(0);
}

/// Delete the .old file from a previous update, if it exists.
pub fn cleanup_old() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(dir) = exe.parent() else { return };
    let old_path = dir.join(format!("{}.old", UPDATE_ASSET));
    if old_path.exists() {
        let _ = std::fs::remove_file(&old_path);
    }
}

