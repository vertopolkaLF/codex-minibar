# Codex Minibar

Windows-first Rust desktop utility for displaying Codex limits in configurable tray icons, exploring usage statistics, and optionally activating a new five-hour limit window.

The UI is built with [windows-reactor](https://github.com/microsoft/windows-rs/pull/4479) (WinUI 3). Requires the Windows App SDK runtime (bundled via `windows-reactor-setup` self-contained deployment).

## Development

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
```

CI runs the same formatting, linting, test, and release-build gates on Windows with the
toolchain pinned in `rust-toolchain.toml`.
