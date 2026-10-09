%global commit b016d7d1fdcf4e5fd2f9c9fa420a8aaa07fee02d
%global shortcommit %(c=%{commit}; echo ${c:0:7})
%global lwedir %{_libexecdir}/wpe-gnome/lwe

Name:           wpe-gnome-scene-renderer
Version:        0.1.0
Release:        1.git%{shortcommit}%{?dist}
Summary:        linux-wallpaperengine build used by wpe-gnome for scene wallpapers
# linux-wallpaperengine is GPL-3.0-only; it runs as a separate process started by wpe-gnome.
License:        GPL-3.0-only
URL:            https://github.com/Almamu/linux-wallpaperengine
# Recursive checkout of %{commit} including submodules (tools/build-rpm.sh creates it).
Source0:        linux-wallpaperengine-%{shortcommit}.tar.gz

BuildRequires:  cmake >= 3.12
BuildRequires:  ninja-build
BuildRequires:  gcc-c++
BuildRequires:  glew-devel
BuildRequires:  freeglut-devel
BuildRequires:  SDL2-devel
BuildRequires:  mpv-libs-devel
BuildRequires:  ffmpeg-free-devel
BuildRequires:  pulseaudio-libs-devel
BuildRequires:  fftw-devel
BuildRequires:  gmp-devel
BuildRequires:  lz4-devel
BuildRequires:  zlib-ng-compat-devel
BuildRequires:  libXrandr-devel
BuildRequires:  libXinerama-devel
BuildRequires:  libXcursor-devel
BuildRequires:  libXi-devel
BuildRequires:  libXxf86vm-devel
BuildRequires:  dbus-devel
BuildRequires:  glfw-devel
BuildRequires:  glm-devel
BuildRequires:  freetype-devel
BuildRequires:  mesa-libEGL-devel
BuildRequires:  mesa-libGL-devel
BuildRequires:  wayland-devel
BuildRequires:  wayland-protocols-devel
BuildRequires:  libwebp-devel

Provides:       linux-wallpaperengine = 0^%{shortcommit}
# The binary ships CEF and other libraries with their own rpaths; do not scan them.
%global __requires_exclude_from ^%{lwedir}/.*$
%global __provides_exclude_from ^%{lwedir}/.*$
%global debug_package %{nil}

%description
A build of linux-wallpaperengine at a commit wpe-gnome was tested with, installed under
%{lwedir} together with the Chromium Embedded Framework it downloads at build time (so the build
needs network access). wpe-gnome starts it as a separate process for scene wallpapers.

%prep
%autosetup -n linux-wallpaperengine-%{shortcommit}

%build
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
ninja -C build

%install
mkdir -p %{buildroot}%{lwedir}
tools_dir="$(pwd)/build/output"
cp -a "$tools_dir/linux-wallpaperengine" %{buildroot}%{lwedir}/
for f in "$tools_dir"/*.so* "$tools_dir"/*.pak "$tools_dir"/*.dat "$tools_dir"/*.bin "$tools_dir"/*.json "$tools_dir"/chrome-sandbox; do
    [ -e "$f" ] && cp -a "$f" %{buildroot}%{lwedir}/
done
for d in lib lib64 locales swiftshader assets; do
    [ -d "$tools_dir/$d" ] && cp -a "$tools_dir/$d" %{buildroot}%{lwedir}/
done
exit 0

%files
%license LICENSE
%{lwedir}/

%changelog
* Thu Oct 09 2026 realistik - 0.1.0-1.gitb016d7d
- Initial package.
