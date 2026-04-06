#![windows_subsystem = "windows"]

mod cli;
mod installer;
mod manifest;
pub mod path_setup;
mod registry;
mod state;
mod steam;
mod ui;
pub mod updater;
mod worker;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "accessforge",
    about = "1-click accessibility mod platform for blind gamers",
    version = env!("CARGO_PKG_VERSION"),
)]
struct App {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Create an accessforge.yml manifest
    Init {
        /// Path to the mod project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Install a mod from a local folder or HTTP URL
    Install {
        /// Path to a local folder, or an HTTP URL
        #[arg(default_value = ".")]
        path: String,
    },

    /// Build a release zip from a local folder
    Package {
        /// Path to the mod project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Add AccessForge to your PATH
    SetupPath,

    /// Launch the GUI with mock data (for testing)
    #[command(name = "--mock", hide = true)]
    Mock,
}

fn main() -> Result<()> {
    // Attach to the parent console for CLI output (windows_subsystem = "windows" hides it)
    if std::env::args().len() > 1 {
        unsafe { windows_sys::Win32::System::Console::AttachConsole(u32::MAX); }
    }

    // Clean up leftover .old file from a previous update
    updater::cleanup_old();

    let app = App::parse();

    match app.command {
        None => ui::run(false),
        Some(Command::Mock) => ui::run(true),
        Some(Command::Init { path }) => {
            let path = resolve_path(path);
            cli::dev_init(&path)
        }
        Some(Command::Install { path }) => {
            if path.starts_with("http://") || path.starts_with("https://") {
                cli::dev_install_url(&path)
            } else {
                let path = resolve_path(PathBuf::from(&path));
                cli::dev_install(&path)
            }
        }
        Some(Command::Package { path }) => {
            let path = resolve_path(path);
            cli::dev_package(&path)
        }
        Some(Command::SetupPath) => {
            if path_setup::is_on_path()? {
                println!("AccessForge is already on your PATH.");
            } else {
                path_setup::add_to_path()?;
                println!("AccessForge added to PATH. Restart your terminal for the change to take effect.");
            }
            Ok(())
        }
    }
}

/// Resolve a path argument to an absolute path using the current working directory.
fn resolve_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
