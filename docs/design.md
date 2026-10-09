# wpe-gnome design note

Live Wallpaper Engine wallpapers as the real GNOME desktop background, on Wayland.

## 1. The machine (verified 2026-10-09)

| Item | Assumed in the brief | Verified |
|---|---|---|
| OS | Fedora 44 Workstation | Fedora 44 Workstation, kernel 7.2.8 |
| Shell | GNOME 50, Wayland only | GNOME Shell 50.5, mutter 50.5 (`libmutter-18`), session type `wayland`, Xwayland running rootless for legacy apps only |
| GPU | – | AMD Radeon RX 7900 XTX, `amdgpu`, Mesa 26.2.3 (radeonsi GL 4.6, RADV Vulkan 1.4) |
| VA-API | "where available" | **Not available out of the box.** Fedora's `mesa-va-drivers` was not installed and the Fedora build has no H.264/HEVC anyway. RPM Fusion is enabled, so `mesa-va-drivers-freeworld` was installed; `vah264dec`/`vah265dec` now exist. |
| Monitors | OLED | One monitor: `DP-1`, Samsung Odyssey G85SB (`SAM` / `H1AK500000`), 3440x1440 at 175 Hz, scale 1.0. Connectors DP-2, DP-3, HDMI-A-1 are free, so hotplug can be tested. |
| Extensions | Blur my Shell | Enabled: `blur-my-shell@aunetx` (v72), `hidetopbar@mathieu.bidon.ca`, `background-logo@fedorahosted.org`. Installed but disabled: window-list, apps-menu, places-menu, launch-new-instance, appindicator. No Dash to Dock, no desktop icons. |
| Background keys | – | `picture-uri` and `picture-uri-dark` both `file:///home/realistik/.config/background`, `picture-options` `zoom`, colour scheme `prefer-dark`. These are saved before the first apply and restored on uninstall. |
| Rust | – | Stable 1.98.1 via rustup in the user's home. Nightly `rustfmt` installed for the import options. |
| Wallpaper Engine | `~/.local/share/Steam/steamapps/common/wallpaper_engine` | Present, with `assets/`, `projects/defaultprojects` (19 projects) and an empty `projects/myprojects`. |
| Workshop | `~/.local/share/Steam/steamapps/workshop/content/431960/<id>` | 27 items, 873 MB. By `type`: 15 `scene`, 7 `video`, 4 `web` (one spelled `Web`), 1 broken (`781890571`: `"general": null`). One scene (`843532366`) uses `gifscene.json` as its entry. |
| Second library | `libraryfolders.vdf` | Lists `/run/media/realistik/ishowspeed/SteamLibrary`, which is **not mounted**. Discovery must skip unmounted libraries silently and rescan when they appear. |
| Nested Shell | `--devkit` or `--nested` | `gnome-shell --devkit` exists in 50.5 but needs the separate `mutter-devkit` package (`/usr/libexec/mutter-devkit`), which was not installed. It is now. `--nested` is gone. |
| Audio | – | PipeWire 1.6.9 with the PulseAudio compatibility server. |
| Power | – | UPower reports no battery (desktop machine); the only device is a mouse battery. The battery rule is implemented but cannot be exercised here. |

Where reality differs from the brief: VA-API needed RPM Fusion packages, the devkit needs an extra package, the second Steam library is offline, and the machine has one monitor only. Everything else matched.

Two safety notes discovered on the way:

- A `gnome-shell` that is killed before it finishes starting leaves `/run/user/1000/gnome-shell-disable-extensions` behind, and the **next** Shell start then disables all extensions. The nested devkit Shell does this too. `just nested` therefore stops the devkit with SIGTERM and removes the marker if it is still there afterwards.
- `--devkit` must run under `dbus-run-session`, otherwise it would register with the live session bus and fight the real Shell for names.

## 2. Renderer evaluation

### linux-wallpaperengine (scenes)

Built from `Almamu/linux-wallpaperengine` master (commit `b016d7d`, June 2026) with CMake + Ninja in about four minutes. Master now bundles a CEF download (used for its own web support), QuickJS, glslang and SPIRV-Cross; the Fedora packages it needs are listed in the README.

Results on this machine:

