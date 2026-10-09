#!/bin/bash
# Builds linux-wallpaperengine from a pinned commit. Usage: build-lwe.sh <repo> <commit> <outdir>
set -euo pipefail
REPO="$1"; COMMIT="$2"; OUT="$3"
SRC="$OUT/src"
mkdir -p "$OUT"
if [ ! -d "$SRC/.git" ]; then
    git clone --recurse-submodules "$REPO" "$SRC"
fi
git -C "$SRC" fetch -q origin
git -C "$SRC" checkout -q "$COMMIT"
git -C "$SRC" submodule update --init --recursive -q
# Web wallpapers are rendered by stagehand-renderer; drop linux-wallpaperengine's CEF (Chromium) build.
git -C "$SRC" checkout -q -- . && git -C "$SRC" apply "$(dirname "${BASH_SOURCE[0]}")/../packaging/lwe-no-cef.patch"
cmake -S "$SRC" -B "$OUT/build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DENABLE_CEF=OFF >/dev/null
ninja -C "$OUT/build"
echo "linux-wallpaperengine built in $OUT/build/output"
