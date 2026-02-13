# tli-tracker (CachyOS, Rust)

CLI tracker inspired by the TLI tracker workflow: track farming sessions, drops, and profit per minute.

## Features
- Start/stop farming sessions
- Log drops with quantity and value
- Summaries with total value and profit per minute
- JSON export for analysis

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

The AppImage automatically launches the web UI on http://127.0.0.1:8787/ and the overlay on http://127.0.0.1:8787/overlay

**Using the AppImage:**
- Double-click to run, or execute `./TLI-Tracker.AppImage` from terminal
- No installation required - runs from any location
- All data stored in `~/.local/share/tli-tracker/sessions.json`
- **Note:** The AppImage is designed for the web interface. For CLI usage, use one of the alternative installation methods below.

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

**Note:** If using the AppImage, the CLI commands are not directly accessible. The AppImage is designed to launch the web interface automatically. For CLI usage, build from source or use `cargo install`.

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

## Web Interface + Overlay

Start the local web UI (when using binary built from source):

```bash
./target/release/tli-tracker serve --host 127.0.0.1 --port 8787
```

Or if installed via cargo:

```bash
tli-tracker serve --host 127.0.0.1 --port 8787
```

Open in browser:
- Dashboard: http://127.0.0.1:8787/
- Overlay: http://127.0.0.1:8787/overlay

The overlay page is designed for OBS (Browser Source) or a desktop window rule to keep it on top.

**Note:** The AppImage automatically starts the web server when launched.

## Data location

Sessions are stored at:

`~/.local/share/tli-tracker/sessions.json`

## Torchlight Infinite game log (UE_game.log)

The tracker automatically detects the `UE_game.log` file produced by Torchlight Infinite.
The web UI header shows whether the log was found.

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

