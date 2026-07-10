# Codex Minibar

Windows-first Rust desktop utility for displaying Codex limits in configurable tray icons, exploring usage statistics, and optionally activating a new five-hour limit window.

The project is under active development. The current foundation contains versioned settings, the limit domain model, and a deduplicating activation scheduler.

## Development

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
```

CI runs the same formatting, linting, test, and release-build gates on Windows with the
toolchain pinned in `rust-toolchain.toml`.
