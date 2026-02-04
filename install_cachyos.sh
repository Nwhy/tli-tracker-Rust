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

Install tli-tracker on CachyOS.

OPTIONS:
  --with-nginx         Configure nginx reverse proxy (default: no)
  --nginx-domain NAME  Domain name for nginx (default: localhost)
  --port PORT          Port for tli-tracker service (default: 8787)
  --help               Show this help message

EXAMPLES:
  sudo $0
  sudo $0 --with-nginx --nginx-domain tracker.local --port 8080
EOF
}

# Default options
WITH_NGINX=false
NGINX_DOMAIN="localhost"
SERVICE_PORT=8787

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-nginx)
      WITH_NGINX=true
      shift
      ;;
    --nginx-domain)
      NGINX_DOMAIN="$2"
      shift 2
      ;;
    --port)
      SERVICE_PORT="$2"
      shift 2
      ;;
    --help)
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

echo "===== TLI Tracker CachyOS Installer ====="
echo "Repository: $REPO_URL"
echo "Work directory: $WORKDIR"
echo "Service port: $SERVICE_PORT"
echo "Nginx proxy: $WITH_NGINX"
if [[ "$WITH_NGINX" == "true" ]]; then
  echo "Nginx domain: $NGINX_DOMAIN"
fi
echo ""

# Check if running with sudo
if [[ $EUID -ne 0 ]]; then
  echo "Error: This script must be run with sudo"
  exit 1
fi

# Step 1: Install build dependencies
echo "[1/6] Installing build dependencies..."
pacman -S --needed --noconfirm rustup git base-devel

# Ensure rustup is initialized for the user
if ! sudo -u "$INSTALL_USER" rustup --version &>/dev/null; then
  echo "Initializing rustup for user $INSTALL_USER..."
  sudo -u "$INSTALL_USER" rustup default stable
else
  echo "Rustup already initialized, updating..."
  sudo -u "$INSTALL_USER" rustup update stable
fi

# Step 2: Clone and build
echo "[2/6] Cloning and building project..."
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cd "$WORKDIR"
sudo -u "$INSTALL_USER" git clone "$REPO_URL" .
sudo -u "$INSTALL_USER" cargo build --release 2>&1 | tee build.log

# Step 3: Auto-detect binary name from build output
echo "[3/6] Detecting binary name..."
# Look for the binary in target/release/ that was just built
BINARY_NAME=$(grep -oP '(?<=Compiling )[a-zA-Z0-9_-]+(?= v)' build.log | tail -1)
if [[ -z "$BINARY_NAME" ]]; then
  # Fallback: try to find executable in target/release
  BINARY_NAME=$(find target/release -maxdepth 1 -type f -executable ! -name "*.d" ! -name "*.so" | head -1 | xargs basename)
fi

if [[ -z "$BINARY_NAME" ]]; then
  echo "Error: Could not detect binary name from build output"
  exit 1
fi

BINARY_PATH="target/release/$BINARY_NAME"
if [[ ! -f "$BINARY_PATH" ]]; then
  echo "Error: Binary not found at $BINARY_PATH"
  exit 1
fi

echo "Detected binary: $BINARY_NAME at $BINARY_PATH"

# Step 4: Install binary
echo "[4/6] Installing binary to $BINARY_INSTALL_PATH..."
install -Dm755 "$BINARY_PATH" "$BINARY_INSTALL_PATH/$BINARY_NAME"
echo "Installed: $BINARY_INSTALL_PATH/$BINARY_NAME"

# Step 5: Create systemd service
echo "[5/6] Creating systemd service..."
cat > "$SYSTEMD_PATH/$SERVICE_NAME.service" <<EOF
[Unit]
Description=TLI Tracker Service
After=network.target

[Service]
Type=simple
User=$INSTALL_USER
WorkingDirectory=/home/$INSTALL_USER
ExecStart=$BINARY_INSTALL_PATH/$BINARY_NAME serve --host 127.0.0.1 --port $SERVICE_PORT
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME.service"
echo "Systemd service created: $SERVICE_NAME.service"
echo "To start the service: sudo systemctl start $SERVICE_NAME"
echo "To check status: sudo systemctl status $SERVICE_NAME"

# Step 6: Configure nginx if requested
if [[ "$WITH_NGINX" == "true" ]]; then
  echo "[6/6] Configuring nginx reverse proxy..."
  
  # Install nginx if not present
  if ! command -v nginx &>/dev/null; then
    echo "Installing nginx..."
    pacman -S --needed --noconfirm nginx
  fi
  
  # Create sites-available and sites-enabled directories if they don't exist
  mkdir -p "$NGINX_SITES_AVAILABLE" "$NGINX_SITES_ENABLED"
  
  # Create nginx config
  cat > "$NGINX_SITES_AVAILABLE/$SERVICE_NAME" <<EOF
server {
    listen 80;
    server_name $NGINX_DOMAIN;

    location / {
        proxy_pass http://127.0.0.1:$SERVICE_PORT;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_cache_bypass \$http_upgrade;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF
  
  # Enable the site
  ln -sf "$NGINX_SITES_AVAILABLE/$SERVICE_NAME" "$NGINX_SITES_ENABLED/$SERVICE_NAME"
  
  # Update nginx.conf to include sites-enabled if not already
  if ! grep -q "include.*sites-enabled" /etc/nginx/nginx.conf; then
    echo "Adding sites-enabled include to nginx.conf..."
    sed -i '/http {/a \    include /etc/nginx/sites-enabled/*;' /etc/nginx/nginx.conf
  fi
  
  # Test and reload nginx
  nginx -t
  systemctl enable nginx
  systemctl restart nginx
  
  echo "Nginx configured for domain: $NGINX_DOMAIN"
  echo "Access the web interface at: http://$NGINX_DOMAIN/"
else
  echo "[6/6] Skipping nginx configuration (use --with-nginx to enable)"
fi

echo ""
echo "===== Installation Complete ====="
echo "Binary installed: $BINARY_INSTALL_PATH/$BINARY_NAME"
echo "Service: $SERVICE_NAME.service"
echo ""
echo "Next steps:"
echo "  1. Start the service: sudo systemctl start $SERVICE_NAME"
echo "  2. Check status: sudo systemctl status $SERVICE_NAME"
if [[ "$WITH_NGINX" == "true" ]]; then
  echo "  3. Access web interface: http://$NGINX_DOMAIN/"
else
  echo "  3. Access web interface: http://127.0.0.1:$SERVICE_PORT/"
fi
echo ""
echo "Clean up work directory:"
echo "  sudo rm -rf $WORKDIR"
