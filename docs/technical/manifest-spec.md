# AccessForge Manifest Spec v1

## Overview

AccessForge is a platform for distributing and installing accessibility mods for mainstream games, primarily screen reader mods for blind players. The manifest file (`accessforge.yml`) lives in the root of a mod's source (GitHub repo or HTTP directory). Installable files come from GitHub Releases or direct HTTP downloads.

## Design Principles

- Mod authors keep full ownership of their repos (no transfers)
- Minimize required fields, let the platform infer what it can
- No paths in the manifest — the platform knows where each loader installs
- Pinned dependency versions, no auto-upgrading — mod author controls when updates go out
- YAML format chosen for familiarity (blind devs, AI-assisted development, screen reader friendly)

## Source Types

The platform supports two source types for mods and dependencies:

- `github:owner/repo` — GitHub-hosted. Releases come from GitHub Releases API. Manifest fetched from the repo's default branch.
- `url:https://example.com/path` — HTTP-hosted. The manifest lives at `{url}/accessforge.yml`. Release assets are downloaded relative to the URL.

## Manifest File: `accessforge.yml`

### GitHub source example

```yaml
spec: 1

id: SparkingZeroAccess
name: Sparking Zero Access
description: Screen reader accessibility mod
author: jessica
version: "1.0.0"
source: github:jessica/sparking-zero-access
license: MIT

game:
  name: Dragon Ball Sparking! ZERO
  store:
    steam: 1790600

loader:
  name: ue4ss

release:
  asset: "SparkingZeroAccess-{version}.zip"

dependencies:
  - name: UTOC Signature Bypass
    type: patch
    source: github:accessforge/utoc-bypass-sparking-zero
    asset: "utoc-bypass.zip"
    version: "1.0.0"
```

### HTTP source example

```yaml
spec: 1

id: SparkingZeroAccess
name: Sparking Zero Access
description: Screen reader accessibility mod
author: jessica
version: "1.0.0a1"
source: "url:https://example.com/my-mod"
license: MIT

game:
  name: Dragon Ball Sparking! ZERO
  store:
    steam: 1790600

loader:
  name: ue4ss

release:
  asset: "SparkingZeroAccess-{version}.zip"

dependencies:
  - name: UTOC Signature Bypass
    type: patch
    source: github:accessforge/utoc-bypass-sparking-zero
    asset: "utoc-bypass.zip"
    version: "1.0.0"
```

## Field Reference

### Top-level (required)

- `spec` (integer) — Manifest spec version. Currently `1`.
- `id` (string) — Stable technical identifier for the mod. Used for folder names, state file keys, and loader registration (e.g. UE4SS mods.txt). Should not contain spaces. Never changes once published.
- `name` (string) — Human-readable display name of the mod. Shown to users in the UI. Can be changed freely without breaking installs.
- `description` (string) — Short description of what the mod does.
- `author` (string) — Mod author name.
- `version` (string) — Mod version. Must be quoted to avoid YAML float parsing. Supports semver, pre-release tags (`1.0.0-alpha.1`, `1.0.0a1`, `1.0.0-rc.1`), and dev versions.
- `source` (string) — Where the mod is hosted. Format: `github:owner/repo` or `url:https://...`.

### Top-level (optional)

- `license` (string) — License identifier (e.g. `MIT`, `GPL-3.0`).

### `game` section

- `game.name` (string) — Human-readable game name.
- `game.store` (object) — Optional. Store IDs for auto-detection.
- `game.store.steam` (integer) — Steam app ID (from the store URL).
- `game.store.gog` (integer) — GOG product ID.
- `game.store.epic` (string) — Epic Games Store ID.

If `store` is omitted or no ID matches, the platform asks the user to locate the game once and remembers the path.

### `loader` field

Tells the platform which mod loader the game uses. The platform handles loader installation and knows where mods go. Always a dict with at least `name`.

