#!/usr/bin/env bash
# Builds the Linux release packages into dist/<version>/:
#   codex-minibar-<version>-<arch>.tar.gz        (binary, desktop entry, icons)
#   codex-minibar_<version>_<debarch>.deb        (Debian/Ubuntu package)
#   codex-minibar-<version>-<rpmrelease>.<arch>.rpm (Fedora/openSUSE package)
#   codex-minibar-<version>-<arch>.AppImage      (portable, x86_64 only)
#   PKGBUILD                                     (Arch User Repository, for the tarball)
# <arch> is `uname -m` (x86_64, aarch64). Every package is for Linux, so no
# name says "linux".
#
# Usage: ./build-linux.sh [--skip-build]
# Honors CARGO_TARGET_DIR for the release binary, including --skip-build.
# Requires: cargo; dpkg-deb for the .deb; rpmbuild for the .rpm; patchelf,
# curl and Mesa's lavapipe with the Vulkan loader (mesa-vulkan-drivers,
# libvulkan1) for the AppImage.
# A missing packaging tool skips its package, except in CI, where it fails.
#
# An AppImage runs on systems at least as new as the one that built it (glibc
# is not bundled), so published ones come from the oldest CI runner.
set -euo pipefail

usage() {
    echo "Usage: $0 [--skip-build]"
}

skip_build=false
if [ "$#" -gt 1 ]; then
    usage >&2
    exit 2
fi
case "${1:-}" in
    "") ;;
    --skip-build) skip_build=true ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$root"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
case "$(uname -m)" in
    x86_64) arch=x86_64 debarch=amd64 ;;
    aarch64) arch=aarch64 debarch=arm64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [ "$skip_build" = false ]; then
    cargo build --release --locked
fi

binary="${CARGO_TARGET_DIR:-target}/release/codex-minibar"
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
    # Panel widgets: a GNOME Shell extension and a Plasma 6 widget. Installed
    # where each desktop looks for them; users enable them themselves.
    local extension=codex-minibar@vertopolkalf.github.io
    local plasmoid=io.github.vertopolkalf.codexminibar
    local file
    for file in metadata.json extension.js stylesheet.css; do
        install -Dm644 "integrations/gnome-shell/$extension/$file" \
            "$prefix/share/gnome-shell/extensions/$extension/$file"
    done
    for file in metadata.json contents/config/config.qml contents/config/main.xml \
        contents/ui/configGeneral.qml contents/ui/main.qml; do
        install -Dm644 "integrations/kde-plasma/$plasmoid/$file" \
            "$prefix/share/plasma/plasmoids/$plasmoid/$file"
    done
}

# Portable tarball: extract anywhere, or into ~/.local for a per-user install.
tar_root="$stage/codex-minibar-$version"
install_tree "$tar_root"
install -m755 packaging/linux/install.sh "$tar_root/install.sh"
tarball="$dist/codex-minibar-$version-$arch.tar.gz"
tar -C "$stage" -czf "$tarball" "codex-minibar-$version"
echo "Built $tarball"

build_deb() {
    deb_root="$stage/deb"
    install_tree "$deb_root/usr"
    mkdir -p "$deb_root/DEBIAN"
    installed_size="$(du -sk "$deb_root/usr" | cut -f1)"
    # GPUI loads xkbcommon, Wayland, X11 and Vulkan at runtime, so they are not
    # visible to ldd and must be listed explicitly. Desktop-specific tray
    # hosts stay optional: Recommends could pull GNOME or Plasma into Xfce,
    # Cinnamon and minimal Wayland sessions that already have another panel.
    cat > "$deb_root/DEBIAN/control" <<CONTROL
Package: codex-minibar
Version: $version
Section: utils
Priority: optional
Architecture: $debarch
Maintainer: vertopolkaLF <noreply@github.com>
Installed-Size: $installed_size
Depends: xdg-utils, ca-certificates, libc6, libssl3t64 | libssl3, libbz2-1.0, zlib1g, libzstd1, liblzma5, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-xcb1, libxcb1, libvulkan1, mesa-vulkan-drivers | vulkan-icd, libfontconfig1, libfreetype6
Recommends: xdg-desktop-portal
Suggests: gnome-shell-extension-appindicator
Homepage: https://github.com/vertopolkaLF/codex-minibar
Description: Tray companion for monitoring AI tools usage
 Codex Minibar shows usage limits, resets and spend of AI coding tools
 (Codex, Claude Code, Cursor, OpenRouter and more) in the system tray.
CONTROL
    deb="$dist/codex-minibar_${version}_${debarch}.deb"
    dpkg-deb --root-owner-group --build "$deb_root" "$deb" >/dev/null
    echo "Built $deb"
}

