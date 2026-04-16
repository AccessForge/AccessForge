use anyhow::{Context, Result};
use crate::registry::http_agent;

const GITHUB_API: &str = "https://api.github.com";
const TOPIC: &str = "accessforge-mod";

/// A discovered mod repository from GitHub topic search.
#[derive(Debug, Clone)]
pub struct DiscoveredMod {
    pub owner: String,
    pub repo: String,
}

/// Search GitHub for repos tagged with the accessforge-mod topic.
pub fn discover_mods() -> Result<Vec<DiscoveredMod>> {
    let url = format!(
        "{GITHUB_API}/search/repositories?q=topic:{TOPIC}&sort=updated&per_page=100"
    );

    let response: serde_json::Value = http_agent()
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "AccessForge")
        .call()
        .context("failed to search GitHub for mods")?
        .body_mut()
        .read_json()
        .context("failed to parse GitHub search response")?;

    let items = response["items"]
        .as_array()
        .context("unexpected GitHub search response format")?;

    let mods = items
        .iter()
        .filter_map(|item| {
            let full_name = item["full_name"].as_str()?;
            let (owner, repo) = full_name.split_once('/')?;
            Some(DiscoveredMod {
                owner: owner.to_string(),
                repo: repo.to_string(),
            })
        })
        .collect();

    Ok(mods)
}
