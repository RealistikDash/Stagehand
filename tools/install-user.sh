#!/bin/bash
# Per-user installation. Usage: install-user.sh <bindir> <libexecdir> <datadir> <configdir> <uuid>
set -euo pipefail
BIN="$1"; LIBEXEC="$2"; DATA="$3"; CONFIG="$4"; UUID="$5"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$BIN" "$LIBEXEC" "$DATA/applications" "$DATA/dbus-1/services" "$DATA/icons/hicolor/scalable/apps" \
    "$DATA/gnome-shell/extensions" "$CONFIG/systemd/user" "$DATA/metainfo"
install -m 755 "$ROOT/target/release/wpe-gnomed" "$ROOT/target/release/wpe-gnome" \
    "$ROOT/target/release/wpe-gnome-renderer" "$ROOT/target/release/wpe-gnomectl" "$BIN/"
sed "s|@BINDIR@|$BIN|g" "$ROOT/data/io.github.realistik.WpeGnome.Daemon.service" \
    > "$DATA/dbus-1/services/io.github.realistik.WpeGnome.Daemon.service"
sed "s|@BINDIR@|$BIN|g" "$ROOT/data/wpe-gnomed.service" > "$CONFIG/systemd/user/wpe-gnomed.service"
install -m 644 "$ROOT/data/io.github.realistik.WpeGnome.desktop" "$DATA/applications/"
install -m 644 "$ROOT/data/io.github.realistik.WpeGnome.svg" "$DATA/icons/hicolor/scalable/apps/"
install -m 644 "$ROOT/data/io.github.realistik.WpeGnome.metainfo.xml" "$DATA/metainfo/"
rm -rf "${DATA:?}/gnome-shell/extensions/$UUID"
cp -r "$ROOT/extension/$UUID" "$DATA/gnome-shell/extensions/$UUID"

if [ -d "$ROOT/build/lwe/build/output" ]; then
    rm -rf "$LIBEXEC/lwe"
    "$ROOT/tools/lwe-runtime-files.sh" "$ROOT/build/lwe/build/output" "$LIBEXEC/lwe"
else
    echo "note: build/lwe not found; run 'just lwe' to install scene support" >&2
fi

update-desktop-database "$DATA/applications" 2>/dev/null || true
gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true
systemctl --user daemon-reload
systemctl --user enable --now wpe-gnomed.service
gnome-extensions enable "$UUID" 2>/dev/null || true
cat <<MSG

Installed. The Shell extension is enabled but GNOME on Wayland loads new extensions only at
login: log out and back in once, then open "Wallpaper Engine" from the app grid.
Recovery from a TTY if the Shell misbehaves:  gnome-extensions disable $UUID
MSG
