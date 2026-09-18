#!/usr/bin/env bash
# Packs the Linux build (see build-linux-docker.sh) for itch.io.
set -euo pipefail

cd "$(dirname "$0")/.."
NAME="5Gatos"
OUT="dist/$NAME-linux"

./scripts/build-linux-docker.sh

rm -rf "$OUT"
mkdir -p "$OUT"
cp target-cross/the5cats-linux-x86_64 "$OUT/$NAME"
chmod +x "$OUT/$NAME"
cp -R assets "$OUT/assets"

cat > "$OUT/README.txt" <<TXT
5Gatos

Run ./5Gatos from this folder (it looks for assets next to itself).

Needs ALSA and a Wayland or X11 session; on Debian/Ubuntu that is
libasound2 and libxkbcommon0, both installed by default on a desktop.

Move: A/D or arrows. Jump: W/Up/Space, again in mid-air to double jump.
Aim with the mouse, left click throws a wool ball. Esc pauses, F11 is
fullscreen.
TXT

( cd dist && tar -czf "$NAME-linux-x86_64.tar.gz" "$NAME-linux" )
echo "listo: dist/$NAME-linux-x86_64.tar.gz ($(du -h "dist/$NAME-linux-x86_64.tar.gz" | cut -f1))"
