#!/bin/bash
# Builds both RPMs into target/rpm. Usage: build-rpm.sh <lwe-repo> <lwe-commit>
# Needs: rpm-build, rpmdevtools, cargo, the BuildRequires of both specs, and network (crates.io
# for vendoring and GitHub for the linux-wallpaperengine checkout).
set -euo pipefail
REPO="$1"; COMMIT="$2"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')
TOP="$ROOT/target/rpm"
SHORT="${COMMIT:0:7}"
mkdir -p "$TOP"/{SOURCES,SPECS,BUILD,RPMS,SRPMS}

echo "== source tarball stagehand-$VERSION"
git -C "$ROOT" archive --format=tar.gz --prefix="stagehand-$VERSION/" -o "$TOP/SOURCES/stagehand-$VERSION.tar.gz" HEAD

echo "== vendor tarball"
(cd "$ROOT" && cargo vendor --versioned-dirs "$TOP/BUILD/vendor-tmp/vendor" >/dev/null)
tar -C "$TOP/BUILD/vendor-tmp" -czf "$TOP/SOURCES/stagehand-$VERSION-vendor.tar.gz" vendor
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
# The spec applies packaging/lwe-no-cef.patch itself; the tarball must be the pristine tree.
git -C "$SRC" checkout -q -- .
tar -C "$(dirname "$SRC")" --exclude=.git --exclude=src/build --exclude=src/build-nocef --transform "s|^src|linux-wallpaperengine-$SHORT|" \
    -czf "$TOP/SOURCES/linux-wallpaperengine-$SHORT.tar.gz" src

cp "$ROOT/packaging/"*.spec "$TOP/SPECS/"
cp "$ROOT/packaging/"*.patch "$TOP/SOURCES/"
for spec in ${STAGEHAND_RPM_SPECS:-stagehand stagehand-scene-renderer}; do
    echo "== rpmbuild $spec"
    rpmbuild --define "_topdir $TOP" -ba "$TOP/SPECS/$spec.spec"
done
echo
echo "RPMs:"
find "$TOP/RPMS" -name '*.rpm'
echo
echo "Install with:  sudo dnf install \$(find $TOP/RPMS -name '*.rpm')"
echo "Then log out and back in; the daemon starts with the session and enables the extension."
