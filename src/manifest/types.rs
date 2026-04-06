use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub name: String,
    #[serde(default)]
    pub store: Option<Store>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub steam: Option<u64>,
    #[serde(default)]
    pub gog: Option<u64>,
    #[serde(default)]
    pub epic: Option<String>,
}

/// Loader configuration — always a dict with at least `name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderField {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

impl LoaderField {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn arch(&self) -> &str {
        self.arch.as_deref().unwrap_or("x64")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Ue4ss,
    BepInEx,
    MelonLoader,
    None,
}

impl LoaderKind {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "ue4ss" => Ok(Self::Ue4ss),
            "bepinex" => Ok(Self::BepInEx),
            "melonloader" => Ok(Self::MelonLoader),
            "none" => Ok(Self::None),
            other => bail!("unsupported loader: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub asset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(rename = "type")]
    pub dep_type: String,
    pub source: String,
    pub asset: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepType {
    Patch,
    Mod,
}

impl DepType {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "patch" => Ok(Self::Patch),
            "mod" => Ok(Self::Mod),
            other => bail!("unsupported dependency type: {other}"),
        }
    }
}

/// Parsed source reference — either a GitHub repo or an HTTP URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    GitHub { owner: String, repo: String },
    Url { url: String },
}

impl Source {
    /// Parse a source string: `github:owner/repo` or `url:https://...`
    pub fn parse(s: &str) -> Result<Self> {
        if let Some(rest) = s.strip_prefix("github:") {
            let (owner, repo) = rest
                .split_once('/')
                .with_context(|| format!("source must be 'github:owner/repo', got '{s}'"))?;
            if owner.is_empty() || repo.is_empty() {
                bail!("owner and repo must not be empty in '{s}'");
            }
            Ok(Source::GitHub {
                owner: owner.to_string(),
                repo: repo.to_string(),
            })
        } else if let Some(rest) = s.strip_prefix("url:") {
            if !rest.starts_with("http://") && !rest.starts_with("https://") {
                bail!("url source must start with http:// or https://, got '{s}'");
            }
            Ok(Source::Url {
                url: rest.to_string(),
            })
        } else {
            bail!("source must start with 'github:' or 'url:', got '{s}'");
        }
    }

    /// Get GitHub owner and repo, if this is a GitHub source.
    pub fn as_github(&self) -> Option<(&str, &str)> {
        match self {
            Source::GitHub { owner, repo } => Some((owner, repo)),
            Source::Url { .. } => None,
        }
    }

    /// Get the URL, if this is an HTTP source.
    pub fn as_url(&self) -> Option<&str> {
        match self {
            Source::Url { url } => Some(url),
            Source::GitHub { .. } => None,
        }
    }

    /// Parse user-friendly input into a Source.
    /// Detects GitHub URLs automatically:
    /// - "https://github.com/owner/repo" → Source::GitHub { owner, repo }
    /// - "https://github.com/owner/repo/anything/else" → Source::GitHub { owner, repo }
    /// - "https://example.com/my-mod" → Source::Url { url }
    /// - "github:owner/repo" → Source::GitHub { owner, repo }
    /// - "url:https://..." → Source::Url { url }
    pub fn parse_user_input(input: &str) -> Result<Self> {
        // Already has a prefix — use Source::parse directly
        if input.starts_with("github:") || input.starts_with("url:") {
            return Source::parse(input);
        }

        // GitHub URL detection
        if let Some(path) = input
            .strip_prefix("https://github.com/")
            .or_else(|| input.strip_prefix("http://github.com/"))
        {
            let path = path.trim_end_matches('/');
            let parts: Vec<&str> = path.splitn(3, '/').collect();
            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Source::parse(&format!("github:{}/{}", parts[0], parts[1]));
            }
            bail!("could not parse GitHub URL: expected https://github.com/owner/repo");
        }

        // Any other URL — treat as HTTP source
        if input.starts_with("http://") || input.starts_with("https://") {
            return Source::parse(&format!("url:{input}"));
        }

        bail!("expected a URL (https://github.com/owner/repo or https://example.com/my-mod)");
    }
}

/// Substitute `{version}` in an asset template.
pub fn resolve_asset(asset_template: &str, version: &str) -> String {
    asset_template.replace("{version}", version)
}
