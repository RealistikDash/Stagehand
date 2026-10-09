// The object the daemon talks to, exported on GNOME Shell's own bus name.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const OBJECT_PATH = '/io/github/realistik/WpeGnome/Shell';
const INTERFACE_VERSION = 1;
const INTERFACE_XML = `
<node>
  <interface name="io.github.realistik.WpeGnome.Shell">
    <method name="Pin">
      <arg type="u" name="pid" direction="in"/>
      <arg type="as" name="connectors" direction="in"/>
    </method>
    <method name="Unpin">
      <arg type="u" name="pid" direction="in"/>
    </method>
    <method name="SetVisible">
      <arg type="u" name="pid" direction="in"/>
      <arg type="b" name="visible" direction="in"/>
    </method>
    <method name="SetDim">
      <arg type="u" name="pid" direction="in"/>
      <arg type="d" name="level" direction="in"/>
    </method>
    <method name="CaptureStill">
      <arg type="u" name="pid" direction="in"/>
      <arg type="s" name="filename" direction="in"/>
    </method>
    <property name="Version" type="u" access="read"/>
    <property name="OcclusionStates" type="a{ss}" access="read"/>
    <signal name="WindowPinned">
      <arg type="u" name="pid"/>
    </signal>
    <signal name="WindowLost">
      <arg type="u" name="pid"/>
    </signal>
    <signal name="OcclusionChanged">
      <arg type="s" name="connector"/>
      <arg type="s" name="state"/>
    </signal>
    <signal name="Ready"/>
  </interface>
</node>`;
// Only in a devkit session (tools/nested.sh): lets the test harness capture the nested stage.
const DEVKIT_INTERFACE_XML = INTERFACE_XML.replace('<signal name="Ready"/>', `
    <method name="Screenshot">
      <arg type="s" name="filename" direction="in"/>
    </method>
    <method name="ToggleOverview"/>
    <method name="Describe">
      <arg type="s" name="json" direction="out"/>
    </method>
    <signal name="Ready"/>`);

export class ShellService {
    constructor(registry, occlusion) {
        this._registry = registry;
        this._occlusion = occlusion;
        this._exported = null;
        this._signals = [];
    }

    export() {
        const xml = GLib.getenv('WPE_GNOME_DEVKIT') ? DEVKIT_INTERFACE_XML : INTERFACE_XML;
        this._exported = Gio.DBusExportedObject.wrapJSObject(xml, this);
        this._exported.export(Gio.DBus.session, OBJECT_PATH);
        this._signals.push([this._registry, this._registry.connect('window-added', (_registry, pinned) => {
            this._emit('WindowPinned', new GLib.Variant('(u)', [pinned.pid]));
        })]);
        this._signals.push([this._registry, this._registry.connect('window-lost', (_registry, pid) => {
            this._emit('WindowLost', new GLib.Variant('(u)', [pid]));
        })]);
        this._signals.push([this._occlusion, this._occlusion.connect('changed', (_occlusion, connector, state) => {
            this._emit('OcclusionChanged', new GLib.Variant('(ss)', [connector, state]));
        })]);
        this._emit('Ready', null);
    }

    unexport() {
        for (const [emitter, id] of this._signals)
            emitter.disconnect(id);

        this._signals = [];
        this._exported?.unexport();
        this._exported = null;
    }

    get Version() {
        return INTERFACE_VERSION;
    }

    get OcclusionStates() {
        return this._occlusion.snapshot();
    }

    Pin(pid, connectors) {
        this._registry.pin(pid, connectors);
    }

    Unpin(pid) {
        this._registry.unpin(pid);
    }

    SetVisible(pid, visible) {
        if (!this._registry.setVisible(pid, visible))
            throw new Gio.DBusError({code: Gio.DBusError.INVALID_ARGS, message: 'No such pinned window'});
    }

    SetDim(pid, level) {
        if (!this._registry.setDim(pid, level))
            throw new Gio.DBusError({code: Gio.DBusError.INVALID_ARGS, message: 'No such pinned window'});
    }

    Screenshot(filename) {
        const screenshot = new Shell.Screenshot();
        const stream = Gio.File.new_for_path(filename).replace(null, false, Gio.FileCreateFlags.NONE, null);
        screenshot.screenshot(false, stream, (_source, result) => {
            screenshot.screenshot_finish(result);
            stream.close(null);
        });
    }

    ToggleOverview() {
        Main.overview.toggle();
    }

    Describe() {
        const windows = this._registry.windows().map(pinned => {
            const rect = pinned.metaWindow.get_frame_rect();
            const buffer = pinned.metaWindow.get_buffer_rect();
            const surface = pinned.actor.get_first_child();

            return {
                pid: pinned.pid,
                connectors: pinned.connectors,
                frame: [rect.x, rect.y, rect.width, rect.height],
                buffer: [buffer.x, buffer.y, buffer.width, buffer.height],
                actor: [pinned.actor.width, pinned.actor.height],
                actorVisible: pinned.actor.visible,
                actorMapped: pinned.actor.mapped,
                surface: surface ? [surface.width, surface.height] : null,
                minimized: pinned.metaWindow.minimized,
                title: pinned.metaWindow.title,
                wmClass: pinned.metaWindow.get_wm_class(),
            };
        });

        return JSON.stringify(windows);
    }

    // Writes the renderer's current frame as PNG; the daemon publishes it for static consumers
    // (lock screen, static blur) instead of the square Workshop preview.
    CaptureStill(pid, filename) {
        if (!this._registry.captureStill(pid, filename))
            throw new Gio.DBusError({code: Gio.DBusError.FAILED, message: 'No image for that window'});
    }

    _emit(name, variant) {
        this._exported?.emit_signal(name, variant);
    }
}
