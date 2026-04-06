use std::collections::HashSet;

use crate::manifest::{Manifest, slugify};
use crate::registry::{self, CachedResponse};
use crate::state::{AppState, CachedManifest, ModState};
use crate::worker::{LoadedMod, ProgressMsg, ProgressTx, TaskResult, report, status};

/// Run mod discovery, sending progress and results through the channel.
pub fn discover_all(tx: ProgressTx, mut state: AppState) {
    status(&tx, "Searching for mods...");

    let repos = match registry::discover_mods() {
        Ok(r) => r,
        Err(e) => {
            // Offline fallback: use cached manifests
            if !state.manifest_cache.is_empty() {
                status(&tx, "Offline — showing cached mods...");
                discover_from_cache(&tx, &state);
                return;
            }
            report(&tx, ProgressMsg::Failed(format!("Failed to search GitHub: {e:#}")));
            return;
        }
    };

    report(&tx, ProgressMsg::DiscoveryStarted { repo_count: repos.len() });

    let mut cache_updated = false;
    let mut discovered_mods: HashSet<(String, String)> = HashSet::new();

    for (i, disc) in repos.iter().enumerate() {
        status(&tx, format!(
            "Loading {} of {}: {}/{}",
            i + 1,
            repos.len(),
            disc.owner,
            disc.repo
        ));

        let cache_key = format!("{}/{}", disc.owner, disc.repo);
        let cached = state.manifest_cache.get(&cache_key);
        // Don't send etag if cached yaml is empty — force a fresh fetch
        let cached_etag = cached
            .filter(|c| !c.yaml.is_empty())
            .and_then(|c| c.etag.as_deref());

        let yaml = match registry::fetch_manifest_yaml_cached(&disc.owner, &disc.repo, cached_etag) {
            Ok(CachedResponse::Fresh { yaml, etag }) => {
                // Update cache
                state.manifest_cache.insert(cache_key.clone(), CachedManifest {
                    etag,
                    yaml: yaml.clone(),
                });
                cache_updated = true;
                yaml
            }
            Ok(CachedResponse::NotModified) => {
                // Use cached version
                match cached {
                    Some(c) => c.yaml.clone(),
                    None => {
                        report(&tx, ProgressMsg::ModSkipped {
                            owner: disc.owner.clone(),
                            repo: disc.repo.clone(),
                            reason: "304 but no cached data".to_string(),
                        });
                        continue;
                    }
                }
            }
            Err(e) => {
                // Network error — try cache fallback
                if let Some(c) = cached {
                    c.yaml.clone()
                } else {
                    report(&tx, ProgressMsg::ModSkipped {
                        owner: disc.owner.clone(),
                        repo: disc.repo.clone(),
                        reason: format!("{e:#}"),
                    });
                    continue;
                }
            }
        };

        let manifest = match Manifest::from_yaml(&yaml) {
            Ok(m) => m,
            Err(e) => {
                report(&tx, ProgressMsg::ModSkipped {
                    owner: disc.owner.clone(),
                    repo: disc.repo.clone(),
                    reason: format!("invalid manifest: {e:#}"),
                });
                continue;
            }
        };

        let game_slug = manifest.game_slug();
        let mod_slug = manifest.slug().to_string();
        let installed = state.installed_mod(&game_slug, &mod_slug).cloned();

        discovered_mods.insert((game_slug, mod_slug));

        let latest_tag = Some(manifest.version.clone());

        report(&tx, ProgressMsg::ModLoaded(Box::new(LoadedMod {
            manifest,
            installed,
            latest_tag,
        })));
    }

    // Emit locally-installed mods not found via GitHub discovery
    emit_local_only_mods(&tx, &state, &discovered_mods);

    // Save updated cache
    if cache_updated {
        let _ = state.save();
    }

    report(&tx, ProgressMsg::DiscoveryFinished);
    report(&tx, ProgressMsg::Done(TaskResult::Discovery));
}

/// Emit installed mods from state.json that weren't found via GitHub discovery.
/// Tries to fetch the latest manifest from the remote source for update checking.
/// Falls back to a minimal manifest from state if the fetch fails.
fn emit_local_only_mods(
    tx: &ProgressTx,
    state: &AppState,
    discovered: &HashSet<(String, String)>,
) {
    use crate::manifest::Source;

    for (game_slug, game) in &state.games {
        for (mod_slug, mod_state) in &game.mods {
            if discovered.contains(&(game_slug.clone(), mod_slug.clone())) {
                continue;
            }

            // Try to fetch the latest manifest from the remote source
            let remote_manifest = Source::parse(&mod_state.source)
                .ok()
                .and_then(|source| {
                    registry::fetch_manifest_for_source(&source).ok()
                })
                .and_then(|yaml| Manifest::from_yaml(&yaml).ok());

            if let Some(manifest) = remote_manifest {
                let latest_tag = Some(manifest.version.clone());
                report(tx, ProgressMsg::ModLoaded(Box::new(LoadedMod {
                    manifest,
                    installed: Some(mod_state.clone()),
                    latest_tag,
                })));
                continue;
            }

            // Fallback: build a minimal manifest from state
            let yaml = format!(
                "spec: 1\nid: {mod_slug}\nname: \"{name}\"\ndescription: \"Locally installed\"\nauthor: unknown\nversion: \"{version}\"\nsource: \"{source}\"\ngame:\n  name: \"{game_name}\"\nloader:\n  name: {loader}\nrelease:\n  asset: \"placeholder.zip\"\n",
                name = mod_state.name.replace('"', "\\\""),
                version = mod_state.version,
                source = mod_state.source,
                game_name = game.name.replace('"', "\\\""),
                loader = mod_state.loader,
            );

            let manifest = match Manifest::from_yaml(&yaml) {
                Ok(m) => m,
                Err(_) => continue,
            };

            report(tx, ProgressMsg::ModLoaded(Box::new(LoadedMod {
                manifest,
                installed: Some(mod_state.clone()),
                latest_tag: None,
            })));
        }
    }
}

