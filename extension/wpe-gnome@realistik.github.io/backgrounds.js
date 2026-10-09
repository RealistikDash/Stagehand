// Adds a live clone of each pinned renderer window to every background actor the Shell creates
// (desktop, overview workspaces, workspace thumbnails). Clutter keeps an actor painted while it
// has mapped clones, so the minimised renderer window keeps receiving frames.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';
import {InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Background from 'resource:///org/gnome/shell/ui/background.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const FADE_MS = 400;
// Blur my Shell builds its own BackgroundManagers for static blur; those read the still image.
const SKIPPED_CONTAINER_PREFIX = 'bms-';
const WORKAREA_REFRESH_DELAY_MS = 500;

function monitorIndexFor(connector) {
    return global.backend.get_monitor_manager().get_monitor_for_connector(connector);
}

function unionGeometry(connectors) {
    const monitors = connectors
        .map(connector => Main.layoutManager.monitors[monitorIndexFor(connector)])
        .filter(monitor => monitor !== undefined);

    if (monitors.length === 0)
        return null;

    const x = Math.min(...monitors.map(monitor => monitor.x));
    const y = Math.min(...monitors.map(monitor => monitor.y));
    const right = Math.max(...monitors.map(monitor => monitor.x + monitor.width));
    const bottom = Math.max(...monitors.map(monitor => monitor.y + monitor.height));

    return {x, y, width: right - x, height: bottom - y};
}

class LiveLayer {
    constructor(backgroundActor) {
        this.backgroundActor = backgroundActor;
        this.monitorIndex = backgroundActor.monitor;
        this._clones = new Map();
        this.widget = new St.Widget({
            x_expand: true,
            y_expand: true,
            clip_to_allocation: true,
        });
        this._dimmer = new St.Widget({
            x_expand: true,
            y_expand: true,
            style: 'background-color: black;',
            opacity: 0,
        });
        backgroundActor.layout_manager = new Clutter.BinLayout();
        backgroundActor.add_child(this.widget);
        backgroundActor.add_child(this._dimmer);
        this._destroyId = backgroundActor.connect('destroy', () => this.destroy());
        this.destroyed = false;
    }

    covers(pinned) {
        return pinned.connectors.some(connector => monitorIndexFor(connector) === this.monitorIndex);
    }

    attach(pinned) {
        if (this.destroyed || this._clones.has(pinned.pid) || !this.covers(pinned))
            return;

        const monitor = Main.layoutManager.monitors[this.monitorIndex];
        const bounds = unionGeometry(pinned.connectors);

        if (!monitor || !bounds)
            return;

        const clone = new Clutter.Clone({
            source: pinned.actor,
            x: bounds.x - monitor.x,
            y: bounds.y - monitor.y,
            width: bounds.width,
            height: bounds.height,
            opacity: 0,
        });
        this.widget.add_child(clone);
        this._clones.set(pinned.pid, clone);
        this.update(pinned);
    }

    update(pinned) {
        const clone = this._clones.get(pinned.pid);

        if (!clone)
            return;

        clone.ease({
            opacity: pinned.visible ? 255 : 0,
            duration: FADE_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
        this._dimmer.ease({
            opacity: Math.round(pinned.dim * 255),
            duration: FADE_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
    }

    detach(pinned) {
        const clone = this._clones.get(pinned.pid);

        if (!clone)
            return;

        this._clones.delete(pinned.pid);
        clone.ease({
            opacity: 0,
            duration: FADE_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => {
                clone.set_source(null);
                clone.destroy();
            },
        });

        if (this._clones.size === 0)
            this._dimmer.opacity = 0;
    }

    destroy() {
        if (this.destroyed)
            return;

        this.destroyed = true;

        if (this._destroyId) {
            this.backgroundActor.disconnect(this._destroyId);
            this._destroyId = 0;
        }

        for (const clone of this._clones.values()) {
            clone.remove_all_transitions();
            clone.set_source(null);
            clone.destroy();
        }

        this._clones.clear();
        this.widget.destroy();
        this._dimmer.destroy();
    }
}

export class Backgrounds {
    constructor(registry) {
        this._registry = registry;
        this._injector = new InjectionManager();
        this._layers = new Set();
        this._signals = [];
        this._refreshId = 0;
    }

    enable() {
        const backgrounds = this;

        this._injector.overrideMethod(Background.BackgroundManager.prototype, '_createBackgroundActor', original => {
            return function () {
                const backgroundActor = original.call(this);
                const containerName = this._container?.name ?? '';

                if (!containerName.startsWith(SKIPPED_CONTAINER_PREFIX))
                    backgrounds._addLayer(backgroundActor);

                return backgroundActor;
            };
        });
        this._signals.push([this._registry, this._registry.connect('window-added', (_registry, pinned) => {
            for (const layer of this._layers)
                layer.attach(pinned);
        })]);
        this._signals.push([this._registry, this._registry.connect('window-removed', (_registry, pinned) => {
            for (const layer of this._layers)
                layer.detach(pinned);
        })]);
        this._signals.push([this._registry, this._registry.connect('window-changed', (_registry, pinned) => {
            for (const layer of this._layers)
                layer.update(pinned);
        })]);
        this._reloadBackgrounds();
    }

    disable() {
        for (const [emitter, id] of this._signals)
            emitter.disconnect(id);

        this._signals = [];

        if (this._refreshId) {
            GLib.source_remove(this._refreshId);
            this._refreshId = 0;
        }

        for (const layer of this._layers)
            layer.destroy();

        this._layers.clear();
        this._injector.clear();
        this._reloadBackgrounds();
    }

    _addLayer(backgroundActor) {
        const layer = new LiveLayer(backgroundActor);
        this._layers.add(layer);
        backgroundActor.connect('destroy', () => this._layers.delete(layer));

        for (const pinned of this._registry.windows())
            layer.attach(pinned);
    }

    _reloadBackgrounds() {
        global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            Main.layoutManager._updateBackgrounds();
            Main.overview._overview._controls._workspacesDisplay._updateWorkspacesViews();

            return GLib.SOURCE_REMOVE;
        });

        if (this._refreshId)
            GLib.source_remove(this._refreshId);

        this._refreshId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, WORKAREA_REFRESH_DELAY_MS, () => {
            this._refreshId = 0;
            global.display.emit('workareas-changed');

            return GLib.SOURCE_REMOVE;
        });
    }
}
