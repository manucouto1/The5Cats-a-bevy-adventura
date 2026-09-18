#!/usr/bin/env bash
# Builds the release and packs it as a double-clickable macOS .app inside a
# zip, ready to upload to itch.io.
#
# The game looks for its `assets/` folder next to the executable and then in
# `Contents/Resources`, which is where this puts it — see `asset_root()` in
# src/main.rs.
set -euo pipefail

cd "$(dirname "$0")/.."
NAME="5Gatos"
APP="dist/$NAME.app"

# Universal binary: Apple Silicon natively, Intel through the second target.
# Without the x86_64 half the game simply will not start on an Intel Mac.
cargo build --release
rustup target add x86_64-apple-darwin >/dev/null
CARGO_TARGET_DIR=target-cross/macos-x86 cargo build --release --target x86_64-apple-darwin

# Only this platform's output — the other scripts write into dist/ too.
rm -rf "$APP" "dist/$NAME-macos.zip"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/$NAME" \
    target/release/the5cats \
    target-cross/macos-x86/x86_64-apple-darwin/release/the5cats
cp -R assets "$APP/Contents/Resources/assets"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>com.manuc.the5cats</string>
    <key>CFBundleVersion</key><string>1.0</string>
    <key>CFBundleShortVersionString</key><string>1.0</string>
    <key>CFBundleExecutable</key><string>$NAME</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cat > "dist/README-macos.txt" <<TXT
5Gatos

The build is not signed, so the first launch needs right-click -> Open (or
run: xattr -dr com.apple.quarantine 5Gatos.app).

Move: A/D or arrows. Jump: W/Up/Space, again in mid-air to double jump.
Aim with the mouse, left click throws a wool ball. Esc pauses, F11 is
fullscreen.
TXT

( cd dist && zip -qr "$NAME-macos.zip" "$NAME.app" README-macos.txt )
echo "listo: dist/$NAME-macos.zip ($(du -h "dist/$NAME-macos.zip" | cut -f1))"
