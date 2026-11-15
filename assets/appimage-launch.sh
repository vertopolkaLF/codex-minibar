#!/bin/sh
# Managed AppImage launcher. A separate TMPDIR keeps short activation/status
# runs from deleting the extracted files of an already running instance.
set -eu
image=@APPIMAGE@
base="${CODEX_MINIBAR_APPIMAGE_TMPDIR:-${TMPDIR:-/tmp}}"
directory="$(mktemp -d "$base/codex-minibar-appimage.XXXXXX")"
if [ "${NO_CLEANUP+x}" != x ]; then
    trap 'rm -rf -- "$directory"' 0
fi
# A delayed updater or terminal handoff can outlive this private directory.
# Descendant launches must allocate beside it, rather than inside it.
CODEX_MINIBAR_APPIMAGE_TMPDIR="$base" TMPDIR="$directory" "$image" "$@"
