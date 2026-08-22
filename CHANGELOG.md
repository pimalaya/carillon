# Changelog

All notable changes to this project are documented in this file, following [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Built the daemon on the standard Pimalaya CLI toolkit (pimalaya-cli, pimalaya-config, pimalaya-stream). The `watch` command opens a blocking TLS stream, drives the I/O-free watch coroutine over it, and fires the account hook on every change, reconnecting with backoff until Ctrl+C.

  Configuration is `[accounts.<name>]` with an `imap` block (`server`, `login`, a `password` or `token` secret), an optional `mailbox`, and a `hook` (a desktop `notify` and/or shell `cmd`). Also ships `manuals` and `completions` commands. Credentials resolve in the frontend (raw or command-backed secret); the watch itself stays I/O-free, so a second frontend would need a driver rather than a second watcher.

  The watch is IDLE plus EXAMINE over an opaque state token (`UIDVALIDITY:HIGHESTMODSEQ` with CONDSTORE, otherwise `UIDVALIDITY:UIDNEXT`), so the daemon signals but never syncs: no replica, no per-message delta, no diff API. Response parsing is imap-codec's, which fuzzes itself; what this crate owns is the 1 MiB parser bound, and a test that an over-bound literal ends the watch.

- Watch every configured account at once. Bare `carillon watch` now watches all accounts concurrently (one thread each, shared Ctrl+C shutdown); naming accounts positionally (`carillon watch work personal`) narrows the watch to that subset (an unknown name is an error). Each account's mailbox comes from its own config; there is no mailbox flag.

- Enriched the desktop notification locally. On a change with a `notify` hook, the daemon opens its own IMAP connection, `UID FETCH`es the newly-arrived messages' envelopes, and fires one notification per new message, exposing `$from`, `$from_name`, `$from_addr`, `$subject`, `$date`, and `$count` to the templates. The ring itself stays content-free: the enrichment happens beside the watch, never inside it. The `cmd` hook is deliberately not enriched (its job is to kick a sync tool) and still receives only the raw `CARILLON_*` event.

- Bumped the Pimalaya libraries: io-imap 0.2 to 0.5, pimalaya-stream 0.1 to 0.3, pimalaya-cli 0.1 to 0.2, pimalaya-config to 0.1.4, and the minimum supported Rust version to 1.89.

  pimalaya-stream 0.3 retries a not-ready socket for a minute by default, which is right for a client running an exchange and wrong for this daemon: it arms a one-second read deadline precisely to be woken up and poll its shutdown flag between IDLE keep-alives. The stream is opened with `Retry::Never` so those wakeups come back, and a Ctrl+C is noticed within the second rather than whenever the server next speaks.
