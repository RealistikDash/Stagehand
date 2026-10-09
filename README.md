# Stagehand

Steam Wallpaper Engine wallpapers as the real GNOME desktop background, on Wayland.

Scene, video and web wallpapers from your Workshop subscriptions run behind every window, show in
the overview and workspace switcher, never appear in Alt-Tab or the dash, follow each monitor,
pause when covered, locked or idle, and come back at login.

- `stagehandd`: the daemon. Discovers wallpapers, tracks monitors, starts and pauses renderers,
  owns the configuration, exposes a D-Bus API. Runs as a systemd user service.
- `stagehand`: the GTK4/libadwaita settings app. A client of the daemon only.
- `stagehand-renderer`: GTK window renderer for videos (GStreamer) and web wallpapers (WebKitGTK).
- `linux-wallpaperengine`: renders scenes, started as a separate process (GPL-3.0, not linked).
- A GNOME Shell extension that pins renderer windows into every background.
- `stagehandctl`: command line client.

See `docs/design.md` for how it works and what was verified on the development machine.

## Requirements

Fedora 44 with GNOME 50 on Wayland. Build dependencies:

```
sudo dnf install cargo rust gtk4-devel libadwaita-devel webkitgtk6.0-devel \
    gstreamer1-devel gstreamer1-plugins-base-devel graphene-devel just \
    gstreamer1-plugin-gtk4 gstreamer1-plugins-good gstreamer1-plugins-bad-free
```

Hardware video decoding and H.264 need RPM Fusion:

```
sudo dnf install mesa-va-drivers-freeworld gstreamer1-plugin-libav gstreamer1-plugins-ugly
```

Scene wallpapers need linux-wallpaperengine, which `just lwe` builds from a pinned commit:

```
sudo dnf install cmake ninja-build gcc-c++ glew-devel freeglut-devel SDL2-devel mpv-libs-devel \
    ffmpeg-free-devel pulseaudio-libs-devel fftw-devel gmp-devel lz4-devel zlib-ng-compat-devel \
    libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel libXxf86vm-devel dbus-devel \
    glfw-devel glm-devel freetype-devel mesa-libEGL-devel mesa-libGL-devel wayland-devel \
    wayland-protocols-devel libwebp-devel
```

The import formatting options need a nightly `rustfmt`: `rustup toolchain install nightly
--profile minimal -c rustfmt`.

## Install for the current user

```
just lwe        # optional, scene support; downloads CEF (~150 MB) during the build
just install    # builds in release mode, installs to ~/.local, enables the unit and extension
```

Log out and back in once: GNOME on Wayland loads new extensions only at login. Then open
"Stagehand" from the app grid, pick a wallpaper and apply it to a monitor.

`just uninstall` removes everything and restores the background keys that were in place before
the first wallpaper was applied.

## Install as RPMs

```
sudo dnf install rpm-build rpmdevtools libappstream-glib desktop-file-utils systemd-rpm-macros
just rpm
sudo dnf install $(find target/rpm/RPMS -name '*.rpm')
```

`stagehand` installs the daemon, app, renderer, CLI, extension, D-Bus activation file and a
systemd user unit with a preset, so the daemon starts with every graphical session. On its first
start it enables the Shell extension. `stagehand-scene-renderer` is linux-wallpaperengine under
`/usr/libexec/stagehand`. Both builds need network access (crates are vendored by the script, CEF
is downloaded by linux-wallpaperengine's CMake).

## Command line

```
stagehandctl list                 # every wallpaper, with support status
stagehandctl show workshop:<id>   # properties of one wallpaper
stagehandctl monitors             # connected monitors and their assignment keys
stagehandctl apply workshop:<id> --monitor '<key>'
stagehandctl apply workshop:<id> --span
stagehandctl status
stagehandctl stop
stagehandctl restore-background
```

## Development

```
just nested     # nested GNOME Shell (devkit) with the extension and daemon, isolated dconf
just lint       # cargo +nightly fmt --check, clippy -D warnings
```

Never test extension changes in the live session first; `just nested` runs them in a nested
Shell with a private D-Bus session and a private dconf database.

## Recovery

If the Shell misbehaves after an update, from a TTY (Ctrl+Alt+F3):

```
gnome-extensions disable stagehand@realistikdash.github.io
systemctl --user stop stagehandd.service
```

To put the original background back: `stagehandctl restore-background`.

## Known limitations

- `application` wallpapers are Windows executables and are listed as unsupported.
- Scene properties and audio settings restart linux-wallpaperengine (it has no live channel);
  the extension crossfades the new renderer in.
- Web wallpapers receive their properties but no audio data; the FPS cap does not apply to them.
- The still image for the lock screen and static blur is a frame captured a few seconds after the
  renderer starts; until then the Workshop preview is used.
- One wallpaper audio volume applies to all monitors.
- Steam libraries on drives that are not mounted are skipped until the next rescan.
