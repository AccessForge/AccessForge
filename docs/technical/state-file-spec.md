# AccessForge Local State File Spec v1

## Overview

The state file tracks what the platform has installed, cached manifest data, and app settings. It lives at `%LOCALAPPDATA%/AccessForge/state.json`. It is machine-generated — users should not need to edit it.

## Design Principles

- The state file is a record of what we did, not a guarantee that it works
- Game updates can break mods at any time — we can't detect or prevent this
- Track enough to support: install, reinstall, update, and "ask once, remember" game paths
- No file-level tracking in v1 — reinstall overwrites everything fresh
- Write-then-rename pattern to prevent corruption
- Keep one `.backup` copy automatically

## State File: `state.json`

```json
{
  "schema_version": 1,
  "last_update_check": "2026-04-02T10:00:00+00:00",
  "manifest_cache": {
    "jessica/sparking-zero-access": {
      "etag": "\"abc123\"",
      "yaml": "spec: 1\nid: sparking-zero-access\n..."
    }
  },
  "games": {
    "dragon-ball-sparking-zero": {
      "name": "Dragon Ball Sparking! ZERO",
      "path": "C:/Program Files (x86)/Steam/steamapps/common/DRAGON BALL Sparking! ZERO",
      "mods": {
        "sparking-zero-access": {
          "name": "Sparking Zero Access",
          "source": "github:jessica/sparking-zero-access",
          "version": "1.2.0",
          "installed_at": "2026-03-31T14:30:00Z",
          "loader": "ue4ss",
          "dependencies": {
            "utoc-signature-bypass": "1.0.0"
          }
        }
      }
    }
  }
}
```

## Field Reference

### Root

- `schema_version` (integer) — State file format version. Currently `1`. For future migrations.
- `last_update_check` (string, optional) — ISO 8601 timestamp of last app update check. Used for daily auto-check cooldown.
- `manifest_cache` (object, optional) — Cached mod manifests keyed by `owner/repo`. Used for ETag-based conditional fetching and offline fallback.

### `manifest_cache` entries

- `etag` (string, optional) — ETag from the last fetch. Sent as `If-None-Match` on subsequent requests.
- `yaml` (string) — The full manifest YAML content.

### `games` object

Keyed by slug (lowercase, hyphens, no special characters). Slug is stable — display name can change.

- `name` (string) — Human-readable game name (display only).
- `path` (string) — Absolute path to game root. Set by auto-detection or user selection via directory picker.

### `mods` object (per game)

Keyed by mod slug.

- `name` (string) — Human-readable mod name.
- `source` (string) — Full source reference for update checks. Format: `github:owner/repo` or `url:https://...`.
- `version` (string) — Currently installed version.
- `installed_at` (string) — ISO 8601 timestamp of install/last reinstall.
- `loader` (string) — Which loader was used to install this mod.
- `local_path` (string, optional) — Path to local source for dev installs (set by `accessforge install` from a local folder).
- `dependencies` (object, optional) — Map of dependency slug to installed version.

## Client UI States

Based on state file, the client shows:

- No mod entry exists — "Install" button
- Mod installed, manifest version not newer — "Reinstall" button
- Mod installed, manifest version is newer — "Update" button

Version comparison uses the `versions` crate for proper ordering (supports semver, pre-release tags like `1.0.0a1`, `1.0.0-rc.1`, etc.).

## Important Limitations

- The state file does NOT guarantee the mod is working
- Game updates (via Steam etc.) can break mods at any time
- "Reinstall" is always available because we can't verify mod integrity
- The platform does not monitor game updates

## File Safety

### Write pattern
1. Write new state to `state.json.tmp`
2. Copy current `state.json` to `state.json.backup`
3. Rename `state.json.tmp` to `state.json` (atomic on NTFS)

### Recovery
- If `state.json` is corrupt or missing, try `state.json.backup`
- If both are gone, start fresh — user re-selects game paths, mods show as not installed
- This is acceptable because reinstalling is cheap (just re-download and extract)

### Validation on load
- Check `schema_version` is recognized
- Unknown fields are preserved (serde default)

## Location

The state file lives at `%LOCALAPPDATA%/AccessForge/state.json` (typically `C:\Users\<user>\AppData\Local\AccessForge\`). Each Windows user gets their own state. The directory is created automatically on first run.
