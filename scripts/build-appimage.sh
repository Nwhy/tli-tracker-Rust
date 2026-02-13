#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$ROOT_DIR/AppDir"
BIN_NAME="tli-tracker"

mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/scalable/apps"

cargo build --release

install -m 0755 "$ROOT_DIR/target/release/$BIN_NAME" "$APPDIR/usr/bin/$BIN_NAME"

install -m 0644 "$ROOT_DIR/appimage/tli-tracker.desktop" "$APPDIR/usr/share/applications/tli-tracker.desktop"
install -m 0644 "$ROOT_DIR/appimage/tli-tracker.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/tli-tracker.svg"

cp "$ROOT_DIR/appimage/tli-tracker.desktop" "$APPDIR/"
cp "$ROOT_DIR/appimage/tli-tracker.svg" "$APPDIR/"

LINUXDEPLOY="$ROOT_DIR/.tools/linuxdeploy-x86_64.AppImage"
APPIMAGETOOL="$ROOT_DIR/.tools/appimagetool-x86_64.AppImage"

mkdir -p "$ROOT_DIR/.tools"

if [[ ! -f "$LINUXDEPLOY" ]]; then
  curl -L -o "$LINUXDEPLOY" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  chmod +x "$LINUXDEPLOY"
fi

if [[ ! -f "$APPIMAGETOOL" ]]; then
  curl -L -o "$APPIMAGETOOL" https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$APPIMAGETOOL"
fi

"$LINUXDEPLOY" --appdir "$APPDIR" --desktop-file "$APPDIR/tli-tracker.desktop" --icon-file "$APPDIR/tli-tracker.svg"

# Replace the generated AppRun with a custom one that launches the web server
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/usr/bin/env bash
set -euo pipefail
SELF="$(readlink -f "$0")"
HERE="${SELF%/*}"
export PATH="$HERE/usr/bin:$PATH"
export LD_LIBRARY_PATH="$HERE/usr/lib:${LD_LIBRARY_PATH:-}"
exec "$HERE/usr/bin/tli-tracker" serve --host 127.0.0.1 --port 8787
APPRUN
chmod +x "$APPDIR/AppRun"

"$APPIMAGETOOL" "$APPDIR" "$ROOT_DIR/TLI-Tracker.AppImage"

echo "Built: $ROOT_DIR/TLI-Tracker.AppImage"
