mod types;

pub use types::*;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Manifest spec version we understand.
const SUPPORTED_SPEC: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub spec: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub license: Option<String>,
    pub game: Game,
    pub loader: LoaderField,
    pub release: Release,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Manifest {
    /// Parse a manifest from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let manifest: Manifest =
            serde_yml::from_str(yaml).context("failed to parse accessforge.yml")?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load manifest from a file path.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).context("failed to read accessforge.yml")?;
        Self::from_yaml(&content)
    }

    /// Validate manifest fields.
    pub fn validate(&self) -> Result<()> {
        if self.spec != SUPPORTED_SPEC {
            bail!(
                "unsupported manifest spec version {} (expected {SUPPORTED_SPEC})",
                self.spec
            );
        }

        if self.id.is_empty() {
            bail!("id must not be empty");
        }

        LoaderKind::from_str(self.loader.name())?;
        Source::parse(&self.source).context("invalid source field")?;

        for dep in &self.dependencies {
            DepType::from_str(&dep.dep_type)
                .with_context(|| format!("in dependency '{}'", dep.name))?;
            Source::parse(&dep.source)
                .with_context(|| format!("invalid source in dependency '{}'", dep.name))?;
        }

        Ok(())
    }

    pub fn parsed_source(&self) -> Result<Source> {
        Source::parse(&self.source)
    }

    pub fn loader_kind(&self) -> Result<LoaderKind> {
        LoaderKind::from_str(self.loader.name())
    }

    pub fn steam_id(&self) -> Option<u64> {
        self.game.store.as_ref()?.steam
    }

    pub fn slug(&self) -> &str {
        &self.id
    }

    pub fn game_slug(&self) -> String {
        slugify(&self.game.name)
    }
}