- `--window 0x0x1280x540 --screenshot out.png <id>` produced correct frames for `1536391368` (Minimal Mystic Lake), `2954387038` (Hu Tao) and `1195626192` (Gaze). Rendering goes through GLFW, so in `--window` mode the program is a plain `xdg_toplevel` Wayland client, which is exactly what the extension needs.
- `--list-properties` prints every `general.properties` entry with type, text and current value; `--set-property name=value` is accepted repeatedly. Properties cannot be changed while the process runs, so a scene property change is a renderer restart (crossfaded by the extension).
- Useful flags: `--fps`, `--silent`, `--volume`, `--noautomute`, `--no-fullscreen-pause` (we do our own pausing), `--scaling fill|fit|stretch|default`, `--assets-dir`, `--disable-mouse`.
- It does not set a Wayland `app_id` (GLFW only sets the X11 class), so the window cannot be recognised by `wm_class`. The extension therefore recognises renderer windows by **client PID**, which `Meta.Window.get_pid()` provides for Wayland clients.
- Licence: GPL-3.0. It is executed as a separate process with its own command line; nothing links against it, so this project keeps its own licence (MIT). The build is not vendored; `just lwe` builds it from a pinned commit into `~/.local/libexec/wpe-gnome/`.

No better maintained fork was found; master is active (last commit September 2026 on the Hanabi side, June 2026 here) and already has the Wayland and property features the brief asked for.

### Video and web

- Video: GStreamer 1.28.7 with the `gtk4paintablesink` from `gst-plugins-rs` (`gstreamer1-plugin-gtk4` 0.15.2 is a Fedora package). `playbin3` picks `vah264dec`/`vah265dec` after the RPM Fusion driver install. Seamless looping uses segment seeks, not EOS.
- Web: WebKitGTK 6.0 (2.54.1) in the same GTK renderer binary. The property and audio listener shims are injected as a user script at document start.

Both live in one Rust binary, `wpe-gnome-renderer`, with a plug-in per kind. This is the "single renderer" variant the brief allowed, with scenes still delegated to linux-wallpaperengine. The one-binary design keeps the window handshake, pause protocol and sizing identical for video and web.

### Prior art: Hanabi

Read `jeffshee/gnome-ext-hanabi` (GNOME 50 branch). The technique that matters:

1. The renderer window is a normal `xdg_toplevel`; the extension keeps it **minimised** so it never shows as a window.
2. `Background.BackgroundManager.prototype._createBackgroundActor` is overridden to add a `Clutter.Clone` of the renderer's window actor inside every background actor. Clutter keeps an actor mapped while it has mapped clones, so a minimised window keeps receiving frame callbacks and the clone stays live. Because every background in the Shell (desktop, overview workspaces, workspace thumbnails) is built through `BackgroundManager`, the live wallpaper appears in all of them without further work.
3. The window is hidden from `Meta.Display.get_tab_list`, `Shell.Global.get_window_actors`, `Workspace._isOverviewWindow`, `WorkspaceThumbnail._isOverviewWindow` and the app tracker (`Shell.WindowTracker.get_window_app`, `Shell.App.get_windows`, `Shell.AppSystem.get_running`) with `InjectionManager` overrides.

