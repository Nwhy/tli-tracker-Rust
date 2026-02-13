# tli-tracker (CachyOS, Rust)

Standalone loot tracker for Torchlight: Infinite. Parses the game's `UE_game.log` to automatically track picked-up items, session stats, and inventory — no manual input needed.

Inspired by [TITrack](https://github.com/astockman99/TITrack).

## Features

- **Standalone native GUI** — black/white themed desktop application (no browser required)
- **Automatic log parsing** — reads `UE_game.log` produced by Torchlight: Infinite via Steam/Proton
- **Flame Elementium tracking** — primary resource display with FE/hour calculation
- **Real-time loot tracking** — detects item pickups and shows deltas per item
- **Session tracking** — start/stop sessions to measure FE/hour and total loot
- **Inventory view** — shows current bag contents parsed from the log
- **Map detection** — identifies the current map from log events
- **File watching** — automatically refreshes when the log file changes
- **CLI commands** — full CLI for scripting and automation
- **JSON export** for external analysis

## Installation

### Recommended: AppImage (CachyOS)

The easiest way to install and run tli-tracker is using the portable AppImage:

**Option 1: Download pre-built AppImage**
```bash
# Download the latest AppImage from GitHub releases
# Visit: https://github.com/Nwhy/tli-tracker-Rust/releases
# Or use wget to download a specific version (replace v0.1.0 with desired version):
wget https://github.com/Nwhy/tli-tracker-Rust/releases/download/v0.1.0/TLI-Tracker.AppImage
chmod +x TLI-Tracker.AppImage
./TLI-Tracker.AppImage
```

**Option 2: Build AppImage yourself**
```bash
# Install build dependencies
sudo pacman -S --needed rustup base-devel curl

# Setup Rust
rustup default stable

# Build the AppImage
chmod +x scripts/build-appimage.sh
./scripts/build-appimage.sh
```

Output: `TLI-Tracker.AppImage`

The AppImage launches the standalone GUI application when double-clicked.

**Using the AppImage:**
- Double-click to run, or execute `./TLI-Tracker.AppImage` from terminal
- No installation required - runs from any location
- All data stored in `~/.local/share/tli-tracker/sessions.json`
- **Note:** The AppImage launches the GUI. For CLI usage, build from source.

### Alternative: Build from Source

```bash
# Install Rust
sudo pacman -S --needed rustup
rustup default stable

# Build the project
cargo build --release
```

Binary: `target/release/tli-tracker`

### Alternative: Install with Cargo

```bash
# Install Rust
sudo pacman -S --needed rustup
rustup default stable

# Install directly from source
cargo install --path .
```

Binary will be installed to `~/.cargo/bin/tli-tracker`

## Usage

### GUI (Recommended)

Launch the standalone GUI:

```bash
./target/release/tli-tracker gui
# Or if installed via cargo:
tli-tracker gui
```

The GUI automatically detects and parses `UE_game.log`. Use the **Start Session** button to begin tracking, then play the game — loot is tracked automatically.

### CLI

**Note:** If using the AppImage, the CLI commands are not directly accessible. The AppImage launches the GUI. For CLI usage, build from source or use `cargo install`.

Initialize storage (when using CLI):

```bash
./target/release/tli-tracker init
# Or if installed via cargo:
tli-tracker init
```

Start a session:

```bash
./target/release/tli-tracker start-session --map "Netherrealm" --notes "Test run"
# Or: tli-tracker start-session --map "Netherrealm" --notes "Test run"
```

Add drops:

```bash
./target/release/tli-tracker add-drop --name "Flame Core" --quantity 2 --value 18.5
# Or: tli-tracker add-drop --name "Flame Core" --quantity 2 --value 18.5
```

End session:

```bash
./target/release/tli-tracker end-session
# Or: tli-tracker end-session
```

Summary:

```bash
./target/release/tli-tracker summary
# Or: tli-tracker summary
```

Export:

```bash
./target/release/tli-tracker export --out ./sessions.json
# Or: tli-tracker export --out ./sessions.json
```

## Data location

Sessions are stored at:

`~/.local/share/tli-tracker/sessions.json`

## Torchlight Infinite game log (UE_game.log)

The tracker automatically detects the `UE_game.log` file produced by Torchlight Infinite.
The GUI header shows whether the log was found.

**Important:** You must enable logging in-game each time you launch Torchlight Infinite
(Settings → Other → Enable Log).

### Log file location

| Platform | Path |
|---|---|
| **Linux (Steam / Proton)** | `~/.steam/steam/steamapps/common/Torchlight Infinite/UE_game/TorchLight/Saved/Logs/UE_game.log` |
| **Linux (Flatpak Steam)** | `~/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/Torchlight Infinite/UE_game/TorchLight/Saved/Logs/UE_game.log` |

If you use a custom Steam library folder the path will be under that library instead.
The tracker checks `libraryfolders.vdf` automatically.

## Releasing

This project uses GitHub Actions to automatically build and publish AppImage releases.

### Creating a new release

1. Update the version in `Cargo.toml` if needed
2. Create and push a git tag:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The GitHub Actions workflow will automatically:
   - Build the AppImage
   - Create a GitHub release
   - Upload the AppImage to the release

The AppImage will be available at: `https://github.com/Nwhy/tli-tracker-Rust/releases`

### Manual workflow trigger

You can also trigger the release workflow manually from the GitHub Actions tab without creating a tag. This will build the AppImage and upload it as an artifact (but won't create a release).