build_rpm() {
    local topdir="$stage/rpm"
    local rpm_root="$stage/rpm-root"
    install_tree "$rpm_root/usr"
    mkdir -p "$topdir"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    # Libraries the binary links are found by rpm's automatic dependencies.
    # GPUI loads xkbcommon, Wayland, X11 and Vulkan at runtime, so those are
    # listed by name; the names resolve on Fedora and openSUSE alike.
    cat > "$topdir/SPECS/codex-minibar.spec" <<SPEC
Name:           codex-minibar
Version:        $version
Release:        1
Summary:        Tray companion for monitoring AI tools usage
License:        Apache-2.0
URL:            https://github.com/vertopolkaLF/codex-minibar
Requires:       xdg-utils
Requires:       ca-certificates
Requires:       libxkbcommon.so.0()(64bit)
Requires:       libxkbcommon-x11.so.0()(64bit)
Requires:       libwayland-client.so.0()(64bit)
Requires:       libX11-xcb.so.1()(64bit)
Requires:       libvulkan.so.1()(64bit)
Requires:       libfontconfig.so.1()(64bit)
Requires:       libfreetype.so.6()(64bit)
Recommends:     xdg-desktop-portal

%global debug_package %{nil}
%global __strip /bin/true

%description
Codex Minibar shows usage limits, resets and spend of AI coding tools
(Codex, Claude Code, Cursor, OpenRouter and more) in the system tray.

%install
cp -a "$rpm_root/." "%{buildroot}/"

%files
%license /usr/share/doc/codex-minibar/LICENSE
/usr/bin/codex-minibar
/usr/share/applications/io.github.vertopolkalf.CodexMinibar.desktop
/usr/share/icons/hicolor/*/apps/codex-minibar.png
%dir /usr/share/doc/codex-minibar
/usr/share/doc/codex-minibar/THIRD_PARTY_NOTICES.md
/usr/share/gnome-shell/extensions/codex-minibar@vertopolkalf.github.io
/usr/share/plasma/plasmoids/io.github.vertopolkalf.codexminibar
SPEC
    rpmbuild --quiet -bb --target "$arch" \
        --define "_topdir $topdir" \
        --define "_build_id_links none" \
        "$topdir/SPECS/codex-minibar.spec" >/dev/null
    local rpm="$dist/codex-minibar-$version-1.$arch.rpm"
    cp "$topdir/RPMS/$arch/codex-minibar-$version-1.$arch.rpm" "$rpm"
    echo "Built $rpm"
}

# The Arch User Repository recipe for codex-minibar-bin, pinned to this
# tarball. Publish it (with `makepkg --printsrcinfo > .SRCINFO`) to the AUR
# after the release is out.
write_pkgbuild() {
    local sha256
    sha256="$(sha256sum "$tarball" | cut -d' ' -f1)"
    sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$sha256/g" \
        packaging/aur/PKGBUILD.in > "$dist/PKGBUILD"
    echo "Wrote $dist/PKGBUILD"
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

# Linked libraries that must come from the host: glibc and the toolchain
# runtimes (host graphics drivers load newer ones than a bundled copy) and the
# X11 client stack. Everything else the binary links is bundled, because which
# libraries are linked differs between build systems. Vulkan, Wayland and
# fontconfig are loaded at runtime, so they stay on the host too.
appimage_host_libraries='^(ld-linux-x86-64|libc|libm|libdl|libpthread|librt|libresolv|libgcc_s|libstdc\+\+|libz|libxcb|libXau|libXdmcp|libbsd|libmd)\.so'

# Copies the libraries an ELF file links (all of them, transitively) into a
# directory, except the host's, and makes them find each other there.
bundle_libraries() {
    local elf="$1" dir="$2"
    if ldd "$elf" | grep -q 'not found'; then
        ldd "$elf" >&2
        exit 1
    fi
    mkdir -p "$dir"
    local library path
    while read -r library path; do
        if [[ "$library" =~ $appimage_host_libraries ]] || [ -e "$dir/$library" ]; then
            continue
        fi
        cp -L "$path" "$dir/$library"
        chmod 755 "$dir/$library"
        patchelf --set-rpath '$ORIGIN' "$dir/$library"
    done < <(ldd "$elf" | awk '$2 == "=>" { print $1, $3 }')
}

# Mesa's lavapipe, a CPU Vulkan driver, with the Vulkan loader and their
# libraries, in usr/lib/lavapipe. GPUI cannot start without a Vulkan device;
# on hosts that have none (virtual machines, missing drivers, headless test
# machines) the app points the loader at this driver at startup
# (src/platform/linux/vulkan.rs). The directory is not on the binary's rpath,
# so the host's loader and drivers are used whenever they work.
bundle_lavapipe() {
    local dir="$1/usr/lib/lavapipe"
    local manifest="" driver loader candidate
    # Older Mesa names the manifest per architecture; newer Mesa shares one.
    for candidate in /usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
        /usr/share/vulkan/icd.d/lvp_icd.json; do
        if [ -f "$candidate" ]; then
            manifest="$candidate"
            break
        fi
    done
    loader="$(/sbin/ldconfig -p | awk '/libvulkan\.so\.1 \(libc6,x86-64\)/ && !found { print $NF; found = 1 }')"
    if [ -z "$manifest" ] || [ -z "$loader" ]; then
        skip_package "lavapipe or the Vulkan loader not found (mesa-vulkan-drivers, libvulkan1); the AppImage has no software Vulkan fallback"
        return
    fi
    driver="$(sed -n 's/.*"library_path": *"\([^"]*\)".*/\1/p' "$manifest")"
    # A bare soname (newer Mesa) is found through the linker cache.
    case "$driver" in
        /*) ;;
        *) driver="$(/sbin/ldconfig -p | awk -v name="$driver" '$1 == name && /x86-64/ && !found { print $NF; found = 1 }')" ;;
    esac
    if [ ! -f "$driver" ]; then
        skip_package "lavapipe manifest $manifest names a missing driver: $driver"
        return
    fi
    bundle_libraries "$driver" "$dir"
    cp -L "$driver" "$dir/libvulkan_lvp.so"
    cp -L "$loader" "$dir/libvulkan.so.1"
    chmod 755 "$dir/libvulkan_lvp.so" "$dir/libvulkan.so.1"
    patchelf --set-rpath '$ORIGIN' "$dir/libvulkan_lvp.so"
    # The loader resolves a relative library path against the manifest.
    sed 's#"library_path": *"[^"]*"#"library_path": "./libvulkan_lvp.so"#' \
        "$manifest" > "$dir/lvp_icd.json"
    echo "Bundled lavapipe ($(du -sh "$dir" | cut -f1))"
}

build_appimage() {
    local appdir="$stage/CodexMinibar.AppDir"
    local tools="target/appimage-tools"
    local app_id="io.github.vertopolkalf.CodexMinibar"
    local output="$dist/codex-minibar-$version-$arch.AppImage"
    mkdir -p "$tools"
    fetch_tool "$appimagetool_url" "$appimagetool_sha256" "$tools/appimagetool"
    fetch_tool "$appimage_runtime_url" "$appimage_runtime_sha256" "$tools/runtime"
    chmod 755 "$tools/appimagetool"

    install_tree "$appdir/usr"
    bundle_libraries "$binary" "$appdir/usr/lib"
    # An rpath instead of LD_LIBRARY_PATH: the app starts provider CLIs, and
    # those must not inherit these libraries.
    patchelf --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/codex-minibar"
    bundle_lavapipe "$appdir"

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

if command -v rpmbuild >/dev/null 2>&1; then
    build_rpm
else
    skip_package "rpmbuild not found; skipping the .rpm"
fi

if [ "$arch" = x86_64 ]; then
    write_pkgbuild
fi

if [ "$arch" != x86_64 ]; then
    echo "The AppImage is built for x86_64 only; skipping it" >&2
elif command -v patchelf >/dev/null 2>&1 && command -v curl >/dev/null 2>&1; then
    build_appimage
else
    skip_package "patchelf or curl not found; skipping the AppImage"
fi
