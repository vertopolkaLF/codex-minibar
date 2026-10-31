#!/usr/bin/env bash
# Builds the Linux release packages into dist/<version>/:
#   codex-minibar-<version>-linux-<arch>.tar.gz  (binary, desktop entry, icons)
#   codex-minibar_<version>_<debarch>.deb        (Debian/Ubuntu package)
#
# Usage: ./build-linux.sh [--skip-build]
# Requires: cargo, dpkg-deb (for the .deb; skipped when missing).
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

# Filesystem layout shared by both packages, rooted at a prefix.
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

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "dpkg-deb not found; skipping the .deb" >&2
    exit 0
fi

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
