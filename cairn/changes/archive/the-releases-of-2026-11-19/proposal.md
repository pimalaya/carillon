---
cairn: change
id: the-releases-of-2026-11-19
status: landed
created: 2026-11-19
---

# The releases of 2026-11-19

## Why

io-imap 0.12.0, io-sasl 0.3.0, io-pimdir 0.18.0, pimalaya-cli 0.7.0 and ical-rs 0.7.2 were released on 2026-11-19; carillon takes them from crates.io, no patch, with one io-sasl in `cargo tree`.

## What

- Cargo.toml: io-imap `0.12`, its `imap` feature enabling `io-imap/std` (the std runtime, `ImapClientStd`, formerly `client`, which now gates the I/O-free coroutines and is pulled by `std`); io-sasl `0.3` (`scram`, which enables its new `client` feature); io-pimdir `0.18`; pimalaya-cli `0.7` (dependency and build-dependency); ical-rs `0.7.2`. Cargo.lock moves those five crates alone.
- No code moved. carillon calls neither `ImapRaw` nor `ImapClient::raw`, the IDLE watch coroutines (`io_imap::watch`) build under `std`, io-pimdir 0.18 is additive over the reader calls carillon makes and still reads draft-14, and ical-rs 0.7.2 fixes JSCalendar conversion, which carillon never runs.
- pimalaya-cli 0.7 adds `CommonCode::RequestOrigin`, so the `carillon-error` schema admits `request-origin`. carillon has no `request` command and never raises it. src/json_schema.rs `the_failure_code_is_typed_by_both_vocabularies` asserts it, failing on 0.6.1.
- cairn/spec/daemon.md: the JSON contract names the common codes; the pimdir requirement names io-pimdir 0.18.
