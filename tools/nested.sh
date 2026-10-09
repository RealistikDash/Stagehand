#!/bin/bash
# Runs a nested GNOME Shell (devkit) on a private D-Bus session with a private dconf database,
# installs the extension into it, starts the daemon against it and leaves a shell for testing.
#
# Nothing here touches the live session: the extension is enabled only in the private dconf
# profile, and the daemon talks to the nested Shell only.
#
# Usage: tools/nested.sh [command...]   (default: an interactive bash)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UUID="stagehand@realistikdash.github.io"
EXT_SRC="$ROOT/extension/$UUID"
EXT_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$UUID"
LOG="${STAGEHAND_NESTED_LOG:-/tmp/stagehand-nested.log}"

mkdir -p "$(dirname "$EXT_DIR")"
rm -rf "$EXT_DIR"
cp -r "$EXT_SRC" "$EXT_DIR"

export DCONF_PROFILE="$ROOT/tools/nested-dconf-profile"
export STAGEHAND_DEVKIT=1
export GTK_A11Y=none
export NO_AT_BRIDGE=1
export STAGEHAND_ROOT="$ROOT"
export STAGEHAND_UUID="$UUID"
export STAGEHAND_LOG="$LOG"

exec dbus-run-session -- bash "$ROOT/tools/nested-inner.sh" "$@"
