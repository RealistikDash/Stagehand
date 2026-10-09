# stagehand task runner. `just --list` shows everything.

set shell := ["bash", "-euo", "pipefail", "-c"]

uuid := "stagehand@realistikdash.github.io"
bindir := env("HOME") / ".local/bin"
libexecdir := env("HOME") / ".local/libexec/stagehand"
datadir := env("XDG_DATA_HOME", env("HOME") / ".local/share")
configdir := env("XDG_CONFIG_HOME", env("HOME") / ".config")
lwe_commit := "b016d7d1fdcf4e5fd2f9c9fa420a8aaa07fee02d"
lwe_repo := "https://github.com/Almamu/linux-wallpaperengine.git"

default:
    @just --list

# Build everything in release mode.
build:
    cargo build --release

# Format (nightly rustfmt for the import options) and lint.
lint:
    cargo +nightly fmt --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo +nightly fmt

# Build linux-wallpaperengine from the pinned commit into build/lwe (needs network for CEF).
lwe:
    tools/build-lwe.sh "{{lwe_repo}}" "{{lwe_commit}}" build/lwe

# Install everything for the current user: binaries, extension, unit, D-Bus service, desktop file.
install: build
    tools/install-user.sh "{{bindir}}" "{{libexecdir}}" "{{datadir}}" "{{configdir}}" "{{uuid}}"

# Remove the per-user installation and restore the original background keys.
uninstall:
    tools/uninstall-user.sh "{{bindir}}" "{{libexecdir}}" "{{datadir}}" "{{configdir}}" "{{uuid}}"

# Run a nested devkit Shell with the extension and daemon for safe testing.
nested *ARGS:
    cargo build
    tools/nested.sh {{ARGS}}

# Build the RPMs into target/rpm (stagehand and stagehand-scene-renderer).
rpm:
    tools/build-rpm.sh "{{lwe_repo}}" "{{lwe_commit}}"
