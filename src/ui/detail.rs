use crate::worker::LoadedMod;
use versions::Versioning;
use wxdragon::prelude::*;

/// UI-side mod entry wrapping the shared LoadedMod.
#[derive(Clone)]
pub struct ModEntry(pub LoadedMod);

impl ModEntry {
    pub fn has_update(&self) -> bool {
        if let (Some(inst), Some(tag)) = (&self.0.installed, &self.0.latest_tag) {
            let installed_ver = inst.version.trim_start_matches('v');
            let latest_ver = tag.trim_start_matches('v');

            match (Versioning::new(installed_ver), Versioning::new(latest_ver)) {
                (Some(iv), Some(lv)) => lv > iv,
                _ => installed_ver != latest_ver,
            }
        } else {
            false
        }
    }

    pub fn is_installed(&self) -> bool {
        self.0.installed.is_some()
    }

    pub fn action_label(&self) -> &str {
        if !self.is_installed() {
            "Install"
        } else if self.has_update() {
            "Update"
        } else {
            "Reinstall"
        }
    }

    pub fn version_display(&self) -> String {
        let latest = self.latest_version();
        match &self.0.installed {
            Some(m) if self.has_update() => {
                format!("Installed: {}. Latest: {latest}", m.version)
            }
            Some(m) => format!("Version {}", m.version),
            None => format!("Version {latest}"),
        }
    }

    fn latest_version(&self) -> &str {
        self.0
            .latest_tag
            .as_deref()
            .map(|t| t.strip_prefix('v').unwrap_or(t))
            .unwrap_or(&self.0.manifest.version)
    }

    pub fn list_label_browse(&self) -> String {
        format!("{} ({})", self.0.manifest.name, self.0.manifest.game.name)
    }

    pub fn list_label_installed(&self) -> String {
        format!("{} ({})", self.0.manifest.name, self.0.manifest.game.name)
    }

    pub fn list_label_update(&self) -> String {
        format!("{} ({})", self.0.manifest.name, self.0.manifest.game.name)
    }

    fn meta_text(&self) -> String {
        let m = &self.0.manifest;
        format!("For {}, by {}. {}.", m.game.name, m.author, self.version_display())
    }

    fn also_installs_text(&self) -> String {
        let mod_deps: Vec<&str> = self
            .0
            .manifest
            .dependencies
            .iter()
            .filter(|d| d.dep_type == "mod")
            .map(|d| d.name.as_str())
            .collect();
        if mod_deps.is_empty() {
            String::new()
        } else {
            format!("Also installs: {}", mod_deps.join(", "))
        }
    }

    /// Full detail text for the read-only TextCtrl — all sections joined with blank lines.
    pub fn detail_text(&self) -> String {
        let m = &self.0.manifest;
        let mut parts = vec![
            m.name.clone(),
            m.description.clone(),
            self.meta_text(),
        ];
        let also = self.also_installs_text();
        if !also.is_empty() {
            parts.push(also);
        }
        let tech = self.tech_details_text();
        if !tech.is_empty() {
            parts.push(tech);
        }
        parts.join("\n\n")
    }

    fn tech_details_text(&self) -> String {
        let m = &self.0.manifest;
        let loader = &m.loader;

        let patch_deps: Vec<&str> = m
            .dependencies
            .iter()
            .filter(|d| d.dep_type == "patch")
            .map(|d| d.name.as_str())
            .collect();

        if loader.name() == "none" && patch_deps.is_empty() {
            return String::new();
        }

        let mut lines = vec![format!("Loader: {}", loader.name())];
        if let Some(ver) = loader.version() {
            lines.push(format!("Loader version: {ver}"));
        }
        if !patch_deps.is_empty() {
            lines.push(format!("Also installs patches: {}", patch_deps.join(", ")));
        }
        lines.join("\n")
    }
}

/// Detail panel — a focusable read-only TextCtrl so NVDA reads it on Tab,
/// plus an action button below.
#[derive(Copy, Clone)]
pub struct DetailPanel {
    detail_text: TextCtrl,
    pub action_btn: Button,
}

impl DetailPanel {
    pub fn build(parent: &Panel, empty_message: &str) -> Self {
        let detail_text = TextCtrl::builder(parent)
            .with_style(TextCtrlStyle::ReadOnly | TextCtrlStyle::MultiLine)
            .build();
        let action_btn = Button::builder(parent).with_label("Install").build();
        action_btn.enable(false);

        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(&detail_text, 1, SizerFlag::Expand | SizerFlag::All, 4);
        sizer.add(&action_btn, 0, SizerFlag::All, 8);
        parent.set_sizer(sizer, true);

        let panel = Self { detail_text, action_btn };
        panel.show_empty(empty_message);
        panel
    }

    pub fn show_empty(&self, message: &str) {
        self.detail_text.set_value(message);
        self.action_btn.enable(false);
    }

