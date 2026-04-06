# AccessForge Design Notes

## What is AccessForge?

A platform for distributing and installing accessibility mods (primarily screen reader mods for blind players) for mainstream games. The goal is 1-click install and easy updates.

## Philosophy

- Mod authors keep their GitHub repos — no repo transfers, no gatekeeping
- Add an `accessforge.yml` to your repo root, platform handles the rest
- Compete with others by being easier for both mod authors and players

## How it works

1. The platform discovers mod repos via GitHub topic search (`accessforge-mod`)
2. Each mod repo has an `accessforge.yml` manifest in its root
3. The AccessForge client fetches manifests (with ETag caching for performance), shows available mods in a tabbed UI (Browse, Installed, Updates, About)
4. Player clicks "Install" — platform resolves game path, installs loader + dependencies + mod
5. Platform checks for updates by comparing manifest version against installed version
6. If GitHub is unreachable, cached manifests are shown (offline mode)

## Source types

Mods can be hosted in two ways:

- `github:owner/repo` — GitHub-hosted. Releases come from GitHub Releases API. Manifest fetched from the repo's default branch.
- `url:https://example.com/path` — HTTP-hosted. For pre-release testing with friends before publishing to GitHub. Manifest lives at `{url}/accessforge.yml`. Release assets are downloaded relative to the URL.

## Manifest identity

- `id` — stable technical identifier, never changes. Used for folder names, state keys, loader registration (e.g. UE4SS mods.txt).
- `name` — display name shown to users. Can be changed freely (fix typos, rebranding) without breaking installs.

## Loader support

The platform knows how to install and manage these mod loaders. Loader knowledge is baked into the app as data-driven configurations.

- **UE4SS** — for Unreal Engine 4/5 games. Auto-finds `*/Binaries/Win64/` (always one level deep). Source: `github:UE4SS-RE/RE-UE4SS`. Mods go in `{bin_path}/Mods/`. Requires enabling in `mods.txt` (handled automatically by the platform).
- **BepInEx** — for Unity (Mono) games. Installs to game root. Source: `github:BepInEx/BepInEx`. Mods go in `BepInEx/plugins/`. Drop-in, auto-loads.
- **MelonLoader** — for Unity (Mono + IL2CPP) games. Installs to game root. Source: `github:LavaGang/MelonLoader`. Mods go in `Mods/`. Drop-in, auto-loads.
- **none** — no loader needed. Release zip extracts directly to game root.

Loader version can be pinned (exact or partial: `"5.4"` resolves to latest `5.4.x`). Architecture defaults to x64, with optional `arch: x86` for rare 32-bit games.

## Asset resolution

`release.asset` is a filename template. `{version}` is an optional placeholder substituted at resolution time.

- **GitHub source:** Find the release tagged with the manifest version (tries `v{version}` then `{version}`), match the resolved asset name against release assets.
- **HTTP source:** Substitute version, download from `{source_url}/{resolved_asset}`.

## Game path detection

- Steam: read `libraryfolders.vdf` + `appmanifest_{id}.acf` via `steamlocate` crate. Also retrieves the game name.
- Non-Steam: user is prompted with a directory picker dialog (DirPickerCtrl). The path is validated based on loader type (UE4SS checks for `*/Binaries/Win64/`, BepInEx/MelonLoader checks for `.exe` in root).
- Saved paths: once a game is located, the path is stored in `state.json` and reused.
- Future: GOG (registry), Epic (`LauncherInstalled.dat`), Battle.net (registry)

## Manifest caching

On discovery, manifests are fetched with ETag-based conditional requests. If the manifest hasn't changed since last fetch, GitHub returns 304 Not Modified (no body, faster, lighter on rate limits). Cached manifests are stored in `state.json`. If GitHub is unreachable, cached data is shown as an offline fallback.

## Self-update

AccessForge checks for updates to itself:
- Daily auto-check after mod discovery completes (timestamp stored in `state.json`)
- Manual check via "Check for updates" button in the About tab
- Downloads new exe, renames current to `.old`, renames new to current, auto-restarts
- Cleans up `.old` on next startup
- Update source: `AccessForge/AccessForge` GitHub releases

