#!/bin/bash
# Per-user removal. Usage: uninstall-user.sh <bindir> <libexecdir> <datadir> <configdir> <uuid>
set -uo pipefail
BIN="$1"; LIBEXEC="$2"; DATA="$3"; CONFIG="$4"; UUID="$5"

if [ -x "$BIN/wpe-gnomectl" ]; then
    "$BIN/wpe-gnomectl" restore-background 2>/dev/null || true
fi
systemctl --user disable --now wpe-gnomed.service 2>/dev/null || true
gnome-extensions disable "$UUID" 2>/dev/null || true
rm -f "$BIN/wpe-gnomed" "$BIN/wpe-gnome" "$BIN/wpe-gnome-renderer" "$BIN/wpe-gnomectl"
rm -f "$DATA/dbus-1/services/io.github.realistik.WpeGnome.Daemon.service"
rm -f "$CONFIG/systemd/user/wpe-gnomed.service"
rm -f "$DATA/applications/io.github.realistik.WpeGnome.desktop"
rm -f "$DATA/icons/hicolor/scalable/apps/io.github.realistik.WpeGnome.svg"
rm -f "$DATA/metainfo/io.github.realistik.WpeGnome.metainfo.xml"
rm -rf "${DATA:?}/gnome-shell/extensions/$UUID"
rm -rf "${LIBEXEC:?}/lwe"
rmdir "$LIBEXEC" 2>/dev/null || true
systemctl --user daemon-reload
echo "Removed. Configuration in $CONFIG/wpe-gnome and the still images in the cache were kept."
