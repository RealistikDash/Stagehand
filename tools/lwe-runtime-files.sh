#!/bin/bash
# Copies the runtime subset of a linux-wallpaperengine build output into a target directory.
# Usage: lwe-runtime-files.sh <build/output> <destination>
set -euo pipefail
SRC="$1"; DEST="$2"
mkdir -p "$DEST"
cp -a "$SRC/linux-wallpaperengine" "$DEST/"
for f in "$SRC"/*.so* "$SRC"/*.pak "$SRC"/*.dat "$SRC"/*.bin "$SRC"/*.json "$SRC"/chrome-sandbox; do
    [ -e "$f" ] && cp -a "$f" "$DEST/"
done
for d in lib lib64 locales swiftshader assets; do
    [ -d "$SRC/$d" ] && cp -a "$SRC/$d" "$DEST/"
done
exit 0
