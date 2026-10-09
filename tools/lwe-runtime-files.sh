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
# CEF ships unstripped (libcef.so alone is 1.4 GB); stripping brings the set down to ~300 MB.
find "$DEST" -type f \( -name '*.so' -o -name '*.so.*' -o -name linux-wallpaperengine \) -exec strip --strip-unneeded {} + 2>/dev/null
exit 0
