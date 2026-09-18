#!/usr/bin/env bash
# Builds the browser version and zips it the way itch.io wants it: an
# `index.html` at the root of the archive, with the wasm, the JS glue and
# the assets beside it.
#
# Level JSONs are compiled into the binary for this target (see
# `read_level_file` in src/game_state.rs); everything else — sprites, fonts,
# audio, shaders — is fetched from `assets/` at runtime.
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="dist/web"
WASM="target-cross/wasm/wasm32-unknown-unknown/release/the5cats.wasm"

CARGO_TARGET_DIR=target-cross/wasm cargo build --release --target wasm32-unknown-unknown

rm -rf "$OUT"
mkdir -p "$OUT"
wasm-bindgen --no-typescript --target web --out-dir "$OUT" --out-name the5cats "$WASM"

# Optional but worth it: takes tens of MB off the download.
if command -v wasm-opt >/dev/null 2>&1; then
    echo "optimizando con wasm-opt…"
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
        -o "$OUT/the5cats_bg.wasm" "$OUT/the5cats_bg.wasm"
fi

cp scripts/web/index.html "$OUT/index.html"
cp -R assets "$OUT/assets"
# Trim what the browser will never fetch: the level JSONs are compiled in,
# and the tilemap sources and level drafts are editor files.
find "$OUT/assets" -name "*.json" -delete
find "$OUT/assets" -name "*.tset" -delete
find "$OUT/assets" -name "*.txt" -delete
rm -rf "$OUT/assets/levels/projects" "$OUT/assets/levels/level2b" \
       "$OUT/assets/levels/level2/level2_new"

( cd "$OUT" && zip -qr ../5Gatos-web.zip . )
echo "listo: dist/5Gatos-web.zip ($(du -h dist/5Gatos-web.zip | cut -f1))"
