#!/usr/bin/env bash
# Builds the Linux x86_64 release in a container. Bevy links against ALSA,
# udev and xkbcommon, whose headers only exist on Linux, so this is the way
# to get a Linux binary without a Linux machine.
#
# The source is streamed into the container instead of bind-mounted: the
# daemon may well be remote (`docker context ls`), in which case a `-v`
# would mount a path on that host, not this one. The cargo registry and the
# target directory live in named volumes so a second run is incremental.
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="target-cross/the5cats-linux-x86_64"
IMAGE="rust:1-bookworm"

mkdir -p target-cross
tar -cf /tmp/the5cats-src.tar \
    --exclude=target --exclude=target-cross --exclude=dist --exclude=.git \
    -C .. "$(basename "$PWD")"

CID=$(docker create --platform linux/amd64 \
    -v the5cats-linux-target:/target \
    -v the5cats-cargo-registry:/usr/local/cargo/registry \
    "$IMAGE" bash -euo pipefail -c '
        mkdir -p /src && tar -xf /src.tar -C /src
        apt-get update -qq
        apt-get install -y -qq --no-install-recommends \
            pkg-config libasound2-dev libudev-dev \
            libwayland-dev libxkbcommon-dev
        cd /src/*/
        CARGO_TARGET_DIR=/target cargo build --release
        mkdir -p /out && cp /target/release/the5cats /out/
    ')

docker cp /tmp/the5cats-src.tar "$CID:/src.tar" >/dev/null
docker start -a "$CID"
docker cp "$CID:/out/the5cats" "$OUT" >/dev/null
docker rm -f "$CID" >/dev/null
rm -f /tmp/the5cats-src.tar
echo "listo: $OUT"
