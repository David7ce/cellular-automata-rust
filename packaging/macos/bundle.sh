#!/bin/bash
# Builds ConwayLife.app, then a DMG and ZIP of it, from an already-built
# release binary. macOS-only (sips/iconutil/hdiutil/codesign). Run from the
# repo root, e.g.:
#   packaging/macos/bundle.sh target/release/conway_life 0.2.0 macOS-arm64
#
# Used by .github/workflows/release.yml for both the arm64 and Intel
# builds, which only differ in which binary they hand this script.
set -euo pipefail

BINARY="$1"
APP_VERSION="$2"
ARCH_LABEL="$3"

APP="ConwayLife.app"
rm -rf "$APP" icon.iconset dmg_temp
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/GameOfLife"
chmod +x "$APP/Contents/MacOS/GameOfLife"

# Build the .icns from the 1024px master with macOS's own icon tools.
mkdir -p icon.iconset
for size in 16 32 64 128 256 512; do
  sips -z "$size" "$size" packaging/icons/icon-1024.png --out "icon.iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" packaging/icons/icon-1024.png --out "icon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns icon.iconset -o "$APP/Contents/Resources/icon.icns"

cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Game of Life</string>
  <key>CFBundleDisplayName</key><string>Game of Life</string>
  <key>CFBundleIdentifier</key><string>io.github.David7ce.ConwayLife</string>
  <key>CFBundleVersion</key><string>$APP_VERSION</string>
  <key>CFBundleShortVersionString</key><string>$APP_VERSION</string>
  <key>CFBundleExecutable</key><string>GameOfLife</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature — unsigned .app bundles are blocked by Gatekeeper on
# launch. Not a real Developer ID signature (needs a paid Apple account),
# but it lets the app run locally without notarization.
codesign --sign - --force --deep "$APP"

mkdir -p dist dmg_temp
cp -R "$APP" dmg_temp/
ln -s /Applications dmg_temp/Applications

for _ in 1 2 3; do
  hdiutil create -volname "Game of Life" -srcfolder dmg_temp -ov -format UDZO \
    "dist/ConwayLife-$APP_VERSION-$ARCH_LABEL.dmg" && break
  sleep 3
done

zip -ry "dist/ConwayLife-$APP_VERSION-$ARCH_LABEL.zip" "$APP"