/// Strip a leading `v` prefix from a version string (e.g. `"v1.2.3"` → `"1.2.3"`).
/// Returns the string unchanged if it does not start with `v`.
pub fn normalize_version(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

/// Convert a name to a URL-safe slug.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_manifest() {
        let yaml = r#"
spec: 1
id: TestMod
name: Test Mod
description: A test mod
author: tester
version: "1.0.0"
source: github:tester/test-mod
game:
  name: Test Game
  store:
    steam: 12345
loader:
  name: ue4ss
release:
  asset: "TestMod-{version}.zip"
dependencies:
  - name: Some Patch
    type: patch
    source: github:org/some-patch
    asset: "patch-{version}.zip"
    version: "1.0.0"
"#;
        let manifest = Manifest::from_yaml(yaml).unwrap();
        assert_eq!(manifest.name, "Test Mod");
        assert_eq!(manifest.steam_id(), Some(12345));
        assert_eq!(manifest.loader_kind().unwrap(), LoaderKind::Ue4ss);
        assert_eq!(manifest.slug(), "TestMod");
    }

    #[test]
    fn test_source_parse_github() {
        let source = Source::parse("github:jessica/sparking-zero-access").unwrap();
        assert_eq!(source, Source::GitHub {
            owner: "jessica".to_string(),
            repo: "sparking-zero-access".to_string(),
        });
        assert_eq!(source.as_github(), Some(("jessica", "sparking-zero-access")));
        assert_eq!(source.as_url(), None);
    }

    #[test]
    fn test_source_parse_url() {
        let source = Source::parse("url:https://example.com/my-mod").unwrap();
        assert_eq!(source, Source::Url {
            url: "https://example.com/my-mod".to_string(),
        });
        assert_eq!(source.as_url(), Some("https://example.com/my-mod"));
        assert_eq!(source.as_github(), None);
    }

    #[test]
    fn test_source_parse_invalid() {
        assert!(Source::parse("invalid:foo/bar").is_err());
        assert!(Source::parse("github:").is_err());
        assert!(Source::parse("url:ftp://bad").is_err());
    }

    #[test]
    fn test_resolve_asset() {
        assert_eq!(resolve_asset("MyMod-{version}.zip", "1.0.0"), "MyMod-1.0.0.zip");
        assert_eq!(resolve_asset("MyMod-{version}.zip", "1.0.0a1"), "MyMod-1.0.0a1.zip");
        assert_eq!(resolve_asset("MyMod.zip", "1.0.0"), "MyMod.zip");
    }

    #[test]
    fn test_http_source_manifest() {
        let yaml = r#"
spec: 1
id: TestMod
name: Test Mod
description: HTTP hosted mod
author: tester
version: "1.0.0a1"
source: "url:https://example.com/my-mod"
game:
  name: Test Game
loader:
  name: none
release:
  asset: "TestMod-{version}.zip"
"#;
        let manifest = Manifest::from_yaml(yaml).unwrap();
        let source = manifest.parsed_source().unwrap();
        assert_eq!(source.as_url(), Some("https://example.com/my-mod"));
    }

    #[test]
    fn test_invalid_loader() {
        let yaml = r#"
spec: 1
id: BadMod
name: Bad Mod
description: Bad
author: x
version: "1.0.0"
source: github:x/y
game:
  name: Game
loader:
  name: invalid_loader
release:
  asset: "x.zip"
"#;
        assert!(Manifest::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Dragon Ball Sparking! ZERO"), "dragon-ball-sparking-zero");
        assert_eq!(slugify("Sparking Zero Access"), "sparking-zero-access");
    }

    #[test]
    fn test_parse_user_input_github_url() {
        let source = Source::parse_user_input("https://github.com/jessica/sparking-zero-access").unwrap();
        assert_eq!(source, Source::GitHub {
            owner: "jessica".to_string(),
            repo: "sparking-zero-access".to_string(),
        });
    }

    #[test]
    fn test_parse_user_input_github_url_with_path() {
        let source = Source::parse_user_input("https://github.com/jessica/sparking-zero-access/tree/main").unwrap();
        assert_eq!(source, Source::GitHub {
            owner: "jessica".to_string(),
            repo: "sparking-zero-access".to_string(),
        });
    }

    #[test]
    fn test_parse_user_input_github_url_trailing_slash() {
        let source = Source::parse_user_input("https://github.com/jessica/sparking-zero-access/").unwrap();
        assert_eq!(source, Source::GitHub {
            owner: "jessica".to_string(),
            repo: "sparking-zero-access".to_string(),
        });
    }

    #[test]
    fn test_parse_user_input_http_url() {
        let source = Source::parse_user_input("https://example.com/my-mod").unwrap();
        assert_eq!(source, Source::Url {
            url: "https://example.com/my-mod".to_string(),
        });
    }

    #[test]
    fn test_parse_user_input_prefixed() {
        let source = Source::parse_user_input("github:owner/repo").unwrap();
        assert_eq!(source, Source::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        });

        let source = Source::parse_user_input("url:https://example.com").unwrap();
        assert_eq!(source, Source::Url {
            url: "https://example.com".to_string(),
        });
    }

    #[test]
    fn test_parse_user_input_invalid() {
        assert!(Source::parse_user_input("not-a-url").is_err());
        assert!(Source::parse_user_input("ftp://example.com").is_err());
        assert!(Source::parse_user_input("").is_err());
    }

    #[test]
    fn test_manifest_with_dependencies() {
        let yaml = r#"
spec: 1
id: TestMod
name: Test Mod
description: A test mod
author: tester
version: "1.0.0"
source: github:tester/test-mod
game:
  name: Test Game
loader:
  name: ue4ss
release:
  asset: "TestMod-{version}.zip"
dependencies:
  - name: Patch One
    type: patch
    source: github:org/patch-one
    asset: "patch-{version}.zip"
    version: "1.0.0"
  - name: Mod Dep
    type: mod
    source: github:org/mod-dep
    asset: "moddep-{version}.zip"
    version: "2.0.0"
"#;
        let manifest = Manifest::from_yaml(yaml).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].dep_type, "patch");
        assert_eq!(manifest.dependencies[1].dep_type, "mod");
    }

    #[test]
    fn test_manifest_invalid_dependency_type() {
        let yaml = r#"
spec: 1
id: TestMod
name: Test Mod
description: A test mod
author: tester
version: "1.0.0"
source: github:tester/test-mod
game:
  name: Test Game
loader:
  name: none
release:
  asset: "TestMod-{version}.zip"
dependencies:
  - name: Bad Dep
    type: unknown
    source: github:org/bad
    asset: "bad.zip"
    version: "1.0.0"
"#;
        assert!(Manifest::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_manifest_quoted_values_with_colons() {
        let yaml = r#"
spec: 1
id: TestMod
name: "Test: Mod"
description: "A mod for Game: The Sequel"
author: tester
version: "1.0.0"
source: github:tester/test-mod
game:
  name: "Game: The Sequel"
loader:
  name: none
release:
  asset: "TestMod-{version}.zip"
"#;
        let manifest = Manifest::from_yaml(yaml).unwrap();
        assert_eq!(manifest.name, "Test: Mod");
        assert_eq!(manifest.game.name, "Game: The Sequel");
    }
}
