# AccessForge Progress Report

## Status: v1 Ready

All core features for end users and mod developers are implemented. The app can discover, install, update, and manage accessibility mods for mainstream games.

## Completed Features

### Core Infrastructure
- Cargo project with Rust 2024 edition
- Release profile optimized (strip, lto, opt-level z, panic abort)
- Windows application manifest embedded (Common Controls v6, DPI awareness, UTF-8)

### Manifest System (`src/manifest/`)
- YAML manifest parsing and validation (`accessforge.yml`)
- `Source` enum: `github:owner/repo` and `url:https://...` sources
- `Source::parse_user_input()` — smart parsing of GitHub URLs, raw URLs, and prefixed sources
- `{version}` template substitution in asset filenames
- `id` field (stable technical identifier) separate from `name` (display name)
- Loader field as dict with `name`, optional `version` (exact or partial), optional `arch`
- Dependency support with `patch` and `mod` types
- Version parsing via `versions` crate (semver, pre-release, dev versions)

### State Management (`src/state/`)
- `state.json` at `%LOCALAPPDATA%/AccessForge/`
- Safe write-then-rename pattern with `.backup` recovery
- Schema versioning for future migrations
- Game path and mod install tracking
- `last_update_check` timestamp for daily app update checks
- `manifest_cache` for ETag-based conditional manifest fetching

### Steam Integration (`src/steam/`)
- Game path auto-detection via `steamlocate` crate
- Game name + path lookup by app ID (`find_game_info`)
- UE `*/Binaries/Win64/` directory scanning (skips `Engine/` directory, case-insensitive)

### Mod Discovery (`src/registry/`)
- GitHub topic-based discovery (`accessforge-mod` topic)
- Manifest fetching from GitHub (raw content) and HTTP sources
- ETag-based conditional fetching (304 Not Modified support)
- Release fetching by version tag (tries `v{version}` then `{version}`)
- Release listing for partial version matching
- Offline fallback — shows cached manifests when GitHub is unreachable

### Loader System (`src/installer/`)
- Data-driven loader definitions (`LoaderDef` structs)
- Supported loaders: UE4SS, BepInEx, MelonLoader, none
- Loader download and extraction
- Partial version resolution (e.g. `"5.4"` matches latest `5.4.x`)
- Architecture support (x64 default, x86 optional)
- Post-install hooks (UE4SS `mods.txt` enable, prepended not appended)
- Mod and patch install path resolution per loader

### Worker Thread System (`src/worker/`)
- Background threads for network/IO operations
- `mpsc` channel-based progress reporting
- Timer-based UI polling (50ms)
- Shared between GUI and CLI
- Discovery worker with incremental mod loading and manifest caching
- Install worker with per-step progress and PostInstall step

### Self-Update System (`src/updater.rs`)
- Version check against `AccessForge/AccessForge` GitHub releases
- Compares `CARGO_PKG_VERSION` with latest release tag
- Download + rename-swap update (current→.old, new→current)
- Auto-restart after successful update
- Cleanup of `.old` file on startup
- Daily auto-check (timestamp in state.json), manual check bypasses cooldown

### GUI (`src/ui/`)
- wxDragon (wxWidgets) accessible GUI
- Four tabs: Browse, Installed, Updates, About
- Reusable tab builder (`tabs.rs`) — eliminates tab construction duplication
- SplitterWindow: list on left, detail panel on right (hidden until selection)
- WebView detail panel with HTML (mod name, description, "For [game], by [author]", version)
- "Also installs" for mod-type dependencies
- Collapsible "Technical details" for loader info and patch dependencies
- Screen reader labels above each list
- Detail panel hidden when no mod selected
- Install/Reinstall/Update buttons (disabled until selection, only disabled during install)
- Install log dialog (`install_dialog.rs`) with listbox log + gauge progress bar
  - Per-step log messages (installing loader, dependency, mod, post-install, saving)
  - Gauge increments per step for screen reader progress announcements
  - Error bell on failure
  - Close button enabled when done/failed
  - Modal dialog — blocks caller, returns install result
