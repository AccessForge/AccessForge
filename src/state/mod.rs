use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub schema_version: u32,
    #[serde(default)]
    pub games: HashMap<String, GameState>,
    #[serde(default)]
    pub last_update_check: Option<String>,
    #[serde(default)]
    pub manifest_cache: HashMap<String, CachedManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedManifest {
    #[serde(default)]
    pub etag: Option<String>,
    pub yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub mods: HashMap<String, ModState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModState {
    pub name: String,
    pub source: String,
    pub version: String,
    pub installed_at: String,
    pub loader: String,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            games: HashMap::new(),
            last_update_check: None,
            manifest_cache: HashMap::new(),
        }
    }

    /// Get the state file path (%LOCALAPPDATA%/AccessForge/state.json).
    pub fn state_path() -> Result<PathBuf> {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .context("LOCALAPPDATA environment variable not set")?;
        let dir = PathBuf::from(local_app_data).join("AccessForge");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
        Ok(dir.join("state.json"))
    }

    /// Load state from the default location. Falls back to backup, then fresh state.
    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;
        Self::load_from(&path)
    }

    /// Load state from a specific path with backup fallback.
    pub fn load_from(path: &Path) -> Result<Self> {
        // Try primary file
        if path.exists() {
            match Self::read_and_validate(path) {
                Ok(state) => return Ok(state),
                Err(e) => {
                    eprintln!("Warning: state.json is corrupt ({e}), trying backup...");
                }
            }
        }

        // Try backup
        let backup = path.with_extension("json.backup");
        if backup.exists() {
            match Self::read_and_validate(&backup) {
                Ok(state) => {
                    eprintln!("Recovered state from backup.");
                    return Ok(state);
                }
                Err(e) => {
                    eprintln!("Warning: backup is also corrupt ({e}), starting fresh.");
                }
            }
        }

        Ok(Self::new())
    }

    fn read_and_validate(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let state: Self = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if state.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported schema version {} (expected {SCHEMA_VERSION})",
                state.schema_version
            );
        }
        Ok(state)
    }

    /// Save state using write-then-rename pattern.
    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;
        self.save_to(&path)
    }

    /// Save state to a specific path using write-then-rename for safety.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        let backup = path.with_extension("json.backup");

        // Write to temp file
        let json = serde_json::to_string_pretty(self).context("failed to serialize state")?;
        std::fs::write(&tmp, &json)
            .with_context(|| format!("failed to write {}", tmp.display()))?;

        // Copy current to backup (if it exists)
        if path.exists() {
            std::fs::copy(path, &backup)
                .with_context(|| format!("failed to create backup at {}", backup.display()))?;
        }

        // Rename temp to primary (atomic on NTFS)
        std::fs::rename(&tmp, path)
            .with_context(|| format!("failed to rename temp to {}", path.display()))?;

        Ok(())
    }

    /// Get or create a game entry.
    pub fn get_or_create_game(&mut self, slug: &str, name: &str, path: &str) -> &mut GameState {
        self.games.entry(slug.to_string()).or_insert_with(|| GameState {
            name: name.to_string(),
            path: path.to_string(),
            mods: HashMap::new(),
        })
    }

    /// Check if a mod is installed for a game.
    pub fn installed_mod(&self, game_slug: &str, mod_slug: &str) -> Option<&ModState> {
        self.games.get(game_slug)?.mods.get(mod_slug)
    }

    /// Whether we should check for app updates (true if never checked or >24h ago).
    pub fn should_check_updates(&self) -> bool {
        let Some(last) = &self.last_update_check else {
            return true;
        };
        let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(last) else {
            return true;
        };
        let elapsed = chrono::Utc::now().signed_duration_since(last_time);
        elapsed.num_hours() >= 24
    }

    /// Record that we just checked for updates.
    pub fn mark_update_checked(&mut self) {
        self.last_update_check = Some(chrono::Utc::now().to_rfc3339());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new_state() {
        let state = AppState::new();
        assert_eq!(state.schema_version, 1);
        assert!(state.games.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let dir = std::env::temp_dir().join("accessforge_test_state");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");

        let mut state = AppState::new();
        state.games.insert(
            "test-game".to_string(),
            GameState {
                name: "Test Game".to_string(),
                path: "C:/Games/Test".to_string(),
                mods: HashMap::new(),
            },
        );

        state.save_to(&path).unwrap();
        let loaded = AppState::load_from(&path).unwrap();
        assert_eq!(loaded.games.len(), 1);
        assert!(loaded.games.contains_key("test-game"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backup_recovery() {
        let dir = std::env::temp_dir().join("accessforge_test_backup_recovery");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");

        // Save a valid state
        let mut state = AppState::new();
        state.games.insert(
            "my-game".to_string(),
            GameState {
                name: "My Game".to_string(),
                path: "C:/Games/MyGame".to_string(),
                mods: HashMap::new(),
            },
        );
        state.save_to(&path).unwrap();

        // Save again so the first save becomes the backup
        state.save_to(&path).unwrap();

        // Corrupt the primary file
        std::fs::write(&path, "not valid json!!!").unwrap();

        // Load should recover from backup
        let loaded = AppState::load_from(&path).unwrap();
        assert_eq!(loaded.games.len(), 1);
        assert!(loaded.games.contains_key("my-game"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_both_corrupt_starts_fresh() {
        let dir = std::env::temp_dir().join("accessforge_test_both_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");

        std::fs::write(&path, "corrupt").unwrap();
        std::fs::write(path.with_extension("json.backup"), "also corrupt").unwrap();

        let loaded = AppState::load_from(&path).unwrap();
        assert!(loaded.games.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_check_updates_never_checked() {
        let state = AppState::new();
        assert!(state.should_check_updates());
    }

    #[test]
    fn test_should_check_updates_recently_checked() {
        let mut state = AppState::new();
        state.mark_update_checked();
        assert!(!state.should_check_updates());
    }

    #[test]
    fn test_should_check_updates_old_check() {
        let mut state = AppState::new();
        // Set to 25 hours ago
        let old_time = chrono::Utc::now() - chrono::Duration::hours(25);
        state.last_update_check = Some(old_time.to_rfc3339());
        assert!(state.should_check_updates());
    }

    #[test]
    fn test_manifest_cache_round_trip() {
        let dir = std::env::temp_dir().join("accessforge_test_cache_rt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");

        let mut state = AppState::new();
        state.manifest_cache.insert(
            "owner/repo".to_string(),
            CachedManifest {
                etag: Some("\"abc123\"".to_string()),
                yaml: "spec: 1\nid: test".to_string(),
            },
        );

        state.save_to(&path).unwrap();
        let loaded = AppState::load_from(&path).unwrap();

        let cached = loaded.manifest_cache.get("owner/repo").unwrap();
        assert_eq!(cached.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(cached.yaml, "spec: 1\nid: test");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_installed_mod_lookup() {
        let mut state = AppState::new();
        let game = state.get_or_create_game("my-game", "My Game", "C:/Games");
        game.mods.insert(
            "my-mod".to_string(),
            ModState {
                name: "My Mod".to_string(),
                source: "github:test/test".to_string(),
                version: "1.0.0".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                loader: "none".to_string(),
                local_path: None,
                dependencies: HashMap::new(),
            },
        );

        assert!(state.installed_mod("my-game", "my-mod").is_some());
        assert!(state.installed_mod("my-game", "other-mod").is_none());
        assert!(state.installed_mod("other-game", "my-mod").is_none());
    }
}
