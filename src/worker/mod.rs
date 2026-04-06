pub mod discover;
pub mod install;

use crate::manifest::Manifest;
use crate::state::{AppState, ModState};
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;

/// A sender for progress messages from a background thread.
pub type ProgressTx = mpsc::Sender<ProgressMsg>;

/// A mod that has been loaded from a remote source.
#[derive(Clone)]
pub struct LoadedMod {
    pub manifest: Manifest,
    pub installed: Option<ModState>,
    pub latest_tag: Option<String>,
}

/// Progress messages sent from background workers to the UI or CLI.
pub enum ProgressMsg {
    /// A status line to display.
    Status(String),

    /// Discovery found repos, now fetching manifests.
    DiscoveryStarted { repo_count: usize },

    /// One mod loaded successfully.
    ModLoaded(Box<LoadedMod>),

    /// One mod skipped due to an error.
    ModSkipped {
        owner: String,
        repo: String,
        reason: String,
    },

    /// All mods have been sent.
    DiscoveryFinished,

    /// An installation step is in progress.
    InstallProgress { step: InstallStep, detail: String },

    /// The operation completed successfully.
    Done(TaskResult),

    /// The operation failed.
    Failed(String),
}

#[derive(Debug, Clone, Copy)]
pub enum InstallStep {
    InstallingLoader,
    InstallingDependency,
    InstallingMod,
    PostInstall,
    SavingState,
}

impl fmt::Display for InstallStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallingLoader => write!(f, "Installing loader"),
            Self::InstallingDependency => write!(f, "Installing dependency"),
            Self::InstallingMod => write!(f, "Installing mod"),
            Self::PostInstall => write!(f, "Running post-install"),
            Self::SavingState => write!(f, "Saving state"),
        }
    }
}

pub enum TaskResult {
    /// Discovery complete.
    Discovery,
    /// Installation complete.
    Install { mod_name: String, version: String },
}

/// Send a progress message. Ignores send failures (receiver dropped = cancelled).
pub fn report(tx: &ProgressTx, msg: ProgressMsg) {
    let _ = tx.send(msg);
}

/// Send a status string.
pub fn status(tx: &ProgressTx, msg: impl Into<String>) {
    report(tx, ProgressMsg::Status(msg.into()));
}

/// Spawn a discovery worker thread.
pub fn spawn_discover(state: AppState) -> mpsc::Receiver<ProgressMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || discover::discover_all(tx, state));
    rx
}

/// Spawn a mock discovery worker thread (for --mock mode).
pub fn spawn_discover_mock() -> mpsc::Receiver<ProgressMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || discover::discover_mock(tx));
    rx
}

/// Spawn an installation worker thread.
pub fn spawn_install(manifest: Manifest, game_root: PathBuf) -> mpsc::Receiver<ProgressMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || install::run_install(tx, manifest, game_root));
    rx
}
