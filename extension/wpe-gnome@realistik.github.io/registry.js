// Tracks which windows belong to pinned renderer processes. Windows are recognised by the PID of
// their Wayland client, which the daemon announces before the window maps.

import GLib from 'gi://GLib';
import * as Signals from 'resource:///org/gnome/shell/misc/signals.js';

// Mutter hides windows asynchronously; re-check shortly after minimising.
const MINIMISE_RESYNC_DELAY_MS = 250;

class PinnedWindow {
    constructor(pid, connectors, metaWindow, actor) {
        this.pid = pid;
        this.connectors = connectors;
        this.metaWindow = metaWindow;
        this.actor = actor;
        this.visible = true;
        this.dim = 0;
        this._shown = false;
        this._signals = [];
        this._resyncId = 0;
    }

    // The window must go through mutter's show path once before it is minimised: a window that
    // is minimised before it was ever shown keeps a frozen surface and never receives frames.
    hold() {
        const window = this.metaWindow;
        this._signals.push(window.connect_after('shown', () => {
            this._shown = true;
            this._keepMinimised();
        }));
        this._signals.push(window.connect('notify::minimized', () => {
            if (this._shown)
                this._keepMinimised();
        }));
        this._signals.push(window.connect_after('raised', () => window.lower()));
        this._signals.push(window.connect('notify::above', () => {
            if (window.above)
                window.unmake_above();
        }));

        if (this.actor.visible) {
            this._shown = true;
            this._keepMinimised();
        }
    }

    _keepMinimised() {
        const window = this.metaWindow;

        if (window.above)
            window.unmake_above();

        if (window.demands_attention)
            window.unset_demands_attention();

        if (!window.minimized)
            window.minimize();

        if (this._resyncId)
            return;

        this._resyncId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, MINIMISE_RESYNC_DELAY_MS, () => {
            this._resyncId = 0;

            if (window.minimized && this.actor?.visible) {
                window.unminimize();
                window.minimize();
            }

            return GLib.SOURCE_REMOVE;
        });
    }

    release() {
        for (const id of this._signals)
            this.metaWindow.disconnect(id);

        this._signals = [];

        if (this._resyncId) {
            GLib.source_remove(this._resyncId);
            this._resyncId = 0;
        }
    }
}

export class Registry extends Signals.EventEmitter {
    constructor() {
        super();
        this._expected = new Map();
        this._windows = new Map();
        this._byWindow = new Map();
        this._windowCreatedId = 0;
        this._mapId = 0;
        this._allWindowActors = () => [];
    }

    enable(allWindowActors) {
        this._allWindowActors = allWindowActors;
        this._windowCreatedId = global.display.connect('window-created', (_display, window) => {
            this._tryAdopt(window);
        });
        this._mapId = global.window_manager.connect('map', (_wm, actor) => {
            const window = actor.get_meta_window();

            if (window)
                this._tryAdopt(window);
        });
    }

    disable() {
        if (this._windowCreatedId) {
            global.display.disconnect(this._windowCreatedId);
            this._windowCreatedId = 0;
        }

        if (this._mapId) {
            global.window_manager.disconnect(this._mapId);
            this._mapId = 0;
        }

        for (const pid of [...this._windows.keys()])
            this.unpin(pid);

        this._expected.clear();
    }

    pin(pid, connectors) {
        this._expected.set(pid, connectors);

        for (const actor of this._allWindowActors()) {
            const window = actor.get_meta_window();

            if (window)
                this._tryAdopt(window);
        }
    }

    unpin(pid) {
        this._expected.delete(pid);
        const pinned = this._windows.get(pid);

        if (!pinned)
            return;

        this._windows.delete(pid);
        this._byWindow.delete(pinned.metaWindow);
        pinned.release();
        this.emit('window-removed', pinned);
    }

    setVisible(pid, visible) {
        const pinned = this._windows.get(pid);

        if (!pinned)
            return false;

        pinned.visible = visible;
        this.emit('window-changed', pinned);

        return true;
    }

    setDim(pid, level) {
        const pinned = this._windows.get(pid);

        if (!pinned)
            return false;

        pinned.dim = Math.min(Math.max(level, 0), 1);
        this.emit('window-changed', pinned);

        return true;
    }

    isPinned(metaWindow) {
        return this._byWindow.has(metaWindow);
    }

    windows() {
        return [...this._windows.values()];
    }

    _tryAdopt(window) {
        if (this._byWindow.has(window))
            return;

        const pid = window.get_pid();
        const connectors = this._expected.get(pid);

        if (!connectors || this._windows.has(pid))
            return;

        const actor = window.get_compositor_private();

        if (!actor)
            return;

        const pinned = new PinnedWindow(pid, connectors, window, actor);
        this._windows.set(pid, pinned);
        this._byWindow.set(window, pinned);
        pinned.hold();
        pinned.unmanagedId = window.connect('unmanaged', () => {
            if (this._windows.get(pid) !== pinned)
                return;

            this._windows.delete(pid);
            this._byWindow.delete(window);
            pinned.release();
            this.emit('window-removed', pinned);
            this.emit('window-lost', pid);
        });
        console.log('[wpe-gnome] Adopted renderer window.');
        this.emit('window-added', pinned);
    }
}
