mod discovery;
mod releases;

pub use discovery::*;
pub use releases::*;

use anyhow::{Context, Result};
use crate::manifest::Source;

/// Fetch a manifest based on source type.
pub fn fetch_manifest_for_source(source: &Source) -> Result<String> {
    match source {
        Source::GitHub { owner, repo } => fetch_manifest_yaml(owner, repo),
        Source::Url { url } => fetch_manifest_url(url),
    }
}

/// Fetch the raw accessforge.yml from a GitHub repo's default branch.
pub fn fetch_manifest_yaml(owner: &str, repo: &str) -> Result<String> {
    match fetch_manifest_yaml_cached(owner, repo, None)? {
        CachedResponse::Fresh { yaml, .. } => Ok(yaml),
        CachedResponse::NotModified => {
            anyhow::bail!("unexpected 304 without etag")
        }
    }
}

/// Result of a cached manifest fetch.
pub enum CachedResponse {
    /// New or updated content.
    Fresh { yaml: String, etag: Option<String> },
    /// Content hasn't changed since the provided etag.
    NotModified,
}

/// Fetch manifest with optional ETag for conditional requests.
/// If `etag` is provided and content hasn't changed, returns `NotModified`.
pub fn fetch_manifest_yaml_cached(
    owner: &str,
    repo: &str,
    etag: Option<&str>,
) -> Result<CachedResponse> {
    let url = format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/accessforge.yml"
    );

    let mut req = ureq::get(&url)
        .header("User-Agent", "AccessForge");

    if let Some(etag) = etag {
        req = req.header("If-None-Match", etag);
    }

    match req.call() {
        Ok(mut resp) => {
            let new_etag = resp
                .headers()
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let body = resp
                .body_mut()
                .read_to_string()
                .context("failed to read manifest response")?;
            Ok(CachedResponse::Fresh {
                yaml: body,
                etag: new_etag,
            })
        }
        Err(ureq::Error::StatusCode(304)) => Ok(CachedResponse::NotModified),
        Err(e) => Err(e).with_context(|| format!("failed to fetch manifest from {owner}/{repo}")),
    }
}

/// Fetch accessforge.yml from an HTTP source URL.
fn fetch_manifest_url(base_url: &str) -> Result<String> {
    let url = format!("{}/accessforge.yml", base_url.trim_end_matches('/'));

    let body = ureq::get(&url)
        .header("User-Agent", "AccessForge")
        .call()
        .with_context(|| format!("failed to fetch manifest from {url}"))?
        .body_mut()
        .read_to_string()
        .context("failed to read manifest response")?;

    Ok(body)
}
