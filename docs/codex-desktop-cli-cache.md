# Codex Desktop CLI cache

Discovery returns installed CLI source paths without copying or deleting files.
Desktop package versions are ordered numerically in both registry and WindowsApps
discovery. Explicit CLI paths retain their existing priority.

Before quota fallback, activation, account login, or troubleshooting launches the desktop CLI,
Minibar prepares a copy under `%LOCALAPPDATA%/Codex Minibar/desktop-cli/<package>`.
Preparation is serialized. Copies are published from unique temporary files;
an unsuccessful replacement never deletes the previous executable. A source
size/modification-time stamp avoids repeated copies and detects same-sized updates.
Existing caches without stamps are migrated when selected for use.

After successful preparation, keep the selected version and the newest other
cached version. Prune older recognized package directories and abandoned partial
copies. Never recursively delete directories: unknown files, symlinks, and Windows
reparse points are preserved. Cleanup logs the deleted version count and bytes.

A preparation lease pins the executable until launch (through the entire login
and activation operation). Windows file sharing also prevents deletion/replacement
across processes; after spawning, the running executable mapping protects the file.
Busy versions are retried on later preparations. Retention can temporarily exceed
two versions while they are busy, or permanently if a directory contains unknown
files/links or an explicitly configured CLI. Configured paths are protected whenever
settings change, including disabled instances. Copy failures retain existing versions and use an existing copy of
the matching architecture where available, without running cleanup.

For troubleshooting with a cached Codex CLI, Minibar owns the PowerShell console
child and retains its lease until the console closes. This avoids releasing the
lease when a Windows Terminal launcher exits before PowerShell has launched Codex.

Background startup maintenance also prunes existing caches to the newest two
versions without copying or launching a CLI, so OAuth-only users get the cleanup.
Further cleanup happens after successful desktop CLI preparation. Merely opening
Settings or detecting an installation does not trigger copying or cleanup.
Development tests use temporary directories; they never clean the user's cache.
