%global commit b016d7d1fdcf4e5fd2f9c9fa420a8aaa07fee02d
%global shortcommit %(c=%{commit}; echo ${c:0:7})
%global lwedir %{_libexecdir}/stagehand/lwe

Name:           stagehand-scene-renderer
Version:        0.1.0
Release:        1.git%{shortcommit}%{?dist}
Summary:        linux-wallpaperengine build used by stagehand for scene wallpapers
# linux-wallpaperengine is GPL-3.0-only; it runs as a separate process started by stagehand.
License:        GPL-3.0-only
URL:            https://github.com/Almamu/linux-wallpaperengine
# Recursive checkout of the pinned commit including submodules (tools/build-rpm.sh creates it).
Source0:        linux-wallpaperengine-%{shortcommit}.tar.gz
# Adds an ENABLE_CEF option; Stagehand renders web wallpapers itself, so CEF is left out.
Patch0:         lwe-no-cef.patch

BuildRequires:  cmake >= 3.12
BuildRequires:  patchelf
BuildRequires:  ninja-build
BuildRequires:  gcc-c++
BuildRequires:  glew-devel
BuildRequires:  freeglut-devel
BuildRequires:  SDL2-devel
BuildRequires:  mpv-libs-devel
BuildRequires:  (ffmpeg-free-devel or ffmpeg-devel)
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
# The binary and its library carry $ORIGIN rpaths; do not scan them.
%global __requires_exclude_from ^%{lwedir}/.*$
%global __provides_exclude_from ^%{lwedir}/.*$
%global debug_package %{nil}

%description
A build of linux-wallpaperengine at a commit Stagehand was tested with, installed under
%{lwedir}. Its web wallpaper support (Chromium Embedded Framework) is compiled out because
Stagehand renders web wallpapers itself. Stagehand starts it as a separate process for scene
wallpapers.

%prep
%autosetup -n linux-wallpaperengine-%{shortcommit} -p1

%build
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DENABLE_CEF=OFF
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
# Bundled libraries (kissfft) live next to the output directory; ship them with relative runpaths.
mkdir -p %{buildroot}%{lwedir}/lib
cp -a build/lib/*.so* %{buildroot}%{lwedir}/lib/
patchelf --set-rpath '$ORIGIN:$ORIGIN/lib' %{buildroot}%{lwedir}/linux-wallpaperengine
for f in %{buildroot}%{lwedir}/*.so %{buildroot}%{lwedir}/*.so.*; do
    [ -f "$f" ] && patchelf --set-rpath '$ORIGIN:$ORIGIN/lib' "$f"
done
# debuginfo extraction is off for this package, so strip explicitly.
find %{buildroot}%{lwedir} -type f \( -name '*.so' -o -name '*.so.*' -o -name linux-wallpaperengine \) -exec strip --strip-unneeded {} +
exit 0

%files
%license LICENSE
%{lwedir}/

%changelog
* Fri Oct 09 2026 RealistikDash - 0.1.0-1.gitb016d7d
- Initial package.