- `loader.name` (string, required) — Loader name. One of: `ue4ss`, `bepinex`, `melonloader`, `none`.
- `loader.version` (string, optional) — Pin to a specific version or partial version. If omitted, the platform installs the latest release. Partial versions are supported: `"5.4"` resolves to the latest `5.4.x` release.
- `loader.arch` (string, optional) — Architecture. `x64` (default) or `x86`. Only relevant for BepInEx and MelonLoader.

```yaml
# Latest version, default arch (x64)
loader:
  name: ue4ss

# Pin to exact version
loader:
  name: bepinex
  version: "5.4.23.5"

# Partial version — latest 5.4.x
loader:
  name: bepinex
  version: "5.4"

# Pin version and architecture
loader:
  name: melonloader
  version: "0.7"
  arch: x86
```

Supported loaders and platform behavior:

- `ue4ss` — For Unreal Engine 4/5 games. Platform finds `*/Binaries/Win64/` in game dir (always one level deep). Installs UE4SS there. Mods go in `{bin_path}/Mods/`. x64 only.
- `bepinex` — For Unity (Mono) games. Installs to game root. Mods go in `BepInEx/plugins/`. Supports x64 and x86.
- `melonloader` — For Unity (Mono + IL2CPP) games. Installs to game root. Mods go in `Mods/`. Supports x64 and x86.
- `none` — No mod loader. Release zip is extracted directly to game root. Zip must mirror game folder structure.

### `release` section

- `release.asset` (string) — Filename template for the release asset to download. Use `{version}` as an optional placeholder for the version number. Examples: `"MyMod-{version}.zip"`, `"MyMod.zip"`.

### Asset resolution by source type

**GitHub source:**
1. Read `version` from the manifest
2. Find the GitHub Release tagged with that version (tries `v{version}` and `{version}`)
3. In that release, find the asset matching `release.asset` (after `{version}` substitution)

**HTTP source:**
1. Substitute `{version}` in the asset template with the manifest's `version`
2. Download from `{source_url}/{resolved_asset}`

Example: `source: url:https://example.com/my-mod`, `version: "1.0.0a1"`, `asset: "MyMod-{version}.zip"` resolves to `https://example.com/my-mod/MyMod-1.0.0a1.zip`.

### `dependencies` section

List of external dependencies the mod needs.

- `name` (string) — Human-readable dependency name.
- `type` (string) — `patch` or `mod`. See below.
- `source` (string) — Source reference. Format: `github:owner/repo` or `url:https://...`.
- `asset` (string) — Filename template for the release asset. Supports `{version}` placeholder.
- `version` (string) — Pinned version. Must be quoted.

Dependency types:

- `patch` — Extracts to the exe directory. For UE games, that's `*/Binaries/Win64/`. For Unity/none games, that's game root.
- `mod` — Extracts to where the loader puts mods (same location as the main mod).

Dependencies use the same asset resolution as the main mod, using the dependency's own `source` and `version` fields.

## Game Path Auto-Detection

Resolution order:
1. Check saved paths (user already pointed to it before)
2. Try provider lookup from store IDs in manifest
3. Ask the user once, save for next time

### Steam detection
Platform reads `libraryfolders.vdf` to find all Steam library paths, then checks `appmanifest_{id}.acf` for the install folder.

### UE game exe detection
For `loader:
  name: ue4ss`, the platform scans for `*/Binaries/Win64/` inside the game root. All UE4/UE5 games use this convention (one folder deep):
- Sparking Zero: `SparkingZERO/Binaries/Win64/`
- Hogwarts Legacy: `Phoenix/Binaries/Win64/`
- Palworld: `Pal/Binaries/Win64/`
- Tekken 8: `Polaris/Binaries/Win64/`

## Update Flow

1. Fetch the latest manifest from the source (default branch for GitHub, `{url}/accessforge.yml` for HTTP)
2. Compare the manifest's `version` against the installed version
3. If newer, download from the release/URL for that version and install

## Validation Rules

- `version` fields must be strings (warn if YAML parsed as float)
- `source` must be `github:owner/repo` or `url:https://...` format
- `loader` must be one of: `ue4ss`, `bepinex`, `melonloader`, `none`
- `type` on dependencies must be `patch` or `mod`
- `spec` must be a version the platform understands
