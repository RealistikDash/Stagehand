#!/bin/bash
# Inner half of tools/nested.sh: runs inside the private D-Bus session.
set -uo pipefail

cleanup() {
    kill -TERM "$DAEMON_PID" 2>/dev/null || true
    for _ in 1 2 3 4 5 6; do
        kill -0 "$DAEMON_PID" 2>/dev/null || break
        sleep 0.5
    done
    kill -KILL "$DAEMON_PID" 2>/dev/null || true
    pkill -KILL -f 'stagehand-renderer --arguments' 2>/dev/null || true
    kill -TERM "$SHELL_PID" 2>/dev/null || true
    wait "$SHELL_PID" 2>/dev/null || true
    # A Shell killed before it finished starting leaves this marker and the next Shell start
    # (the live one!) would disable all extensions.
    rm -f "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/gnome-shell-disable-extensions"
}
trap cleanup EXIT

# shellcheck disable=SC2086
gnome-shell --devkit ${STAGEHAND_SHELL_ARGS:-} >"$STAGEHAND_LOG" 2>&1 &
SHELL_PID=$!

for _ in $(seq 1 60); do
    if gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner org.gnome.Shell 2>/dev/null | grep -q true; then
        break
    fi
    sleep 0.5
done

WAYLAND=$(grep -oE "Using Wayland display name '[^']+'" "$STAGEHAND_LOG" | head -1 | sed "s/.*'\(.*\)'/\1/")
export WAYLAND_DISPLAY="${WAYLAND:-wayland-1}"
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=GNOME
sleep 3
gnome-extensions enable "$STAGEHAND_UUID"
sleep 1
gnome-extensions info "$STAGEHAND_UUID" | grep -E 'State|Enabled' || true

SCENE="${STAGEHAND_SCENE_RENDERER:-$HOME/.local/libexec/stagehand/lwe/linux-wallpaperengine}"
RUST_LOG="${RUST_LOG:-debug}" "$STAGEHAND_ROOT/target/${STAGEHAND_PROFILE:-debug}/stagehandd" --scene-renderer "$SCENE" >"$STAGEHAND_LOG.daemon" 2>&1 &
DAEMON_PID=$!
sleep 2
echo "Nested Shell pid $SHELL_PID, daemon pid $DAEMON_PID, Wayland $WAYLAND_DISPLAY, log $STAGEHAND_LOG"

if [ $# -gt 0 ]; then
    "$@"
else
    bash
fi
