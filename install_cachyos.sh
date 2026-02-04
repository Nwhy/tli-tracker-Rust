#!/usr/bin/env bash
# install_cachyos.sh
#
# Install script for Nwhy/tli-tracker-Rust on CachyOS (Arch-based).
# - Installs build deps with pacman
# - Clones the repository, builds with cargo --release
# - Installs binary to /usr/local/bin (auto-detects produced binary)
# - Creates a systemd service unit
# - Optionally configures nginx as a reverse proxy
#
# Review the script before running. Run as a user with sudo privileges.
set -euo pipefail

REPO_URL="https://github.com/Nwhy/tli-tracker-Rust.git"
INSTALL_USER="${SUDO_USER:-$(whoami)}"
WORKDIR="/tmp/tli-tracker-install"
SERVICE_NAME="tli-tracker"
BINARY_INSTALL_PATH="/usr/local/bin"
SYSTEMD_PATH="/etc/systemd/system"
NGINX_SITES_AVAILABLE="/etc/nginx/sites-available"
NGINX_SITES_ENABLED="/etc/nginx/sites-enabled"

print_usage() {
  cat <<EOF
Usage: $0 [OPTIONS]

Install tli-tracker on CachyOS (Arch-based).

OPTIONS:
  --nginx         Setup nginx reverse proxy
  -h, --help      Show this help message

Examples:
  sudo $0
  sudo $0 --nginx
EOF
}

SETUP_NGINX=0

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --nginx)
      SETUP_NGINX=1
      shift
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      print_usage
      exit 1
      ;;
  esac
done

echo "=== TLI Tracker CachyOS Installation ==="
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo "Error: This script must be run as root (use sudo)" 
   exit 1
fi

# Install dependencies
echo "[1/6] Installing build dependencies..."
pacman -S --needed --noconfirm rustup base-devel git

# Setup Rust for the install user
echo "[2/6] Setting up Rust toolchain..."
if [[ -n "$SUDO_USER" ]]; then
  sudo -u "$SUDO_USER" rustup default stable
else
  rustup default stable
fi

# Clone and build
echo "[3/6] Cloning repository and building..."
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

if [[ -n "$SUDO_USER" ]]; then
  sudo -u "$SUDO_USER" git clone "$REPO_URL" .
  sudo -u "$SUDO_USER" cargo build --release
else
  git clone "$REPO_URL" .
  cargo build --release
fi

# Auto-detect the binary name from cargo build output
BINARY_NAME=$(find target/release -maxdepth 1 -type f -executable ! -name "*.so" ! -name "*.d" | head -n 1 | xargs basename)
if [[ -z "$BINARY_NAME" ]]; then
  echo "Error: Could not detect built binary in target/release/"
  exit 1
fi

echo "Detected binary: $BINARY_NAME"

# Install binary
echo "[4/6] Installing binary to $BINARY_INSTALL_PATH..."
install -m 755 "target/release/$BINARY_NAME" "$BINARY_INSTALL_PATH/$BINARY_NAME"

# Create systemd service
echo "[5/6] Creating systemd service..."
cat > "$SYSTEMD_PATH/$SERVICE_NAME.service" <<EOF
[Unit]
Description=TLI Tracker Web Service
After=network.target

[Service]
Type=simple
User=$INSTALL_USER
ExecStart=$BINARY_INSTALL_PATH/$BINARY_NAME serve --host 127.0.0.1 --port 8787
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME.service"

echo "Service created: $SERVICE_NAME.service"
echo "To start: systemctl start $SERVICE_NAME"
echo "To check status: systemctl status $SERVICE_NAME"

# Setup nginx if requested
if [[ $SETUP_NGINX -eq 1 ]]; then
  echo "[6/6] Setting up nginx reverse proxy..."
  
  # Install nginx if not present
  pacman -S --needed --noconfirm nginx
  
  # Create sites-available and sites-enabled directories if they don't exist
  mkdir -p "$NGINX_SITES_AVAILABLE" "$NGINX_SITES_ENABLED"
  
  # Create nginx config
  cat > "$NGINX_SITES_AVAILABLE/$SERVICE_NAME" <<EOF
server {
    listen 80;
    server_name localhost;

    location / {
        proxy_pass http://127.0.0.1:8787;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_cache_bypass \$http_upgrade;
    }
}
EOF
  
  # Enable the site
  ln -sf "$NGINX_SITES_AVAILABLE/$SERVICE_NAME" "$NGINX_SITES_ENABLED/$SERVICE_NAME"
  
  # Add include directive to main nginx.conf if not already present
  if ! grep -q "include.*sites-enabled" /etc/nginx/nginx.conf; then
    sed -i '/http {/a \    include '"$NGINX_SITES_ENABLED"'/*;' /etc/nginx/nginx.conf
  fi
  
  # Test nginx config
  nginx -t
  
  # Enable and restart nginx
  systemctl enable nginx
  systemctl restart nginx
  
  echo "Nginx configured as reverse proxy on http://localhost/"
else
  echo "[6/6] Skipping nginx setup (use --nginx to enable)"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Binary installed: $BINARY_INSTALL_PATH/$BINARY_NAME"
echo "Service: $SERVICE_NAME.service"
echo ""
echo "Next steps:"
echo "  1. Start the service: sudo systemctl start $SERVICE_NAME"
echo "  2. Check status: sudo systemctl status $SERVICE_NAME"
echo "  3. Access web UI: http://127.0.0.1:8787/"
if [[ $SETUP_NGINX -eq 1 ]]; then
  echo "  4. Or via nginx: http://localhost/"
fi
echo ""
echo "Data will be stored in: ~/.local/share/tli-tracker/"
