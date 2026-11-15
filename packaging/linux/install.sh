#!/usr/bin/env sh
# Installs the release tarball for the current user.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
mkdir -p "$prefix/bin"
prefix="$(cd "$prefix" && pwd)"
cp -R "$here/share" "$prefix/"
# Existing theme directories keep their mtime when cp replaces icons below
# them. Invalidate desktop icon caches after both first installs and upgrades.
if [ -d "$prefix/share/icons/hicolor" ]; then
    touch "$prefix/share/icons" "$prefix/share/icons/hicolor"
fi
# install replaces the inode, so reinstalling a running binary also works.
install -m755 "$here/bin/codex-minibar" "$prefix/bin/codex-minibar"

# A graphical session may not have ~/.local/bin on PATH yet. Give both
# desktop actions an absolute command, escaping both desktop-entry layers.
desktop="$prefix/share/applications/io.github.vertopolkalf.CodexMinibar.desktop"
launcher_tmp="$(mktemp "$desktop.XXXXXX")"
trap 'rm -f "$launcher_tmp"' 0
MINIBAR_INSTALL_EXECUTABLE="$prefix/bin/codex-minibar" awk '
BEGIN {
    path = ENVIRON["MINIBAR_INSTALL_EXECUTABLE"]
    quoted = "\""
    for (i = 1; i <= length(path); i++) {
        c = substr(path, i, 1)
        if (c == "\\") quoted = quoted "\\\\\\\\"
        else if (c == "\"" || c == "$" || c == "`") quoted = quoted "\\\\" c
        else if (c == "%") quoted = quoted "%%"
        else if (c == "\n") quoted = quoted "\\n"
        else if (c == "\r") quoted = quoted "\\r"
        else if (c == "\t") quoted = quoted "\\t"
        else quoted = quoted c
    }
    quoted = quoted "\""
}
$0 == "Exec=codex-minibar" { print "Exec=" quoted; next }
$0 == "Exec=codex-minibar --show" { print "Exec=" quoted " --show"; next }
$0 == "Exec=codex-minibar --toggle" { print "Exec=" quoted " --toggle"; next }
{ print }
' "$desktop" > "$launcher_tmp"
chmod 644 "$launcher_tmp"
mv -f "$launcher_tmp" "$desktop"
command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "$prefix/share/applications" 2>/dev/null || true
echo "Installed to $prefix. The application launcher is ready; add $prefix/bin to PATH for terminal use."
