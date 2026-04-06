# AccessForge

1-click accessibility mod platform for blind gamers.

AccessForge discovers, installs, and updates screen reader mods for mainstream games. It handles mod loaders, dependencies, and game detection automatically. Download the exe, run it, pick a mod, click install.

Built with accessibility first — the entire UI works with screen readers like NVDA. Portable single-exe app, no installer needed.

## For Players

### Getting started

1. Download `AccessForge.exe` from [GitHub Releases](https://github.com/AccessForge/AccessForge/releases)
2. Put it anywhere and run it
3. Browse available mods, select one, click Install

> **Note:** Windows may show a SmartScreen warning since the exe is not code-signed. Click "More info", then "Run anyway" to proceed.

That's it. AccessForge takes care of:

- Finding your game (Steam auto-detection, or browse to the folder manually for non-Steam games)
- Installing any required technical components (mod loaders, dependencies)
- Installing the mod itself
- Checking for mod updates on every launch
- Updating itself automatically

Tested with NVDA on Windows 10 and 11.

### Updating mods

When a mod author releases a new version, it shows up in the Updates tab. Select it and click Update.

## For Mod Developers

### Quick start

```
accessforge init
```

This walks you through creating an `accessforge.yml` manifest. It auto-detects your git remote, author name, and Steam game info.

Then test your install flow:

```
accessforge install
```

This installs the loader, dependencies, and your mod files locally — exactly as a player would experience it. Run it from your project folder or any subfolder (it searches parent directories for the manifest).

When ready to release:

```
accessforge package
```

This builds a release zip and validates it matches your loader's expected structure.

Finally, publish:

1. Push your code to GitHub (with `accessforge.yml` in the repo root)
2. Create a GitHub Release, upload the zip
3. Add the `accessforge-mod` topic to your repo

Players will now see your mod in AccessForge.

### You keep your repo

AccessForge discovers mods via GitHub topics — there are no repo transfers, no central registry you need permission to join, and no one who can revoke your access. Your mod lives in your repo, on your terms.

### Supported loaders

| Loader | Engine | Mod format |
|--------|--------|------------|
| UE4SS | Unreal Engine 4/5 | Lua scripts |
| BepInEx | Unity (Mono) | .NET DLLs |
| MelonLoader | Unity (Mono + IL2CPP) | .NET DLLs |
| none | Any | Direct file replacement |

For the full walkthrough, see the [Getting Started Guide](docs/getting-started.md).

## Contributing

AccessForge is a community project. It's not owned by any single person, and it's designed to stay that way. The platform exists to make accessibility modding easier for everyone — players and developers alike.

There are many ways to help:

- **Build a mod** — pick a game, create an accessibility mod, and publish it with AccessForge
- **Contribute code** — check the issues, pick one, send a PR
- **Report bugs** — if something doesn't work, open an issue
- **Improve docs** — better guides help everyone
- **Spread the word** — tell other modders and players about AccessForge

All contributions are welcome, no matter the size. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## Commands

```
accessforge                               Launch the GUI
accessforge init [--from <path>]           Create an accessforge.yml manifest
accessforge install [--from <path|url>]    Install a mod from a local folder or URL
accessforge package [--from <path>]        Build a release zip
accessforge setup-path                     Add AccessForge to your PATH
accessforge help                           Show help
accessforge --version                      Show version
```

All commands default to the current directory if `--from` is omitted.

## Building from Source

### Prerequisites

- Rust (latest stable via [rustup](https://rustup.rs))
- CMake
- LLVM (set `LIBCLANG_PATH=C:\Program Files\LLVM\bin`)
- Visual Studio 2022 Build Tools with C++ workload and Windows 11 SDK

### Build

```
cargo build --release
```

The binary is at `target/release/AccessForge.exe`.

## Documentation

- [Getting Started Guide](docs/getting-started.md) — full tutorial for mod developers
- [Manifest Spec](docs/technical/manifest-spec.md) — field reference for `accessforge.yml`
- [State File Spec](docs/technical/state-file-spec.md) — how AccessForge tracks installs
- [Design Notes](docs/technical/design-notes.md) — architecture and design decisions

## License

GPL-3.0 — see [LICENSE](LICENSE) for details.