- Mod moves between tabs after install (Browse/Updates → Installed)
- "Add mod manually" button with custom dialog (`add_mod_dialog.rs`)
- Auto-detects GitHub URLs (https://github.com/owner/repo) and HTTP sources
- Error dialogs for invalid sources, failed fetches, game not found
- Directory picker dialog with DirPickerCtrl when game path can't be auto-detected
  - Validates UE4SS games have `*/Binaries/Win64/` folder structure
  - Validates Unity games (BepInEx/MelonLoader) have a `.exe` in root
  - Saves selected path to state.json for future installs
- About tab with version label, "Check for updates" button, GitHub link, "Add to PATH" button
- Mock data mode (`--mock` flag) for UI testing

### CLI (`src/cli/`, powered by clap)
- All commands default to current working directory if `--from` is omitted
- Argument parsing via `clap` with derive API (colored help, version flag, subcommand validation)
- `init [--from <path>]` — interactive manifest scaffold
  - Auto-detects git remote (source, author), git user.name (author)
  - Prompts: store → steam ID or game path → game name → loader → mod name → description → author → source
  - Smart defaults derived from game name (e.g. "GameName Access", "Screen reader accessibility mod for GameName")
  - Steam app ID lookup with confirmation
  - Inline validation at each step (retries on invalid input)
  - YAML-safe quoting for special characters (colons, etc.)
- `install [--from <path|url>]` — test full install from local folder or HTTP source
  - Runs post-install hooks (e.g. UE4SS mods.txt)
  - Saves state to state.json (GUI shows mod as installed)
  - Local installs record `local_path` in state
- `package [--from <path>]` — build release zip
  - Uses `dist/` folder if present, otherwise project root
  - Excludes manifest, dotfiles, .git, .zip files
  - UE4SS mods: wraps in `Scripts/` prefix (unless already present)
  - Zip named from `release.asset` with `{version}` substituted
  - Post-package validation (UE4SS: Scripts/, BepInEx/MelonLoader: .dll, none: warns if only 1 file)
- `setup-path` — add AccessForge to user PATH (HKCU registry, no admin)

### PATH Setup (`src/path_setup.rs`)
- Adds exe directory to user PATH via HKCU registry
- Checks if already on PATH before adding
- Broadcasts environment change to running programs
- Available via CLI (`setup-path`) and GUI (About tab button)

### Documentation
- `docs/getting-started.md` — tutorial for mod authors
- `docs/technical/manifest-spec.md` — full manifest field reference
- `docs/technical/state-file-spec.md` — state file schema
- `docs/technical/design-notes.md` — architecture and decisions
- `CLAUDE.md` — project instructions for AI assistants

### Tests
- 16 passing tests covering:
  - Manifest parsing (valid, invalid, GitHub source, HTTP source, backward compat)
  - Source parsing (GitHub, URL, invalid)
  - Asset template resolution
  - Slugify
  - State file save/load
  - Loader version resolution (latest tag, partial match, false prefix rejection)
  - UE4SS mods.txt (new file, enable disabled, already enabled)

## Future (v2+)
- Local dependency overrides (`--local utoc-bypass=../path` flag on install, managed via `accessforge.dev.yml`, gitignored, explicit feedback)
- Loader architecture validation (reject `arch: x86` for UE4SS which only supports x64)
- Uninstall flow
- Shared dependency reference counting
- Game version compatibility field
- Tags/categories for browsing
- `validate` command (manifest check without installing)
- `watch` command (auto-copy mod files on change)
- Parallel manifest fetching during discovery
- Download progress reporting (real byte-level progress in gauge)

## Open Questions
- What happens when two mods share a loader and one is uninstalled?
- Optional dependencies?
- Per-game marker files in game directory?
- Multi-author support in manifest?
