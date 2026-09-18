#!/usr/bin/env bash
# Cross-builds the Windows release from macOS with cargo-xwin, which fetches
# the Microsoft SDK and CRT itself, and zips it for itch.io.
set -euo pipefail

cd "$(dirname "$0")/.."
NAME="5Gatos"
OUT="dist/$NAME-windows"

command -v cargo-xwin >/dev/null || cargo install cargo-xwin
rustup target add x86_64-pc-windows-msvc >/dev/null
CARGO_TARGET_DIR=target-cross/windows cargo xwin build --release \
    --target x86_64-pc-windows-msvc

rm -rf "$OUT"
mkdir -p "$OUT"
cp target-cross/windows/x86_64-pc-windows-msvc/release/the5cats.exe "$OUT/$NAME.exe"
cp -R assets "$OUT/assets"

cat > "$OUT/README.txt" <<TXT
5Gatos

Run 5Gatos.exe. Keep it next to the assets folder.

Windows may warn that the publisher is unknown: the build is not signed.
More info -> Run anyway.

Move: A/D or arrows. Jump: W/Up/Space, again in mid-air to double jump.
Aim with the mouse, left click throws a wool ball. Esc pauses, F11 is
fullscreen.
TXT

( cd dist && zip -qr "$NAME-windows.zip" "$NAME-windows" )
echo "listo: dist/$NAME-windows.zip ($(du -h "dist/$NAME-windows.zip" | cut -f1))"
