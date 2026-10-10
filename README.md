# Stagehand

Use your Steam Wallpaper Engine wallpapers as the desktop background on GNOME.

<img width="3440" height="1440" alt="Stagehand running a scene wallpaper on GNOME" src="https://github.com/user-attachments/assets/698b0ef5-ed50-4ee0-bd8b-3dee67046cdc" />

Stagehand finds the wallpapers you are subscribed to in the Steam Workshop and plays them behind
your windows, like a normal GNOME background. Scene, video and web wallpapers are supported. They
show up in the overview and the workspace switcher, stay out of Alt-Tab, can be set per monitor
or stretched across all of them, pause when they are covered or you are away, and come back when
you log in.

## Download

Stagehand runs on Fedora 44 with GNOME 50 on Wayland, which is the default desktop on Fedora
Workstation. You also need Steam with some Wallpaper Engine wallpapers subscribed and downloaded.

1. Go to the [latest release](https://github.com/RealistikDash/Stagehand/releases/latest) and
   download the two `.rpm` files, `stagehand` and `stagehand-scene-renderer`.
2. Open the Downloads folder and double-click each file to install it, or run this in a terminal:

   ```
   sudo dnf install ~/Downloads/stagehand-*.rpm
   ```

3. Log out and back in. Open **Stagehand** from the app grid, pick a wallpaper and press
   **Apply**.

Most video wallpapers use the H.264 format, which Fedora cannot play out of the box. To play
them, enable [RPM Fusion](https://rpmfusion.org/Configuration) and install the codecs:

```
sudo dnf install mesa-va-drivers-freeworld gstreamer1-plugin-libav gstreamer1-plugins-ugly
```

## Build from source

Only needed if you want the newest unreleased code or you are on a different Fedora version.
Otherwise skip to [Using Stagehand](#using-stagehand).

1. Install the build tools and libraries:

   ```
   sudo dnf install cargo rust gtk4-devel libadwaita-devel webkitgtk6.0-devel \
       gstreamer1-devel gstreamer1-plugins-base-devel graphene-devel just \
       gstreamer1-plugin-gtk4 gstreamer1-plugins-good gstreamer1-plugins-bad-free
   ```

2. For video wallpapers, enable [RPM Fusion](https://rpmfusion.org/Configuration) and install
   the codecs and hardware decoding drivers:

   ```
   sudo dnf install mesa-va-drivers-freeworld gstreamer1-plugin-libav gstreamer1-plugins-ugly
   ```

3. For scene wallpapers, which are the most common kind, install the extra dependencies and
   build the scene renderer:

   ```
   sudo dnf install cmake ninja-build gcc-c++ glew-devel freeglut-devel SDL2-devel mpv-libs-devel \
       ffmpeg-free-devel pulseaudio-libs-devel fftw-devel gmp-devel lz4-devel zlib-ng-compat-devel \
       libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel libXxf86vm-devel dbus-devel \
       glfw-devel glm-devel freetype-devel mesa-libEGL-devel mesa-libGL-devel wayland-devel \
       wayland-protocols-devel libwebp-devel
   just lwe
   ```

   This downloads and builds [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine),
   which Stagehand uses to render scenes. It takes a few minutes. Skip this step if you only use
   video or web wallpapers.

4. Build and install Stagehand for your user:

   ```
   just install
   ```

5. Log out and back in. GNOME only loads new extensions at login.

### Build the RPM packages yourself

If you prefer packages you can remove with `dnf`:

```
sudo dnf install rpm-build rpmdevtools libappstream-glib desktop-file-utils systemd-rpm-macros
just rpm
sudo dnf install $(find target/rpm/RPMS -name '*.rpm')
```

This builds two packages: `stagehand` and `stagehand-scene-renderer`. The build needs network
access. Log out and back in afterwards.

## Using Stagehand

Open **Stagehand** from the app grid. The **Library** page lists every wallpaper from your Steam
Workshop subscriptions. Pick one, choose a monitor or **All monitors**, and press **Apply**.
Wallpapers that cannot run on Linux are marked **Not supported**.

Things you can change in the app:

- **Monitors**: a different wallpaper per monitor, or one wallpaper spanning all of them.
- **Properties**: the colour, speed and other options the wallpaper author exposes.
- **Audio**: volume, mute, and mute automatically when another app plays sound.
- **Pause**: pause the wallpaper when a window covers it, when the screen is locked, or when
  you are idle.
- **OLED care**: dim the wallpaper, and hide the frozen frame after a while when paused.
- **Rendering**: frame rate limit and how a wallpaper fills a monitor of a different shape.
- **Start at login**: whether the wallpaper service starts with your session.

Pressing **Stop All** stops every wallpaper. To put the GNOME background you had before back,
run `stagehandctl restore-background` in a terminal.

### Command line

`stagehandctl` does the same things from a terminal, which is handy for scripts and keyboard
shortcuts:

```
stagehandctl list                              # your wallpapers, with support status
stagehandctl monitors                          # connected monitors and their keys
stagehandctl apply workshop:<id> --monitor '<key>'
stagehandctl apply workshop:<id> --span        # one wallpaper across all monitors
stagehandctl status
stagehandctl stop
stagehandctl restore-background                # put the original GNOME background back
```

The `<id>` is the number at the end of a wallpaper's Steam Workshop URL. `stagehandctl list`
prints it for every wallpaper.

## Uninstall

If you installed the downloaded packages:

```
stagehandctl restore-background
sudo dnf remove stagehand stagehand-scene-renderer
```

If you built from source with `just install`, run `just uninstall` instead. It removes everything
Stagehand installed and restores the background you had before.

## If something goes wrong

**The wallpaper does not appear after installing.** Log out and back in. If it still does not
show, check that the extension is enabled:

```
gnome-extensions enable stagehand@realistikdash.github.io
systemctl --user restart stagehandd.service
```

**A video wallpaper is black or stutters.** Make sure the RPM Fusion codecs from step 2 are
installed. Most Workshop videos are H.264, which Fedora cannot play out of the box.

**A scene wallpaper is listed as not supported.** Scenes need the `stagehand-scene-renderer`
package from the release. If you built from source, run `just lwe` and then `just install` again.

**GNOME Shell misbehaves after an update.** Switch to a text console with Ctrl+Alt+F3, log in,
and run:

```
gnome-extensions disable stagehand@realistikdash.github.io
systemctl --user stop stagehandd.service
```

Then switch back with Ctrl+Alt+F2 (or F1). `stagehandctl restore-background` puts your original
background back at any time.

## Known limitations

- "Application" wallpapers are Windows programs and cannot run.
- Changing the properties or audio settings of a scene wallpaper restarts it briefly. Stagehand
  crossfades to the new one.
- Web wallpapers do not react to audio, and the frame rate limit does not apply to them.
- The lock screen and blurred backgrounds show a still frame captured a few seconds after the
  wallpaper starts. Until then the Workshop preview image is used.
- Wallpaper volume is one setting for all monitors.
- Steam libraries on unmounted drives are skipped until the next rescan.

## For developers

See `docs/design.md` for how Stagehand works, what was verified and measured, and the pitfalls
found along the way. `just nested` runs a nested GNOME Shell with the extension and daemon for
safe testing, and `just lint` runs the formatter and clippy. Never test extension changes in your
live session first.

## Licence

Stagehand is licensed under the terms in `LICENSE`. linux-wallpaperengine is a separate GPL-3.0
program that Stagehand starts as a child process.
