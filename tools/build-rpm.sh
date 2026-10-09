#!/bin/bash
# Builds both RPMs into target/rpm. Usage: build-rpm.sh <lwe-repo> <lwe-commit>
# Needs: rpm-build, rpmdevtools, cargo, the BuildRequires of both specs, and network (crates.io
# for vendoring, GitHub for the linux-wallpaperengine checkout and its CEF download).
set -euo pipefail
REPO="$1"; COMMIT="$2"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')
TOP="$ROOT/target/rpm"
SHORT="${COMMIT:0:7}"
mkdir -p "$TOP"/{SOURCES,SPECS,BUILD,RPMS,SRPMS}

echo "== source tarball wpe-gnome-$VERSION"
git -C "$ROOT" archive --format=tar.gz --prefix="wpe-gnome-$VERSION/" -o "$TOP/SOURCES/wpe-gnome-$VERSION.tar.gz" HEAD

echo "== vendor tarball"
(cd "$ROOT" && cargo vendor --versioned-dirs "$TOP/BUILD/vendor-tmp/vendor" >/dev/null)
tar -C "$TOP/BUILD/vendor-tmp" -czf "$TOP/SOURCES/wpe-gnome-$VERSION-vendor.tar.gz" vendor
rm -rf "$TOP/BUILD/vendor-tmp"

echo "== linux-wallpaperengine source tarball ($SHORT)"
SRC="$ROOT/build/lwe/src"
if [ ! -d "$SRC/.git" ]; then
    mkdir -p "$(dirname "$SRC")"
    git clone --recurse-submodules "$REPO" "$SRC"
fi
git -C "$SRC" fetch -q origin
git -C "$SRC" checkout -q "$COMMIT"
git -C "$SRC" submodule update --init --recursive -q
tar -C "$(dirname "$SRC")" --exclude=.git --exclude=build --transform "s|^src|linux-wallpaperengine-$SHORT|" \
    -czf "$TOP/SOURCES/linux-wallpaperengine-$SHORT.tar.gz" src

cp "$ROOT/packaging/"*.spec "$TOP/SPECS/"
for spec in wpe-gnome wpe-gnome-scene-renderer; do
    echo "== rpmbuild $spec"
    rpmbuild --define "_topdir $TOP" -ba "$TOP/SPECS/$spec.spec"
done
echo
echo "RPMs:"
find "$TOP/RPMS" -name '*.rpm'
echo
echo "Install with:  sudo dnf install \$(find $TOP/RPMS -name '*.rpm')"
echo "Then log out and back in; the daemon starts with the session and enables the extension."
