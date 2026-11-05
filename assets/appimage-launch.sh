#!/bin/sh
# Managed AppImage launcher. A separate TMPDIR keeps short activation/status
# runs from deleting the extracted files of an already running instance.
set -eu
image=@APPIMAGE@
directory="$(mktemp -d "${TMPDIR:-/tmp}/codex-minibar-appimage.XXXXXX")"
if [ "${NO_CLEANUP+x}" != x ]; then
    trap 'rm -rf -- "$directory"' 0
fi
TMPDIR="$directory" "$image" "$@"
