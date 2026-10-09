// Keeps pinned windows out of every window list the Shell builds: Alt-Tab, the overview,
// workspace thumbnails, the dash and the app tracker.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Workspace from 'resource:///org/gnome/shell/ui/workspace.js';
import * as WorkspaceThumbnail from 'resource:///org/gnome/shell/ui/workspaceThumbnail.js';

export class Overrides {
    constructor(registry) {
        this._registry = registry;
        this._injector = new InjectionManager();
        this._originalWindowActors = () => global.get_window_actors();
    }

    allWindowActors() {
        return this._originalWindowActors();
    }

    enable() {
        const registry = this._registry;
        const isPinned = window => window !== null && registry.isPinned(window);

        this._injector.overrideMethod(Shell.Global.prototype, 'get_window_actors', original => {
            this._originalWindowActors = () => original.call(global);

            return function () {
                return original.call(this).filter(actor => !isPinned(actor.meta_window));
            };
        });
        this._injector.overrideMethod(Meta.Display.prototype, 'get_tab_list', original => {
            return function (type, workspace) {
                return original.call(this, type, workspace).filter(window => !isPinned(window));
            };
        });
        this._injector.overrideMethod(Workspace.Workspace.prototype, '_isOverviewWindow', original => {
            return function (window) {
                return isPinned(window) ? false : original.call(this, window);
            };
        });
        this._injector.overrideMethod(WorkspaceThumbnail.WorkspaceThumbnail.prototype, '_isOverviewWindow', original => {
            return function (actor) {
                return isPinned(actor.get_meta_window()) ? false : original.call(this, actor);
            };
        });
        this._injector.overrideMethod(Shell.WindowTracker.prototype, 'get_window_app', original => {
            return function (window) {
                return isPinned(window) ? null : original.call(this, window);
            };
        });
        this._injector.overrideMethod(Shell.App.prototype, 'get_windows', original => {
            return function () {
                return original.call(this).filter(window => !isPinned(window));
            };
        });
        this._injector.overrideMethod(Shell.App.prototype, 'get_n_windows', () => {
            return function () {
                return this.get_windows().length;
            };
        });
        this._injector.overrideMethod(Shell.AppSystem.prototype, 'get_running', original => {
            return function () {
                return original.call(this).filter(app => app.get_n_windows() > 0);
            };
        });
        // A window that maps without focus is flagged as demanding attention, and the Shell
        // announces it as "ready" unless the window skips the taskbar.
        this._injector.overrideMethod(Meta.Window.prototype, 'is_skip_taskbar', original => {
            return function () {
                return isPinned(this) || original.call(this);
            };
        });
    }

    disable() {
        this._injector.clear();
        this._originalWindowActors = () => global.get_window_actors();
    }
}
