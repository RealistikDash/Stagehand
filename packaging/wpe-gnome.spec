%global uuid wpe-gnome@realistik.github.io
%global appid io.github.realistik.WpeGnome

Name:           wpe-gnome
Version:        0.1.0
Release:        1%{?dist}
Summary:        Steam Wallpaper Engine wallpapers as live GNOME backgrounds
License:        MIT
URL:            https://github.com/realistik/wpe-gnome
Source0:        %{name}-%{version}.tar.gz
# `cargo vendor` output, so the build needs no network.
Source1:        %{name}-%{version}-vendor.tar.gz

BuildRequires:  cargo >= 1.98
BuildRequires:  rust >= 1.98
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel >= 1.9
BuildRequires:  webkitgtk6.0-devel
BuildRequires:  gstreamer1-devel
BuildRequires:  gstreamer1-plugins-base-devel
BuildRequires:  graphene-devel
BuildRequires:  systemd-rpm-macros
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gnome-shell >= 50
Requires:       gstreamer1-plugin-gtk4
Requires:       gstreamer1-plugins-good
Requires:       gstreamer1-plugins-bad-free
Requires:       webkitgtk6.0
Requires:       libadwaita >= 1.9
Recommends:     wpe-gnome-scene-renderer = %{version}
Recommends:     gstreamer1-plugin-libav
Recommends:     mesa-va-drivers-freeworld
%{?systemd_user_requires}

%description
Shows the scene, video and web wallpapers subscribed to in the Steam Workshop as the real GNOME
desktop background on Wayland: behind every window, in the overview, on every monitor. Scenes are
rendered by linux-wallpaperengine (package wpe-gnome-scene-renderer); videos and web wallpapers by
the bundled GTK renderer. The daemon starts with the graphical session and enables the Shell
extension on first run.

%prep
%autosetup -n %{name}-%{version}
tar xzf %{SOURCE1}
mkdir -p .cargo
cat > .cargo/config.toml <<CFG
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
CFG

%build
cargo build --release --offline

%install
install -D -m 755 target/release/wpe-gnomed %{buildroot}%{_bindir}/wpe-gnomed
install -D -m 755 target/release/wpe-gnome %{buildroot}%{_bindir}/wpe-gnome
install -D -m 755 target/release/wpe-gnome-renderer %{buildroot}%{_bindir}/wpe-gnome-renderer
install -D -m 755 target/release/wpe-gnomectl %{buildroot}%{_bindir}/wpe-gnomectl
sed "s|@BINDIR@|%{_bindir}|g" data/%{appid}.Daemon.service \
    > %{appid}.Daemon.service.out
install -D -m 644 %{appid}.Daemon.service.out \
    %{buildroot}%{_datadir}/dbus-1/services/%{appid}.Daemon.service
sed "s|@BINDIR@|%{_bindir}|g" data/wpe-gnomed.service > wpe-gnomed.service.out
install -D -m 644 wpe-gnomed.service.out %{buildroot}%{_userunitdir}/wpe-gnomed.service
install -D -m 644 data/90-wpe-gnome.preset %{buildroot}%{_userpresetdir}/90-wpe-gnome.preset
install -D -m 644 data/%{appid}.desktop %{buildroot}%{_datadir}/applications/%{appid}.desktop
install -D -m 644 data/%{appid}.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{appid}.svg
install -D -m 644 data/%{appid}.metainfo.xml %{buildroot}%{_metainfodir}/%{appid}.metainfo.xml
mkdir -p %{buildroot}%{_datadir}/gnome-shell/extensions
cp -r extension/%{uuid} %{buildroot}%{_datadir}/gnome-shell/extensions/%{uuid}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{appid}.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/%{appid}.metainfo.xml

%post
%systemd_user_post wpe-gnomed.service

%preun
%systemd_user_preun wpe-gnomed.service

%postun
%systemd_user_postun_with_restart wpe-gnomed.service

%files
%license LICENSE
%doc README.md docs/design.md
%{_bindir}/wpe-gnomed
%{_bindir}/wpe-gnome
%{_bindir}/wpe-gnome-renderer
%{_bindir}/wpe-gnomectl
%{_datadir}/dbus-1/services/%{appid}.Daemon.service
%{_userunitdir}/wpe-gnomed.service
%{_userpresetdir}/90-wpe-gnome.preset
%{_datadir}/applications/%{appid}.desktop
%{_datadir}/icons/hicolor/scalable/apps/%{appid}.svg
%{_metainfodir}/%{appid}.metainfo.xml
%{_datadir}/gnome-shell/extensions/%{uuid}/

%changelog
* Thu Oct 09 2026 realistik - 0.1.0-1
- Initial package.
