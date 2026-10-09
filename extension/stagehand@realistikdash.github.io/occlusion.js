// Reports, per monitor, whether a fullscreen or maximised window covers the background on the
// active workspace. The daemon decides what to do with that.

import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Signals from 'resource:///org/gnome/shell/misc/signals.js';

const STATE_NONE = 'none';
const STATE_MAXIMISED = 'maximised';
const STATE_FULLSCREEN = 'fullscreen';
const RECOMPUTE_DELAY_MS = 150;

export class Occlusion extends Signals.EventEmitter {
    constructor(registry, allWindowActors) {
        super();
        this._registry = registry;
        this._allWindowActors = allWindowActors;
        this._states = new Map();
        this._windowSignals = new Map();
        this._signals = [];
        this._recomputeId = 0;
    }

    enable() {
        const schedule = () => this._schedule();
        this._signals.push([global.display, global.display.connect('window-created', (_display, window) => {
            this._track(window);
            schedule();
        })]);
        this._signals.push([global.window_manager, global.window_manager.connect('switch-workspace', schedule)]);
        this._signals.push([global.window_manager, global.window_manager.connect('minimize', schedule)]);
        this._signals.push([global.window_manager, global.window_manager.connect('unminimize', schedule)]);
        this._signals.push([global.window_manager, global.window_manager.connect('size-changed', schedule)]);
        this._signals.push([global.window_manager, global.window_manager.connect('map', schedule)]);
        this._signals.push([global.window_manager, global.window_manager.connect('destroy', schedule)]);
        this._signals.push([Main.overview, Main.overview.connect('showing', schedule)]);
        this._signals.push([Main.overview, Main.overview.connect('hidden', schedule)]);
        this._signals.push([Main.layoutManager, Main.layoutManager.connect('monitors-changed', schedule)]);

        for (const actor of this._allWindowActors()) {
            const window = actor.get_meta_window();

            if (window)
                this._track(window);
        }

        this._schedule();
    }

    disable() {
        for (const [emitter, id] of this._signals)
            emitter.disconnect(id);

        this._signals = [];

        for (const [window, ids] of this._windowSignals) {
            for (const id of ids)
                window.disconnect(id);
        }

        this._windowSignals.clear();

        if (this._recomputeId) {
            GLib.source_remove(this._recomputeId);
            this._recomputeId = 0;
        }

        this._states.clear();
    }

    snapshot() {
        return Object.fromEntries(this._states);
    }

    _track(window) {
        if (this._windowSignals.has(window))
            return;

        const schedule = () => this._schedule();
        const ids = [
            window.connect('notify::fullscreen', schedule),
            window.connect('notify::maximized-horizontally', schedule),
            window.connect('notify::maximized-vertically', schedule),
            window.connect('notify::minimized', schedule),
            window.connect('position-changed', schedule),
            window.connect('workspace-changed', schedule),
            window.connect('unmanaged', () => {
                const tracked = this._windowSignals.get(window) ?? [];

                for (const id of tracked)
                    window.disconnect(id);

                this._windowSignals.delete(window);
                schedule();
            }),
        ];
        this._windowSignals.set(window, ids);
    }

    _schedule() {
        if (this._recomputeId)
            return;

        this._recomputeId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, RECOMPUTE_DELAY_MS, () => {
            this._recomputeId = 0;
            this._recompute();

            return GLib.SOURCE_REMOVE;
        });
    }

    _recompute() {
        const monitorManager = global.backend.get_monitor_manager();
        const workspace = global.workspace_manager.get_active_workspace();
        const covering = new Map();

        if (!Main.overview.visible) {
            for (const actor of this._allWindowActors()) {
                const window = actor.get_meta_window();

                if (!window || this._registry.isPinned(window) || window.minimized)
                    continue;

                if (!window.located_on_workspace(workspace))
                    continue;

                const index = window.get_monitor();
                const state = window.is_fullscreen()
                    ? STATE_FULLSCREEN
                    : window.is_maximized() ? STATE_MAXIMISED : STATE_NONE;

                if (state === STATE_NONE)
                    continue;

                if (state === STATE_FULLSCREEN || covering.get(index) !== STATE_FULLSCREEN)
                    covering.set(index, state);
            }
        }

        for (const monitor of monitorManager.get_monitors()) {
            const connector = monitor.get_connector();
            const index = monitorManager.get_monitor_for_connector(connector);
            const state = covering.get(index) ?? STATE_NONE;

            if (this._states.get(connector) === state)
                continue;

            this._states.set(connector, state);
            this.emit('changed', connector, state);
        }
    }
}
