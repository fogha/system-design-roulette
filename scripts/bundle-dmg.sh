#!/usr/bin/env bash
# Rebuild the macOS DMG after `tauri build -- --bundles app`.
# Works around hdiutil's auto-sizing bug on macOS 15 (creates an image too
# small for the bundle, failing with ENOSPC) by passing an explicit size.
set -euo pipefail
cd "$(dirname "$0")/.."

APP_DIR="src-tauri/target/release/bundle/macos"
DMG_DIR="src-tauri/target/release/bundle/dmg"
SCRIPT="$DMG_DIR/bundle_dmg.sh"
VERSION="$(node -p "require('./package.json').version")"
DMG="$DMG_DIR/System Design Roulette_${VERSION}_$(uname -m).dmg"

if [ ! -f "$SCRIPT" ]; then
  echo "run \`npm run tauri build -- --bundles app\` first" >&2
  exit 1
fi

rm -f "$DMG" "$DMG_DIR"/rw.*.dmg
bash "$SCRIPT" \
  --volname "System Design Roulette" \
  --disk-image-size 300 \
  --window-size 660 400 \
  --icon-size 128 \
  --app-drop-link 480 170 \
  --icon "System Design Roulette.app" 180 170 \
  "$DMG" "$APP_DIR"

echo "built $DMG"