## Dependency hosting

For dependencies not on GitHub (e.g. Nexus Mods-only tools), maintain a mirror repo:
- `accessforge/mod-dependencies` with releases for mirrored tools
- UTOC Signature Bypass updates rarely (~2 times in 8 months), safe to mirror
- Avoids Nexus Mods login requirement, keeps 1-click experience

Dependencies support both `github:` and `url:` sources. Dependency type (`patch` or `mod`) determines where files are extracted.

## Known accessibility mods in the wild

Steam games:
- Stardew Valley — Stardew Access (also GOG)
- Hades / Hades II — Blind Accessibility
- Factorio — Factorio Access
- CrossCode — CrossedEyes
- A Space For The Unbound — accessibility mod
- Balatro — accessibility mod
- Yu-Gi-Oh Master Duel — Blind Mode
- DDLC Plus — DDLCPlusAccess (MelonLoader)
- Dragon Ball Sparking! ZERO — Sparking Zero Access (UE4SS)

Non-Steam:
- Hearthstone — Hearthstone Access (Battle.net)
- Minecraft Java — vi-access / AudioAccess (Mojang launcher)

## Version handling

Uses the `versions` crate for parsing and comparison. Supports:
- Standard semver: `1.0.0`
- Pre-release: `1.0.0-alpha.1`, `1.0.0a1`, `1.0.0-rc.1`
- Dev versions, commit hashes, arbitrary strings
- Partial version matching for loaders: `"5.4"` matches latest `5.4.x.x`

## Developer workflow

### Commands

All commands default to the current working directory if `--from` is omitted.

**`accessforge init`** — interactive manifest creation. Auto-detects git remote, git user.name, Steam game info. Validates input at each step.

**`accessforge install`** — reads manifest from local folder, installs dependencies from remote sources, copies mod files from local folder, runs post-install hooks, saves state. Tests full install flow without publishing.

**`accessforge install --from <url>`** — reads manifest from HTTP URL, downloads and installs everything from remote. For sharing pre-release builds with testers.

**`accessforge package`** — builds a release zip. Validates the zip structure against the loader type (UE4SS expects `Scripts/`, BepInEx/MelonLoader expects `.dll`).

**`accessforge setup-path`** — adds AccessForge to the user's PATH (HKCU registry, no admin needed).

### Example workflow (Sparking Zero Access)

1. **Set up project:**
   ```
   mkdir sparking-zero-access && cd sparking-zero-access
   git init
   accessforge init
   ```

2. **Develop and test:**
   ```
   accessforge install
   ```
   Full install flow: UE4SS + UTOC bypass + mod files. Verifies a player would get a working setup.

3. **Share with testers:**
   ```
   accessforge install --from "https://example.com/my-mod"
   ```
   Fetches manifest and mod files from HTTP source. Tester gets the same install experience.

4. **Package and publish:**
   ```
   accessforge package
   ```
   Upload to GitHub, tag a release. Players open AccessForge client, see the mod, click Install.

## Architecture

- **GUI framework:** wxDragon (wxWidgets bindings for Rust)
- **CLI framework:** clap (derive API)
- **HTTP client:** ureq (sync, no async runtime)
- **Background work:** std::thread + mpsc channels, Timer-based polling in UI
- **Detail panel:** WebView with HTML for rich, accessible content
- **Install progress:** Modal dialog with listbox log + gauge, error bell on failure

## Tech stack

- Manifest format: YAML (`accessforge.yml`)
- State file format: JSON (`%LOCALAPPDATA%/AccessForge/state.json`)
- Version parsing: `versions` crate
- Steam detection: `steamlocate` crate
- Windows manifest: `embed-manifest` crate (Common Controls v6, DPI awareness)

## Open questions for v2+

- Shared dependency reference counting — what happens when two mods share UE4SS and one is uninstalled?
- Game version compatibility field in manifest?
- Optional dependencies?
- Per-game marker files in game directory for cross-referencing state?
- Multi-author support in manifest?
- Tags/categories for browsing?
- Uninstall flow
- Watch command for live development (auto-copy on file changes)