/// Discover mods from cache only (offline fallback).
fn discover_from_cache(tx: &ProgressTx, state: &AppState) {
    report(tx, ProgressMsg::DiscoveryStarted { repo_count: state.manifest_cache.len() });

    for (_key, cached) in &state.manifest_cache {
        let manifest = match Manifest::from_yaml(&cached.yaml) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let game_slug = manifest.game_slug();
        let mod_slug = manifest.slug();
        let installed = state.installed_mod(&game_slug, &mod_slug).cloned();
        let latest_tag = Some(manifest.version.clone());

        report(tx, ProgressMsg::ModLoaded(Box::new(LoadedMod {
            manifest,
            installed,
            latest_tag,
        })));
    }

    report(tx, ProgressMsg::DiscoveryFinished);
    report(tx, ProgressMsg::Done(TaskResult::Discovery));
}

/// Send mock mod data through the channel (for --mock mode).
pub fn discover_mock(tx: ProgressTx) {
    status(&tx, "Loading mock data...");

    let mods = build_mock_mods();
    report(&tx, ProgressMsg::DiscoveryStarted { repo_count: mods.len() });

    for m in mods {
        report(&tx, ProgressMsg::ModLoaded(Box::new(m)));
    }

    report(&tx, ProgressMsg::DiscoveryFinished);
    report(&tx, ProgressMsg::Done(TaskResult::Discovery));
}

fn build_mock_mods() -> Vec<LoadedMod> {
    let make = |name: &str, desc: &str, author: &str, game: &str, steam_id: u64,
                loader: &str, version: &str, installed: bool, update: bool,
                deps: Vec<(&str, &str, &str)>| {
        let slug = slugify(name);
        let mut dep_yaml = String::new();
        if !deps.is_empty() {
            dep_yaml.push_str("dependencies:\n");
            for (dname, dver, dtype) in &deps {
                let dslug = slugify(dname);
                dep_yaml.push_str(&format!(
                    "  - name: {dname}\n    type: {dtype}\n    source: github:accessforge/{dslug}\n    asset: \"{dslug}-{{version}}.zip\"\n    version: \"{dver}\"\n"
                ));
            }
        }
        let yaml = format!(
            r#"spec: 1
id: {slug}
name: {name}
description: {desc}
author: {author}
version: "{version}"
source: github:{author}/{slug}
game:
  name: {game}
  store:
    steam: {steam_id}
loader:
  name: {loader}
release:
  asset: "{slug}-{{version}}.zip"
{dep_yaml}"#,
        );
        let manifest = Manifest::from_yaml(&yaml).expect("bad mock manifest");
        let inst = if installed {
            Some(ModState {
                name: name.to_string(),
                source: format!("github:{author}/{slug}"),
                version: version.to_string(),
                installed_at: "2026-03-15T12:00:00Z".to_string(),
                loader: loader.to_string(),
                local_path: None,
                dependencies: deps.iter().map(|(n, v, _)| (slugify(n), v.to_string())).collect(),
            })
        } else {
            None
        };
        let latest = if update {
            Some("99.0.0".to_string())
        } else {
            Some(format!("v{version}"))
        };
        LoadedMod { manifest, installed: inst, latest_tag: latest }
    };

    vec![
        make("Stardew Access", "Screen reader accessibility mod for Stardew Valley",
             "stardew-access", "Stardew Valley", 413150, "none", "1.6.0",
             false, false,
             vec![("SMAPI", "4.0.0", "mod")]),
        make("CrossedEyes", "Screen reader mod for CrossCode",
             "CCDirectLink", "CrossCode", 368340, "none", "0.5.2",
             false, false,
             vec![("CCLoader", "2.22.1", "mod"), ("CrossCode Input API", "1.0.0", "mod")]),
        make("Balatro Access", "Screen reader accessibility for Balatro",
             "balatro-access", "Balatro", 2379780, "none", "1.0.0",
             false, false, vec![]),
        make("Sparking Zero Access", "Screen reader accessibility mod for Dragon Ball Sparking! ZERO",
             "jessica", "Dragon Ball Sparking! ZERO", 1790600, "ue4ss", "1.2.0",
             true, false,
             vec![("UTOC Signature Bypass", "1.0.0", "patch")]),
        make("Hades Blind Accessibility", "Screen reader support for Hades",
             "hades-access", "Hades", 1145360, "bepinex", "2.1.0",
             true, false, vec![]),
        make("Factorio Access", "Screen reader accessibility for Factorio",
             "factorio-access", "Factorio", 427520, "none", "0.9.0",
             true, true, vec![]),
        make("Yu-Gi-Oh Blind Mode", "Screen reader mod for Yu-Gi-Oh Master Duel",
             "yugioh-access", "Yu-Gi-Oh! Master Duel", 1449850, "bepinex", "1.3.0",
             true, true, vec![]),
    ]
}
