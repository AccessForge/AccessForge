use crate::manifest::normalize_version;
use crate::worker::LoadedMod;
use versions::Versioning;
use wxdragon::prelude::*;
use wxdragon::widgets::WebView;

/// UI-side mod entry wrapping the shared LoadedMod.
#[derive(Clone)]
pub struct ModEntry(pub LoadedMod);

impl ModEntry {
    pub fn has_update(&self) -> bool {
        if let (Some(inst), Some(tag)) = (&self.0.installed, &self.0.latest_tag) {
            let installed_ver = normalize_version(&inst.version);
            let latest_ver = normalize_version(tag);

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
            .map(normalize_version)
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


    fn also_installs_html(&self) -> String {
        let mod_deps: Vec<String> = self.0.manifest.dependencies.iter()
            .filter(|d| d.dep_type == "mod")
            .map(|d| html_escape(&d.name))
            .collect();
        if mod_deps.is_empty() {
            return String::new();
        }
        format!(r#"<p class="deps">Also installs: {}</p>"#, mod_deps.join(", "))
    }

    fn details_html(&self) -> String {
        let m = &self.0.manifest;
        let loader = &m.loader;

        let patch_deps: Vec<String> = m.dependencies.iter()
            .filter(|d| d.dep_type == "patch")
            .map(|d| html_escape(&d.name))
            .collect();

        let mut details_lines = Vec::new();
        details_lines.push(format!("Loader: {}", html_escape(loader.name())));
        if let Some(ver) = loader.version() {
            details_lines.push(format!("Loader version: {}", html_escape(ver)));
        }
        if !patch_deps.is_empty() {
            details_lines.push(format!("Also installs patches: {}", patch_deps.join(", ")));
        }

        // Only show details section if there's something beyond just "none" loader with no patches
        if loader.name() == "none" && patch_deps.is_empty() {
            return String::new();
        }

        let inner = details_lines.join("<br>");
        format!(
            r#"<details>
  <summary>Technical details</summary>
  <p>{inner}</p>
</details>"#
        )
    }

    pub fn detail_html(&self) -> String {
        let m = &self.0.manifest;
        let version = self.version_display();
        let also_installs = self.also_installs_html();
        let details = self.details_html();

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<style>
  body {{
    font-family: "Segoe UI", sans-serif;
    font-size: 14px;
    margin: 12px;
    color: #222;
    line-height: 1.6;
  }}
  h1 {{
    font-size: 1.4em;
    margin: 0 0 8px 0;
  }}
  .meta {{
    margin: 8px 0;
    color: #444;
  }}
  .deps {{
    margin-top: 12px;
    font-style: italic;
  }}
  details {{
    margin-top: 16px;
    color: #555;
  }}
  summary {{
    cursor: pointer;
  }}
</style>
</head>
<body>
  <h1>{name}</h1>
  <p>{description}</p>
  <p class="meta">For {game}, by {author}. {version}.</p>
  {also_installs}
  {details}
</body>
</html>"#,
            name = html_escape(&m.name),
            description = html_escape(&m.description),
            game = html_escape(&m.game.name),
            author = html_escape(&m.author),
            version = html_escape(&version),
        )
    }
}

/// Detail panel with a WebView for rich HTML content and an action button.
#[derive(Copy, Clone)]
pub struct DetailPanel {
    pub webview: WebView,
    pub action_btn: Button,
}

impl DetailPanel {
    pub fn build(parent: &Panel, empty_message: &str) -> Self {
        let webview = WebView::builder(parent).build();
        let action_btn = Button::builder(parent).with_label("Install").build();
        action_btn.enable(false);

        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(&webview, 1, SizerFlag::Expand | SizerFlag::All, 4);
        sizer.add(&action_btn, 0, SizerFlag::All, 8);
        parent.set_sizer(sizer, true);

        let panel = Self { webview, action_btn };
        panel.show_empty(empty_message);
        panel
    }

    pub fn show_empty(&self, message: &str) {
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8">
<style>
  body {{
    font-family: "Segoe UI", sans-serif;
    font-size: 14px;
    margin: 24px;
    color: #666;
  }}
</style>
</head>
<body>
  <p>{}</p>
</body>
</html>"#,
            html_escape(message)
        );
        self.webview.set_page(&html, "about:blank");
    }

    pub fn populate(&self, entry: &ModEntry) {
        self.webview.set_page(&entry.detail_html(), "about:blank");
        self.action_btn.set_label(entry.action_label());
        self.action_btn.enable(true);
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
