#!/bin/bash
# Per-user removal. Usage: uninstall-user.sh <bindir> <libexecdir> <datadir> <configdir> <uuid>
set -uo pipefail
BIN="$1"; LIBEXEC="$2"; DATA="$3"; CONFIG="$4"; UUID="$5"

if [ -x "$BIN/stagehandctl" ]; then
    "$BIN/stagehandctl" restore-background 2>/dev/null || true
fi
systemctl --user disable --now stagehandd.service 2>/dev/null || true
gnome-extensions disable "$UUID" 2>/dev/null || true
rm -f "$BIN/stagehandd" "$BIN/stagehand" "$BIN/stagehand-renderer" "$BIN/stagehandctl"
rm -f "$DATA/dbus-1/services/io.github.realistikdash.Stagehand.Daemon.service"
rm -f "$CONFIG/systemd/user/stagehandd.service"
rm -f "$DATA/applications/io.github.realistikdash.Stagehand.desktop"
rm -f "$DATA/icons/hicolor/scalable/apps/io.github.realistikdash.Stagehand.svg"
rm -f "$DATA/metainfo/io.github.realistikdash.Stagehand.metainfo.xml"
rm -rf "${DATA:?}/gnome-shell/extensions/$UUID"
rm -rf "${LIBEXEC:?}/lwe"
rmdir "$LIBEXEC" 2>/dev/null || true
systemctl --user daemon-reload
echo "Removed. Configuration in $CONFIG/stagehand and the still images in the cache were kept."
