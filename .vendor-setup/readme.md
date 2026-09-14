## windows-reactor-setup

Vendored from [microsoft/windows-rs](https://github.com/microsoft/windows-rs) tag `74` (WASDK 2.4.0).
Do not point this crate at the same git checkout as `.vendor`; Cargo would unify
the whole windows-rs tree onto windows-core 0.100 and break the hooks-era reactor.

Windows Reactor Setup stages the Windows App SDK runtime files needed by a
[`windows-reactor`](https://crates.io/crates/windows-reactor) application that runs
fully self-contained.

* [Getting
  started](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-reactor-setup.md)

Add it as a build dependency:

```toml
[build-dependencies]
windows-reactor-setup = "0.100"
```

Call the setup function from `build.rs`:

```rust,no_run
windows_reactor_setup::as_self_contained();
```
