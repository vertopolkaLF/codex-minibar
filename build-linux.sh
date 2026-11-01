#!/usr/bin/env bash
# Builds the Linux release packages into dist/<version>/:
#   codex-minibar-<version>-linux-<arch>.tar.gz  (binary, desktop entry, icons)
#   codex-minibar_<version>_<debarch>.deb        (Debian/Ubuntu package)
#   codex-minibar-<version>-linux-<arch>.AppImage (portable, x64 only)
#
# Usage: ./build-linux.sh [--skip-build]
# Requires: cargo; dpkg-deb for the .deb; patchelf and curl for the AppImage.
# A missing packaging tool skips its package, except in CI, where it fails.
#
# An AppImage runs on systems at least as new as the one that built it (glibc
# is not bundled), so published ones come from the oldest CI runner.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$root"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
case "$(uname -m)" in
    x86_64) arch=x64 debarch=amd64 ;;
    aarch64) arch=arm64 debarch=arm64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--skip-build" ]; then
    cargo build --release --locked
fi

binary="target/release/codex-minibar"
dist="dist/$version"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$dist"

skip_package() {
    echo "$1" >&2
    if [ -n "${CI:-}" ]; then
        exit 1
    fi
}

# Filesystem layout shared by every package, rooted at a prefix.
install_tree() {
    local prefix="$1"
    install -Dm755 "$binary" "$prefix/bin/codex-minibar"
    install -Dm644 packaging/linux/io.github.vertopolkalf.CodexMinibar.desktop \
        "$prefix/share/applications/io.github.vertopolkalf.CodexMinibar.desktop"
    for size in 16 24 32 48 64 128 256 512; do
        install -Dm644 "assets/icons/app-icon-$size.png" \
            "$prefix/share/icons/hicolor/${size}x${size}/apps/codex-minibar.png"
    done
    install -Dm644 LICENSE "$prefix/share/doc/codex-minibar/LICENSE"
    install -Dm644 THIRD_PARTY_NOTICES.md \
        "$prefix/share/doc/codex-minibar/THIRD_PARTY_NOTICES.md"
}

# Portable tarball: extract anywhere, or into ~/.local for a per-user install.
tar_root="$stage/codex-minibar-$version"
install_tree "$tar_root"
cat > "$tar_root/install.sh" <<'INSTALL'
#!/usr/bin/env sh
# Installs Codex Minibar for the current user into ~/.local.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
mkdir -p "$prefix"
cp -R "$here/bin" "$here/share" "$prefix/"
command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "$prefix/share/applications" 2>/dev/null || true
echo "Installed to $prefix. Make sure $prefix/bin is on your PATH."
INSTALL
chmod 755 "$tar_root/install.sh"
tarball="$dist/codex-minibar-$version-linux-$arch.tar.gz"
tar -C "$stage" -czf "$tarball" "codex-minibar-$version"
echo "Built $tarball"

build_deb() {
    deb_root="$stage/deb"
    install_tree "$deb_root/usr"
    mkdir -p "$deb_root/DEBIAN"
    installed_size="$(du -sk "$deb_root/usr" | cut -f1)"
    # GPUI loads xkbcommon, Wayland, X11 and Vulkan at runtime, so they are not
    # visible to ldd and must be listed explicitly.
    cat > "$deb_root/DEBIAN/control" <<CONTROL
Package: codex-minibar
Version: $version
Section: utils
Priority: optional
Architecture: $debarch
Maintainer: vertopolkaLF <noreply@github.com>
Installed-Size: $installed_size
Depends: libc6, libssl3t64 | libssl3, zlib1g, libzstd1, libbz2-1.0, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-xcb1, libxcb1, libvulkan1, mesa-vulkan-drivers | vulkan-icd, libfontconfig1, libfreetype6
Recommends: xdg-desktop-portal, gnome-shell-extension-appindicator | plasma-workspace
Homepage: https://github.com/vertopolkaLF/codex-minibar
Description: Tray companion for monitoring AI tools usage
 Codex Minibar shows usage limits, resets and spend of AI coding tools
 (Codex, Claude Code, Cursor, OpenRouter and more) in the system tray.
CONTROL
    deb="$dist/codex-minibar_${version}_${debarch}.deb"
    dpkg-deb --root-owner-group --build "$deb_root" "$deb" >/dev/null
    echo "Built $deb"
}

# Pinned build tools, verified before they run.
appimagetool_url="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
appimagetool_sha256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
appimage_runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
appimage_runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

fetch_tool() {
    local url="$1" sha256="$2" path="$3"
    if [ ! -f "$path" ] || ! echo "$sha256  $path" | sha256sum --check --status; then
        curl --fail --silent --show-error --location --retry 3 --output "$path" "$url"
        echo "$sha256  $path" | sha256sum --check --status
    fi
}

# Libraries that are not on every desktop. The graphics, display and font
# libraries (Vulkan, Wayland, X11, fontconfig) and glibc must come from the
# host: they match its drivers and configuration.
appimage_libraries="libssl.so.3 libcrypto.so.3 libbz2.so.1.0 libzstd.so.1 libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1"

build_appimage() {
    local appdir="$stage/CodexMinibar.AppDir"
    local tools="target/appimage-tools"
    local app_id="io.github.vertopolkalf.CodexMinibar"
    local output="$dist/codex-minibar-$version-linux-$arch.AppImage"
    mkdir -p "$tools"
    fetch_tool "$appimagetool_url" "$appimagetool_sha256" "$tools/appimagetool"
    fetch_tool "$appimage_runtime_url" "$appimage_runtime_sha256" "$tools/runtime"
    chmod 755 "$tools/appimagetool"

    install_tree "$appdir/usr"
    mkdir -p "$appdir/usr/lib"
    local library path
    for library in $appimage_libraries; do
        path="$(ldd "$binary" | awk -v name="$library" '$1 == name { print $3 }')"
        if [ -z "$path" ]; then
            echo "$binary does not link $library; update appimage_libraries" >&2
            exit 1
        fi
        cp -L "$path" "$appdir/usr/lib/$library"
        chmod 755 "$appdir/usr/lib/$library"
        patchelf --set-rpath '$ORIGIN' "$appdir/usr/lib/$library"
    done
    # An rpath instead of LD_LIBRARY_PATH: the app starts provider CLIs, and
    # those must not inherit these libraries.
    patchelf --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/codex-minibar"

    ln -s usr/bin/codex-minibar "$appdir/AppRun"
    cp "$appdir/usr/share/applications/$app_id.desktop" "$appdir/$app_id.desktop"
    cp assets/icons/app-icon-256.png "$appdir/codex-minibar.png"
    ln -s codex-minibar.png "$appdir/.DirIcon"

    # Extract-and-run: build machines need no FUSE.
    ARCH=x86_64 "$tools/appimagetool" --appimage-extract-and-run --no-appstream \
        --runtime-file "$tools/runtime" "$appdir" "$output" >/dev/null
    chmod 755 "$output"
    echo "Built $output"
}

if command -v dpkg-deb >/dev/null 2>&1; then
    build_deb
else
    skip_package "dpkg-deb not found; skipping the .deb"
fi

if [ "$arch" != x64 ]; then
    echo "The AppImage is built for x64 only; skipping it" >&2
elif command -v patchelf >/dev/null 2>&1 && command -v curl >/dev/null 2>&1; then
    build_appimage
else
    skip_package "patchelf or curl not found; skipping the AppImage"
fi
