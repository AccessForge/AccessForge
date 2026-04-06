# Getting Started with AccessForge

This guide walks you through creating an accessibility mod, writing your manifest, testing locally, and packaging for release.

## Prerequisites

- A game with a mod loader (UE4SS, BepInEx, MelonLoader) or one that supports direct file mods
- Your mod files (Lua scripts, DLLs, or other assets)
- The AccessForge executable

## Step 1: Set up AccessForge

Download `AccessForge.exe` and place it somewhere convenient (e.g. `C:\AccessForge\`). To use it from the terminal without typing the full path, run it once to add itself to your PATH:

```
C:\AccessForge\AccessForge.exe setup-path
```

Restart your terminal after running this. You can now use `accessforge` from anywhere.

## Step 2: Create your manifest

In the root of your mod project folder, run:

```
accessforge init
```

This walks you through an interactive setup:
1. Which store the game is on (Steam or none)
2. Steam app ID or game folder path
3. Game name (auto-detected from Steam or folder name)
4. Which mod loader to use
5. Mod name and description (smart defaults based on game name)
6. Author (auto-detected from git config)
7. Source (auto-detected from git remote)

It creates an `accessforge.yml` manifest with all the right values.

### Manual manifest creation

You can also write the manifest by hand. Here's a minimal example for a UE4SS Lua mod:

```yaml
spec: 1

id: MyGameAccess
name: My Game Access
description: Screen reader accessibility mod for My Game
author: yourname
version: "1.0.0"
source: github:yourname/my-game-access

game:
  name: My Game
  store:
    steam: 123456

loader:
  name: ue4ss

release:
  asset: "MyGameAccess-{version}.zip"
```

Here's one for a BepInEx Unity mod:

```yaml
spec: 1

id: MyUnityAccess
name: My Unity Access
description: Screen reader mod for My Unity Game
author: yourname
version: "1.0.0"
source: github:yourname/my-unity-access

game:
  name: My Unity Game
  store:
    steam: 654321

loader:
  name: bepinex

release:
  asset: "MyUnityAccess-{version}.zip"
```

And one for a game that doesn't need a mod loader:

```yaml
spec: 1

id: MySimpleAccess
name: My Simple Access
description: Accessibility mod for My Simple Game
author: yourname
version: "1.0.0"
source: github:yourname/my-simple-access

game:
  name: My Simple Game
  store:
    steam: 111222

loader:
  name: none

release:
  asset: "MySimpleAccess-{version}.zip"
```

### Key fields

- `id` — A stable technical identifier for your mod. Used for folder names and internal tracking. Pick something short without spaces (e.g. `SparkingZeroAccess`). Never change this once published.
- `name` — The display name users see. You can change this later without breaking anything.
- `version` — Your mod's version. Must be quoted. Supports semver (`1.0.0`), pre-release tags (`1.0.0-alpha.1`, `1.0.0a1`), and dev versions.
- `source` — Where your mod is hosted. Use `github:yourname/your-repo` for GitHub.
- `game.store.steam` — The Steam app ID from the game's store page URL. This lets AccessForge find the game automatically.
- `loader.name` — Which mod loader your game uses: `ue4ss`, `bepinex`, `melonloader`, or `none`.
- `release.asset` — The filename template for your release zip. Use `{version}` as a placeholder that gets replaced with the version number.

### Adding dependencies

If your mod requires patches or other mods to work, list them:

```yaml
dependencies:
  - name: UTOC Signature Bypass
    type: patch
    source: github:accessforge/utoc-bypass-sparking-zero
    asset: "utoc-bypass.zip"
    version: "1.0.0"
```

- `type: patch` — extracted next to the game executable
- `type: mod` — extracted to the mod loader's mod folder

### Pinning a loader version

By default AccessForge installs the latest loader version. To pin a specific version:

```yaml
loader:
  name: bepinex
  version: "5.4.23.5"
```

Partial versions work too — `"5.4"` installs the latest 5.4.x release.

## Step 3: Organize your mod files

Your project folder should look something like this:

For a UE4SS Lua mod — put your Lua files in a subfolder (AccessForge wraps them in `Scripts/` automatically during install):
```
my-mod/
  accessforge.yml
  SparkingAccess/
    main.lua
    helpers.lua
```

Then install with:
```
accessforge install --from SparkingAccess
```

Or `cd SparkingAccess && accessforge install` (it finds the manifest in the parent directory).

For a BepInEx or MelonLoader mod:
```
my-mod/
  accessforge.yml
  MyMod.dll
```

If you have a build step that outputs to a different folder, you can use a `dist/` folder. AccessForge will package from `dist/` if it exists:

```
my-mod/
  accessforge.yml
  src/
    ...
  dist/
    MyMod.dll
```

## Step 4: Test locally

To test the full install flow (loader + dependencies + your mod), as a player would experience it:

```
accessforge install
```

Run this from your mod's project folder (or any subfolder — it searches parent directories for `accessforge.yml`). For example, if you're inside `my-mod/Scripts/`, it finds the manifest in `my-mod/` and copies files from `Scripts/`.

It:
1. Finds `accessforge.yml` in the current directory or a parent
2. Finds the game via Steam or a previously saved path
3. Installs the mod loader (if needed)
4. Downloads and installs dependencies from their remote sources
5. Copies your local mod files to the game (wrapped in the right folder structure for the loader)
6. Runs post-install hooks (e.g. enables the mod in UE4SS mods.txt)
7. Saves install state so the GUI knows about it

You can also point it at a specific folder:

```
accessforge install --from Scripts
```

### Share with testers before publishing

Host your manifest and mod zip on any web server (even a simple file host). Your testers run:

```
accessforge install --from "https://example.com/my-mod"
```

For this to work, your files should be at:
- `https://example.com/my-mod/accessforge.yml` — the manifest
- `https://example.com/my-mod/MyMod-1.0.0.zip` — the release zip

Update your manifest's `source` field to point to the URL:

```yaml
source: "url:https://example.com/my-mod"
```

## Step 5: Package for release

When you're ready to publish:

```
accessforge package
```

This creates a zip file named according to your `release.asset` template (e.g. `MyGameAccess-1.0.0.zip`) in your project folder.

If a `dist/` folder exists, it packages from there. Otherwise it packages everything in the project root, excluding the manifest, dotfiles, `.git`, and any `.zip` files.

After packaging, AccessForge validates the zip:
- UE4SS mods: checks for files under `Scripts/`
- BepInEx/MelonLoader mods: checks for `.dll` files
- No-loader mods: warns if only 1 file (mods typically have multiple)

## Step 6: Publish

1. Create a GitHub repository for your mod
2. Push your code (including `accessforge.yml` in the repo root)
3. Create a GitHub Release with a tag matching your version (e.g. `v1.0.0`)
4. Upload the zip from Step 5 as a release asset
5. Add the `accessforge-mod` topic to your repository so AccessForge can discover it

Players can now find and install your mod through the AccessForge app.

## Updating your mod

1. Make your changes
2. Update `version` in `accessforge.yml`
3. Run `accessforge package` to build the new zip
4. Create a new GitHub Release with the new version tag
5. Upload the new zip

Players will see the update in the Updates tab and can install it with one click.

## Command reference

```
accessforge                               Launch GUI
accessforge init [--from <path>]           Create accessforge.yml
accessforge install [--from <path|url>]    Install from local folder or HTTP URL
accessforge package [--from <path>]        Build release zip from local folder
accessforge setup-path                     Add AccessForge to your PATH
accessforge help                           Show help
```

All commands default to the current directory if `--from` is omitted.

## Further reading

- [Manifest spec](technical/manifest-spec.md) — full field reference and validation rules
- [State file spec](technical/state-file-spec.md) — how AccessForge tracks installed mods
- [Design notes](technical/design-notes.md) — architecture and design decisions
