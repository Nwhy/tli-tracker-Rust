# Install on CachyOS

This document describes how to install and run tli-tracker on CachyOS (Arch-based Linux distribution).

## Quick Install

The easiest way to install tli-tracker on CachyOS is using the automated install script:

```bash
curl -O https://raw.githubusercontent.com/Nwhy/tli-tracker-Rust/main/install_cachyos.sh
chmod +x install_cachyos.sh
sudo ./install_cachyos.sh
```

This will:
- Install Rust and build dependencies
- Clone and build the project
- Install the binary to `/usr/local/bin/`
- Create a systemd service for automatic startup
- Optionally configure nginx as a reverse proxy

## Install with Nginx

To also set up nginx as a reverse proxy:

```bash
sudo ./install_cachyos.sh --nginx
```

This will configure nginx to proxy requests from `http://localhost/` to the tli-tracker web service.

## Manual Installation

If you prefer to install manually:

### 1. Install Dependencies

```bash
sudo pacman -S --needed rustup base-devel git
rustup default stable
```

### 2. Clone and Build

```bash
git clone https://github.com/Nwhy/tli-tracker-Rust.git
cd tli-tracker-Rust
cargo build --release
```

### 3. Install Binary

```bash
sudo install -m 755 target/release/tli-tracker /usr/local/bin/
```

### 4. Create Systemd Service (Optional)

Create `/etc/systemd/system/tli-tracker.service`:

```ini
[Unit]
Description=TLI Tracker Web Service
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
ExecStart=/usr/local/bin/tli-tracker serve --host 127.0.0.1 --port 8787
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable tli-tracker.service
sudo systemctl start tli-tracker.service
```

## Usage

After installation, access the web interface at:

- Direct: http://127.0.0.1:8787/
- Via nginx (if configured): http://localhost/

The overlay is available at:

- Direct: http://127.0.0.1:8787/overlay
- Via nginx: http://localhost/overlay

## Service Management

Check service status:

```bash
sudo systemctl status tli-tracker
```

View logs:

```bash
sudo journalctl -u tli-tracker -f
```

Restart service:

```bash
sudo systemctl restart tli-tracker
```

Stop service:

```bash
sudo systemctl stop tli-tracker
```

## Data Location

Session data is stored at:

```
~/.local/share/tli-tracker/sessions.json
```

## Uninstall

To remove tli-tracker:

```bash
# Stop and disable service
sudo systemctl stop tli-tracker
sudo systemctl disable tli-tracker
sudo rm /etc/systemd/system/tli-tracker.service

# Remove binary
sudo rm /usr/local/bin/tli-tracker

# Remove nginx config (if configured)
sudo rm /etc/nginx/sites-available/tli-tracker
sudo rm /etc/nginx/sites-enabled/tli-tracker
sudo systemctl restart nginx

# Remove data (optional)
rm -rf ~/.local/share/tli-tracker/
```

## Troubleshooting

### Service won't start

Check the logs:

```bash
sudo journalctl -u tli-tracker -n 50
```

### Port already in use

If port 8787 is already in use, you can change it by editing the service file or running manually with a different port:

```bash
tli-tracker serve --host 127.0.0.1 --port 8788
```

### Permission issues

Make sure the user specified in the service file has the correct permissions and can access the data directory.
