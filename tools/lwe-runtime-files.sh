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
# Bundled libraries (kissfft) are built next to the output directory, and the build tree's
# runpaths point there; ship them under lib/ and make the runpaths relative.
mkdir -p "$DEST/lib"
cp -a "$SRC/../lib/"*.so* "$DEST/lib/" 2>/dev/null || true
patchelf --set-rpath '$ORIGIN:$ORIGIN/lib' "$DEST/linux-wallpaperengine"
for f in "$DEST"/*.so "$DEST"/*.so.*; do
    [ -f "$f" ] && patchelf --set-rpath '$ORIGIN:$ORIGIN/lib' "$f"
done
find "$DEST" -type f \( -name '*.so' -o -name '*.so.*' -o -name linux-wallpaperengine \) -exec strip --strip-unneeded {} + 2>/dev/null
exit 0
