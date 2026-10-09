// wpe-gnome Shell extension: the one non-Rust component. It only places renderer windows into the
// background and reports Shell state over D-Bus; every decision lives in the daemon.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Backgrounds} from './backgrounds.js';
import {ShellService} from './dbus.js';
import {Occlusion} from './occlusion.js';
import {Overrides} from './overrides.js';
import {Registry} from './registry.js';

export default class WpeGnomeExtension extends Extension {
    enable() {
        this._registry = new Registry();
        this._overrides = new Overrides(this._registry);
        this._backgrounds = new Backgrounds(this._registry, () => this._overrides.allWindowActors());
        this._occlusion = new Occlusion(this._registry, () => this._overrides.allWindowActors());
        this._service = new ShellService(this._registry, this._occlusion);

        // Overrides first: they capture the unfiltered window list the other parts rely on.
        this._overrides.enable();
        this._registry.enable(() => this._overrides.allWindowActors());
        this._backgrounds.enable();
        this._occlusion.enable();
        this._service.export();
        console.log('[wpe-gnome] Extension enabled.');
    }

    disable() {
        this._service?.unexport();
        this._occlusion?.disable();
        this._backgrounds?.disable();
        this._registry?.disable();
        this._overrides?.disable();
        this._service = null;
        this._occlusion = null;
        this._backgrounds = null;
        this._registry = null;
        this._overrides = null;
        console.log('[wpe-gnome] Extension disabled.');
    }
}