    pub fn populate(&self, entry: &ModEntry) {
        self.detail_text.set_value(&entry.detail_text());
        self.action_btn.set_label(entry.action_label());
        self.action_btn.enable(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Dependency, Game, LoaderField, Manifest, Release};
    use crate::state::ModState;
    use crate::worker::LoadedMod;
    use std::collections::HashMap;

    fn base_manifest() -> Manifest {
        Manifest {
            spec: 1,
            id: "test-mod".to_string(),
            name: "Test Mod".to_string(),
            description: "A test mod".to_string(),
            author: "TestAuthor".to_string(),
            version: "1.0.0".to_string(),
            source: "github:owner/repo".to_string(),
            license: None,
            game: Game { name: "Test Game".to_string(), store: None },
            loader: LoaderField { name: "ue4ss".to_string(), version: None, arch: None },
            release: Release { asset: "TestMod-{version}.zip".to_string() },
            dependencies: vec![],
        }
    }

    fn installed_state(version: &str) -> ModState {
        ModState {
            name: "Test Mod".to_string(),
            source: "github:owner/repo".to_string(),
            version: version.to_string(),
            installed_at: "2024-01-01T00:00:00Z".to_string(),
            loader: "ue4ss".to_string(),
            local_path: None,
            dependencies: HashMap::new(),
        }
    }

    fn entry(installed: Option<ModState>, latest_tag: Option<&str>) -> ModEntry {
        ModEntry(LoadedMod {
            manifest: base_manifest(),
            installed,
            latest_tag: latest_tag.map(|s| s.to_string()),
        })
    }

    // --- has_update ---

    #[test]
    fn has_update_not_installed_is_false() {
        assert!(!entry(None, Some("v1.0.0")).has_update());
    }

    #[test]
    fn has_update_no_latest_tag_is_false() {
        assert!(!entry(Some(installed_state("1.0.0")), None).has_update());
    }

    #[test]
    fn has_update_same_version_is_false() {
        assert!(!entry(Some(installed_state("1.0.0")), Some("v1.0.0")).has_update());
    }

    #[test]
    fn has_update_newer_available_is_true() {
        assert!(entry(Some(installed_state("1.0.0")), Some("v1.1.0")).has_update());
    }

    #[test]
    fn has_update_installed_is_newer_is_false() {
        assert!(!entry(Some(installed_state("2.0.0")), Some("v1.0.0")).has_update());
    }

    // --- action_label ---

    #[test]
    fn action_label_not_installed() {
        assert_eq!(entry(None, Some("v1.0.0")).action_label(), "Install");
    }

    #[test]
    fn action_label_installed_up_to_date() {
        assert_eq!(
            entry(Some(installed_state("1.0.0")), Some("v1.0.0")).action_label(),
            "Reinstall"
        );
    }

    #[test]
    fn action_label_update_available() {
        assert_eq!(
            entry(Some(installed_state("1.0.0")), Some("v1.1.0")).action_label(),
            "Update"
        );
    }

    // --- version_display ---

    #[test]
    fn version_display_not_installed_shows_latest() {
        assert_eq!(entry(None, Some("v1.5.0")).version_display(), "Version 1.5.0");
    }

    #[test]
    fn version_display_installed_current() {
        assert_eq!(
            entry(Some(installed_state("1.5.0")), Some("v1.5.0")).version_display(),
            "Version 1.5.0"
        );
    }

    #[test]
    fn version_display_update_available_shows_both() {
        assert_eq!(
            entry(Some(installed_state("1.0.0")), Some("v1.5.0")).version_display(),
            "Installed: 1.0.0. Latest: 1.5.0"
        );
    }

    #[test]
    fn version_display_falls_back_to_manifest_version() {
        assert_eq!(entry(None, None).version_display(), "Version 1.0.0");
    }

    // --- also_installs_text ---

    #[test]
    fn also_installs_text_no_deps_is_empty() {
        assert_eq!(entry(None, None).also_installs_text(), "");
    }

    #[test]
    fn also_installs_text_only_patches_is_empty() {
        let mut e = entry(None, None);
        e.0.manifest.dependencies = vec![Dependency {
            name: "utoc-bypass".to_string(),
            dep_type: "patch".to_string(),
            source: "github:owner/utoc-bypass".to_string(),
            asset: "utoc-{version}.zip".to_string(),
            version: "1.0.0".to_string(),
        }];
        assert_eq!(e.also_installs_text(), "");
    }

    #[test]
    fn also_installs_text_mod_dep_listed() {
        let mut e = entry(None, None);
        e.0.manifest.dependencies = vec![Dependency {
            name: "Screen Reader Helper".to_string(),
            dep_type: "mod".to_string(),
            source: "github:owner/helper".to_string(),
            asset: "helper-{version}.zip".to_string(),
            version: "1.0.0".to_string(),
        }];
        assert_eq!(e.also_installs_text(), "Also installs: Screen Reader Helper");
    }

    // --- tech_details_text ---

    #[test]
    fn tech_details_text_none_loader_no_patches_is_empty() {
        let mut e = entry(None, None);
        e.0.manifest.loader = LoaderField { name: "none".to_string(), version: None, arch: None };
        assert_eq!(e.tech_details_text(), "");
    }

    #[test]
    fn tech_details_text_ue4ss_shows_loader_name() {
        let text = entry(None, None).tech_details_text();
        assert!(text.contains("Loader: ue4ss"), "got: {text}");
        assert!(!text.contains("Loader version:"), "got: {text}");
    }

    #[test]
    fn tech_details_text_shows_loader_version() {
        let mut e = entry(None, None);
        e.0.manifest.loader =
            LoaderField { name: "ue4ss".to_string(), version: Some("5.4".to_string()), arch: None };
        let text = e.tech_details_text();
        assert!(text.contains("Loader version: 5.4"), "got: {text}");
    }

    #[test]
    fn tech_details_text_none_loader_with_patch_shows_patch() {
        let mut e = entry(None, None);
        e.0.manifest.loader = LoaderField { name: "none".to_string(), version: None, arch: None };
        e.0.manifest.dependencies = vec![Dependency {
            name: "utoc-bypass".to_string(),
            dep_type: "patch".to_string(),
            source: "github:owner/utoc-bypass".to_string(),
            asset: "utoc-{version}.zip".to_string(),
            version: "1.0.0".to_string(),
        }];
        let text = e.tech_details_text();
        assert!(text.contains("Also installs patches: utoc-bypass"), "got: {text}");
    }
}
