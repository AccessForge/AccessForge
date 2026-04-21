use crate::installer;
use crate::installer::loaders::get_loader_def;
use crate::manifest::{DepType, Manifest, Source, normalize_version, slugify};
use crate::registry;
use crate::state::{AppState, ModState};
use crate::worker::{InstallStep, ProgressMsg, ProgressTx, TaskResult, report, status};
use std::collections::HashMap;
use std::path::PathBuf;

/// Run mod installation with progress reporting.
pub fn run_install(tx: ProgressTx, manifest: Manifest, game_root: PathBuf) {
    let loader = match manifest.loader_kind() {
        Ok(l) => l,
        Err(e) => {
            report(&tx, ProgressMsg::Failed(format!("Invalid loader: {e:#}")));
            return;
        }
    };

    // Install loader
    report(&tx, ProgressMsg::InstallProgress {
        step: InstallStep::InstallingLoader,
        detail: manifest.loader.name().to_string(),
    });
    status(&tx, format!("Installing {}...", manifest.loader.name()));

    if let Err(e) = installer::install_loader(loader, manifest.loader.version(), manifest.loader.arch(), &game_root) {
        report(&tx, ProgressMsg::Failed(format!("Failed to install loader: {e:#}")));
        return;
    }

    // Install dependencies
    for dep in &manifest.dependencies {
        report(&tx, ProgressMsg::InstallProgress {
            step: InstallStep::InstallingDependency,
            detail: dep.name.clone(),
        });
        status(&tx, format!("Installing dependency: {}...", dep.name));

        let dep_type = match DepType::from_str(&dep.dep_type) {
            Ok(t) => t,
            Err(e) => {
                report(&tx, ProgressMsg::Failed(format!("Bad dependency type for '{}': {e:#}", dep.name)));
                return;
            }
        };

        let source = match Source::parse(&dep.source) {
            Ok(s) => s,
            Err(e) => {
                report(&tx, ProgressMsg::Failed(format!("Bad source for '{}': {e:#}", dep.name)));
                return;
            }
        };

        let asset = match registry::fetch_release_for_source(&source, &dep.asset, &dep.version) {
            Ok(Some(a)) => a,
            Ok(None) => {
                report(&tx, ProgressMsg::Failed(format!("No release found for dependency '{}'", dep.name)));
                return;
            }
            Err(e) => {
                report(&tx, ProgressMsg::Failed(format!("Failed to fetch dependency '{}': {e:#}", dep.name)));
                return;
            }
        };

        let data = match installer::download(&asset.download_url) {
            Ok(d) => d,
            Err(e) => {
                report(&tx, ProgressMsg::Failed(format!("Failed to download '{}': {e:#}", dep.name)));
                return;
            }
        };

        let target = match dep_type {
            DepType::Patch => match installer::patch_install_path(loader, &game_root) {
                Ok(p) => p,
                Err(e) => { report(&tx, ProgressMsg::Failed(format!("{e:#}"))); return; }
            },
            DepType::Mod => match installer::mod_install_path(loader, &game_root) {
                Ok(p) => p,
                Err(e) => { report(&tx, ProgressMsg::Failed(format!("{e:#}"))); return; }
            },
        };

        if let Err(e) = installer::extract_zip(&data, &target) {
            report(&tx, ProgressMsg::Failed(format!("Failed to extract '{}': {e:#}", dep.name)));
            return;
        }
    }

    // Install the mod itself
    report(&tx, ProgressMsg::InstallProgress {
        step: InstallStep::InstallingMod,
        detail: manifest.name.clone(),
    });
    status(&tx, format!("Installing {}...", manifest.name));

    let source = match manifest.parsed_source() {
        Ok(s) => s,
        Err(e) => {
            report(&tx, ProgressMsg::Failed(format!("{e:#}")));
            return;
        }
    };

    let asset = match registry::fetch_release_for_source(
        &source,
        &manifest.release.asset,
        &manifest.version,
    ) {
        Ok(Some(a)) => a,
        Ok(None) => {
            report(&tx, ProgressMsg::Failed("No release found for the mod".to_string()));
            return;
        }
        Err(e) => {
            report(&tx, ProgressMsg::Failed(format!("Failed to fetch mod release: {e:#}")));
            return;
        }
    };

    let data = match installer::download(&asset.download_url) {
        Ok(d) => d,
        Err(e) => {
            report(&tx, ProgressMsg::Failed(format!("Failed to download mod: {e:#}")));
            return;
        }
    };

    let mod_path = match installer::mod_install_path(loader, &game_root) {
        Ok(p) => p,
        Err(e) => {
            report(&tx, ProgressMsg::Failed(format!("{e:#}")));
            return;
        }
    };

    let extract_result = if loader == crate::manifest::LoaderKind::Ue4ss {
        let mod_dir = mod_path.join(manifest.slug());
        std::fs::create_dir_all(&mod_dir)
            .map_err(anyhow::Error::from)
            .and_then(|_| installer::extract_zip(&data, &mod_dir))
    } else {
        std::fs::create_dir_all(&mod_path)
            .map_err(anyhow::Error::from)
            .and_then(|_| installer::extract_zip(&data, &mod_path))
    };

    if let Err(e) = extract_result {
        report(&tx, ProgressMsg::Failed(format!("Failed to extract mod: {e:#}")));
        return;
    }

    // Run loader-specific post-install (e.g. UE4SS mods.txt)
    let def = get_loader_def(loader);
    if let Some(post_install) = def.post_install {
        report(&tx, ProgressMsg::InstallProgress {
            step: InstallStep::PostInstall,
            detail: String::new(),
        });
        if let Err(e) = post_install(&manifest.slug(), &mod_path) {
            report(&tx, ProgressMsg::Failed(format!("Post-install failed: {e:#}")));
            return;
        }
    }

    // Save state
    report(&tx, ProgressMsg::InstallProgress {
        step: InstallStep::SavingState,
        detail: String::new(),
    });

    let version = normalize_version(&asset.tag).to_string();

    let mut state = AppState::load().unwrap_or_else(|e| {
        eprintln!("warning: failed to load state, starting fresh: {e:#}");
        AppState::new()
    });
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
            version: version.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            loader: manifest.loader.name().to_string(),
            local_path: None,
            dependencies: deps,
        },
    );

    if let Err(e) = state.save() {
        report(&tx, ProgressMsg::Failed(format!("Failed to save state: {e:#}")));
        return;
    }

    report(&tx, ProgressMsg::Done(TaskResult::Install {
        mod_name: manifest.name.clone(),
        version,
    }));
}
