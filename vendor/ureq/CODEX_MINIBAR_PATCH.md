# ureq 2.12.1 local patch

Source: crates.io ureq 2.12.1, Apache-2.0 OR MIT (licenses retained).

`src/unit.rs` emits origin-form request targets inside HTTPS CONNECT tunnels.
Previously an HTTP proxy made ureq send absolute-form even after CONNECT;
Cursor's Envoy gateway rejects that form with plain HTTP 403 Forbidden.
Plain HTTP proxy requests retain absolute-form. Query encoding is unchanged.
Regression tests cover HTTPS, explicit ports, HTTP forwarding, and queries.
