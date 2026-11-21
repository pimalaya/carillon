---
cairn: log
change: io-family-releases
landed: 2026-11-21
---

# The io-* releases of 2026-11-21

carillon moves to io-gcal 0.6.0, io-gmail 0.10, io-gpeople 0.9.0, io-http 0.9, io-jmap 0.11, io-msgraph 0.10.0, io-pimdir 0.19.1, io-webdav 0.13.0, pimalaya-stream 0.4.1 (io-proxy 0.3.0 underneath) and, for the live tests, io-oauth 0.8. No `[patch.crates-io]`, and no code moved.

## What landed

- Cargo.toml: the family rule cargo-006 renamed the blocking client feature `client` to `std`, `client` now gating the I/O-free coroutines. The `jmap`, `msgraph`, `gmail`, `gcal` and `gpeople` features name `io-<crate>/std` where they named `io-<crate>/client`. The `caldav` and `carddav` features add `io-http/std`, as src/dav.rs drives `io_http::client::HttpClientStd`, which only the TLS features enabled before. io-pimdir keeps `client`, which did not move.
- Cargo.lock: the moved crates alone, no unrelated churn.
- io-pimdir 0.19.1 fixes summaries a sync writes; a pimdir watch reads the store and never syncs it, so nothing user-visible moved and the CHANGELOG is untouched.

## Spec

No spec moved.

## Verified

`cargo fmt --check`, `cargo build`, `cargo clippy --locked --all-targets -- -D warnings` over the CI feature sets (default, all, `rustls-ring` alone, `imap,rustls-ring`, `jmap,native-tls`, `maildir`, `caldav,rustls-aws`, and `rustls-ring` with each backend), `cargo test --all-features` (188 passed, 9 ignored: the live suites) and `cargo test --no-default-features --features rustls-ring,maildir` (70 passed).