Hanabi identifies its windows by a JSON blob in the title and spawns the renderer itself with `Meta.WaylandClient`. We cannot do either (the daemon owns renderers and linux-wallpaperengine's title is not ours), so identification is by PID, handed over D-Bus before the window maps (see §3.4). Checked against the 50.5 sources extracted from `libshell-18.so`: all hook points still exist and `_isOverviewWindow` is still `!window.skip_taskbar`.

## 3. Architecture

```
wpe-gnome (GTK4/libadwaita UI) ──D-Bus──▶ wpe-gnomed (daemon) ──spawn──▶ linux-wallpaperengine (scene)
wpe-gnomectl (CLI)             ──D-Bus──▶     │                 ──spawn──▶ wpe-gnome-renderer (video | web)
                                              │
                                              └──D-Bus──▶ GNOME Shell extension (GJS)
                                                          pins renderer windows into every background
```

### 3.1 Crate graph (one-way)

```
wpe-gnome-core        pure domain: models, project.json parsing, Steam layout, config, traits
      ▲
wpe-gnome-ipc         D-Bus wire types and the two interface definitions (zbus)
      ▲          ▲
wpe-gnomed       wpe-gnome (UI), wpe-gnomectl (CLI)
wpe-gnome-renderer  depends on core only (protocol types)
```

- `wpe-gnome-core` has no GTK, no D-Bus, no GStreamer. It defines the boundary traits (`MonitorSource`, `RendererLauncher`, `BackgroundPinner`, `StillImageSink`, `SessionSignals`, `PowerSource`) and the repositories (`WorkshopRepository`, `ConfigRepository`). Tests and the CLI use it without a session.
- `wpe-gnome-ipc` is the only crate that both sides of the bus depend on. It owns the zbus proxies and the `zvariant` wire structs and converts them to core models.
- `wpe-gnomed` implements the traits with real systems (Mutter `DisplayConfig`, child processes, the extension proxy, GSettings through `gio`, `org.gnome.ScreenSaver`, UPower) and owns the orchestration.
- `wpe-gnome-renderer` is a GTK program; it only shares the stdin protocol types with core.
- `wpe-gnome` is a client of the daemon's D-Bus API. Closing it changes nothing.

### 3.2 Runtime choice

- The daemon runs on `smol` (`async-io` + `async-executor`). zbus 5 uses `async-io` by default, so no second runtime is needed and the whole process has one executor. `notify` runs its own inotify thread and hands events to the executor through an `async-channel`. Children are `async-process` commands.
- The GTK processes use the GLib main loop only. zbus futures with the `async-io` backend are executor-agnostic, so the UI awaits them with `glib::spawn_future_local`. No tokio anywhere.
- GStreamer in the renderer uses its bus watch on the GLib main context.

### 3.3 Daemon state machine

The daemon is one actor: a `Service` that owns the configuration, the library, the monitor list, the renderer table and the pause state, and processes `Command`s from a single channel. D-Bus handlers, the extension's signals, the Steam directory watcher and the monitor watcher all send commands; nothing else touches the state. Every command is handled inside its own error scope so one failing wallpaper or monitor never stops the loop.

Per monitor (or for the span group) the daemon keeps a `Slot`:

```
Empty ──apply──▶ Starting(child, gate held) ──Pin acked──▶ Running ──pause rule──▶ Paused(SIGSTOP) ──resume──▶ Running
                                                                      └──timeout──▶ Paused+Hidden (clone hidden, still shows)
```

### 3.4 Window handshake

The daemon must tell the extension about a renderer PID before its window maps, otherwise the window would be visible and focused for a frame. Spawning gives us the PID only after exec, so every renderer is started as

```
/bin/sh -c 'read _ && exec "$@"' wpe-gate <renderer argv...>
```

with stdin piped. The shell's PID is the renderer's PID after `exec`. The daemon calls `Pin(pid, connectors)` on the extension, waits for the reply, then writes the "go" line. If the extension is not running the gate is never opened and nothing is spawned at all, so a missing extension never results in stray windows.

The extension side:

- `Pin(pid, connectors, size)` records the expectation; when a window with that PID maps it is minimised, kept minimised and at the bottom, and removed from every switcher through the overrides listed in §2.
- One `Clutter.Clone` of the window actor is added to every `BackgroundManager` background actor whose monitor is in `connectors`. A spanned window is cloned into each of its monitors with an offset, clipped to the monitor.
- `SetVisible(pid, bool)` fades the clones in or out (used for the OLED timeout: when the clone is hidden, the still image below shows).
- `SetDim(pid, level)` darkens the clones.
- `Unpin(pid)` tears everything down, and `disable()` tears everything down.
- The extension emits `WindowPinned(pid)`, `WindowLost(pid)` and `Occlusion(connector, occluded)`. Occlusion is "a non-minimised window on the active workspace is fullscreen, or maximised in both directions, on this monitor". That is placement state the Shell knows; what to do about it is the daemon's decision.

Blur my Shell creates its own `BackgroundManager`s for *static* blur. Those managers are skipped (their container actors are named `bms-*`), so static blur reads the still frame as the brief requires. Dynamic blur reads the framebuffer behind the panel, which now contains the clone, so it samples the live wallpaper. See §5 for the verification.

### 3.5 Pause rules and OLED care

Pausing is `SIGSTOP` to the renderer's process group (the renderer, its CEF or WebKit helpers, everything). While stopped a client submits no frames and uses no CPU or GPU; the last buffer stays on screen through the clone. Resume is `SIGCONT`. Before stopping its own renderer the daemon sends a `pause` command on stdin so audio stops cleanly; linux-wallpaperengine has no such channel and is simply stopped.

Inputs: extension occlusion signals (fullscreen / maximised), `org.gnome.ScreenSaver.ActiveChanged` (lock and blank), `org.gnome.SessionManager.Presence` (idle), UPower `OnBattery`. All default on except battery. After `paused_still_timeout` seconds in the paused state the clone is hidden so the still image is shown; the screen is never inhibited.

### 3.6 Identity of monitors

`org.gnome.Mutter.DisplayConfig.GetCurrentState` gives connector, vendor, product and serial per monitor. Assignments are stored under `vendor/product/serial` with the connector as a fallback key, so a cable swap keeps the assignment and two identical panels without serials fall back to connectors.

### 3.7 Still image for static consumers

On every apply the daemon writes `$XDG_CACHE_HOME/wpe-gnome/still-<connector>.png` from the wallpaper's preview (GIF previews use their first frame) and sets `picture-uri` and `picture-uri-dark` to the primary monitor's still. The original values are saved into the config on the first apply and restored by `wpe-gnomectl restore-background`, which `just uninstall` runs.

## 4. Milestones

1. Discovery and CLI listing (`wpe-gnomectl list`).
2. Daemon, renderer (video) and extension pinning in the nested devkit Shell.
3. Scenes through linux-wallpaperengine.
4. Web through WebKitGTK with the property listener.
5. UI.
6. Autorun, pause rules, Blur my Shell verification, polish and measurements.

## 5. Verification (nested devkit Shell, 2026-10-09)

Everything below was exercised through `tools/nested.sh`: a `gnome-shell --devkit` instance on a
private D-Bus session with a private dconf profile, the extension installed into it, and the daemon
and CLI driving it. Screenshots were taken with the extension's devkit-only `Screenshot` method.

| Check | Result |
|---|---|
| Video wallpaper pinned behind the desktop | Yes. The window is minimised, its clone fills the background actor; the overview shows it as the workspace background and lists no renderer window. |
| Scene wallpaper (linux-wallpaperengine, `--window`) | Yes, after the fix in §5.1. Hu Tao (`2954387038`) renders at monitor size and animates. |
| Web wallpaper (WebKitGTK) | Yes. Solar System (`793602574`) loads, receives its properties and animates. |
| Live updates | Screenshot pairs 4 s apart differ for all three kinds (video 34 %, scene 7 %, web 0.4 % of pixels). |
| Not a window | Not in the overview window list, not in Alt-Tab (`get_tab_list` override), no "is ready" notification (`is_skip_taskbar` override). The renderer never takes focus because mutter denies focus to windows started without a user interaction. |
| Still image | A real frame (`MetaWindowActor.get_image`) is captured 6 s after pinning and published to `picture-uri`/`picture-uri-dark`; the square Workshop preview is only used until then. |
| Pause on fullscreen | A fullscreen GTK window on the monitor stops the renderer (`SIGSTOP`, process state `T`, `status` shows `paused ([Fullscreen])`); closing it resumes. |
| Blur my Shell v72 | Enabled in the nested session. Panel blur takes the wallpaper's colour in both modes: dynamic blur reads the framebuffer, which contains the clone; static blur reads the published still. The overview blur uses BMS's own `BackgroundManager` (container `bms-*`), which the extension skips, so it blurs the still. No JS errors. |
| Span across monitors | With a second virtual monitor (`--virtual-monitor 1024x640`), one 2304x800 renderer is cloned into both backgrounds with per-monitor offsets and the image is continuous across the seam. |
| Per-monitor assignment | Switching the second monitor to its own video leaves the first monitor's renderer running. |
| Daemon restart | Renderers are started behind a gate only after `Pin` was acknowledged; with the extension absent the slot reports "the shell extension is not running" and nothing is spawned. |
| Hardware decode | `vah264dec`/`vah265dec` exist (rank primary+1) once `mesa-va-drivers-freeworld` is installed and the GStreamer registry cache is refreshed; `playbin3` picks them. All seven workshop videos are H.264 except one MPEG-4 Part 2. |

### 5.1 Things that only showed up on the real Shell

- **Minimising before the first show freezes the surface.** The first version minimised the renderer window in `window-created`, before mutter had shown it once. The window then kept its first buffer forever (a 640x480 GLFW frame for scenes, which the clone stretched), and never received another frame callback. Minimising in `shown`, as Hanabi does, fixes it. The daemon still pins the PID before the window exists, so the window never appears as a window.
- **`Meta.Window.get_pid()`** works for Wayland clients and identifies the gated process.
- **dconf profiles need D-Bus-safe names.** `user-db:wpe-gnome-nested` produced a GDBus assertion and a hang inside `g_settings_sync()`; the profile is now `wpe_gnome_nested`. That hang also led to the rule that GSettings is only ever touched on a blocking thread with a timeout.
- **`async-process` drops stdio flags on conversion** from a `std::process::Command`; the pipes must be configured on the async command.
- **A GApplication id must differ from the daemon's bus name**, otherwise GTK tries to activate the daemon as an application (`org.gtk.Actions` error). The daemon is `io.github.realistik.WpeGnome.Daemon`.
- **`inotify` sees every read of the media file.** The watcher only reacts to create, remove, rename and data changes.

## 6. Measurements
