use anyhow::{Context, Result};
use crate::manifest::{Source, resolve_asset};
use crate::registry::http_agent;

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub tag: String,
    pub download_url: String,
}

/// Fetch a release asset based on source type.
pub fn fetch_release_for_source(
    source: &Source,
    asset_template: &str,
    version: &str,
) -> Result<Option<ReleaseAsset>> {
    match source {
        Source::GitHub { owner, repo } => {
            fetch_github_release_by_tag(owner, repo, asset_template, version)
        }
        Source::Url { url } => {
            let asset_name = resolve_asset(asset_template, version);
            let download_url = format!(
                "{}/{}",
                url.trim_end_matches('/'),
                asset_name
            );
            Ok(Some(ReleaseAsset {
                tag: version.to_string(),
                download_url,
            }))
        }
    }
}

/// Find a GitHub release by tag and locate the matching asset.
/// Tries `v{version}` first, then `{version}`.
pub fn fetch_github_release_by_tag(
    owner: &str,
    repo: &str,
    asset_template: &str,
    version: &str,
) -> Result<Option<ReleaseAsset>> {
    let asset_name = resolve_asset(asset_template, version);

    let tags_to_try = [format!("v{version}"), version.to_string()];

    for tag in &tags_to_try {
        let url = format!("{GITHUB_API}/repos/{owner}/{repo}/releases/tags/{tag}");

        let response: serde_json::Value = match http_agent()
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "AccessForge")
            .call()
        {
            Ok(mut resp) => resp
                .body_mut()
                .read_json()
                .context("failed to parse release response")?,
            Err(ureq::Error::StatusCode(404)) => continue,
            Err(e) => return Err(e).context("failed to fetch release"),
        };

        let assets = match response["assets"].as_array() {
            Some(a) => a,
            None => continue,
        };

        for asset in assets {
            let name = match asset["name"].as_str() {
                Some(n) => n,
                None => continue,
            };
            if name == asset_name {
                let download_url = asset["browser_download_url"]
                    .as_str()
                    .context("asset has no download URL")?
                    .to_string();
                return Ok(Some(ReleaseAsset {
                    tag: tag.clone(),
                    download_url,
                }));
            }
        }
    }

    Ok(None)
}

/// List all release tags for a GitHub repo (non-prerelease, non-draft).
pub fn list_release_tags(owner: &str, repo: &str) -> Result<Vec<String>> {
    let url = format!("{GITHUB_API}/repos/{owner}/{repo}/releases?per_page=100");

    let response: serde_json::Value = http_agent()
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "AccessForge")
        .call()
        .context("failed to list releases")?
        .body_mut()
        .read_json()
        .context("failed to parse releases response")?;

    let releases = response
        .as_array()
        .context("unexpected releases response format")?;

    let tags: Vec<String> = releases
        .iter()
        .filter(|r| !r["prerelease"].as_bool().unwrap_or(true))
        .filter(|r| !r["draft"].as_bool().unwrap_or(true))
        .filter_map(|r| r["tag_name"].as_str().map(|s| s.to_string()))
        .collect();

    Ok(tags)
}

/// Fetch the latest release (used by loader installs which have hardcoded GitHub repos).
pub fn fetch_latest_release_asset(
    owner: &str,
    repo: &str,
    asset_name: &str,
) -> Result<Option<ReleaseAsset>> {
    let url = format!("{GITHUB_API}/repos/{owner}/{repo}/releases/latest");

    let response: serde_json::Value = match http_agent()
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "AccessForge")
        .call()
    {
        Ok(mut resp) => resp
            .body_mut()
            .read_json()
            .context("failed to parse release response")?,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(e) => return Err(e).context("failed to fetch latest release"),
    };

    let tag = response["tag_name"]
        .as_str()
        .context("release has no tag_name")?
        .to_string();

    let assets = response["assets"]
        .as_array()
        .context("release has no assets")?;

    for asset in assets {
        let name = match asset["name"].as_str() {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with(asset_name) || name == asset_name {
            let download_url = asset["browser_download_url"]
                .as_str()
                .context("asset has no download URL")?
                .to_string();
            return Ok(Some(ReleaseAsset {
                tag,
                download_url,
            }));
        }
    }

    Ok(None)
}
