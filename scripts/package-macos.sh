#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd "$SCRIPT_DIR/.." && pwd)
APP_DIR="$REPO_ROOT/target/SSH Tray.app"

cd "$REPO_ROOT"
cargo build --release --features panel --bins

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
cp "target/release/ssh-tray" "$APP_DIR/Contents/MacOS/ssh-tray"
cp "target/release/ssh-tray-panel" "$APP_DIR/Contents/MacOS/ssh-tray-panel"
cp "packaging/Info.plist" "$APP_DIR/Contents/Info.plist"
printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"
chmod 755 "$APP_DIR/Contents/MacOS/ssh-tray" "$APP_DIR/Contents/MacOS/ssh-tray-panel"
