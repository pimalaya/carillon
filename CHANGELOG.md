# Changelog

All notable changes to this project are documented in this file, following [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Built the daemon on carillon-core using the standard Pimalaya CLI toolkit (pimalaya-cli, pimalaya-config, pimalaya-stream). The `watch` command opens a blocking TLS stream, drives core's I/O-free watch coroutine over it, and fires the account hook on every change, reconnecting with backoff until Ctrl+C.

  Configuration is `[accounts.<name>]` with an `imap` block (`server`, `login`, a `password` or `token` secret), an optional `mailbox`, and a `hook` (a desktop `notify` and/or shell `cmd`). Also ships `manuals` and `completions` commands. Credentials resolve locally (raw or command-backed secret); core stays I/O-free and never mints the event `id`/`ts`.

- Watch every configured account at once. Bare `carillon watch` now watches all accounts concurrently (one thread each, shared Ctrl+C shutdown); naming accounts positionally (`carillon watch work personal`) narrows the watch to that subset (an unknown name is an error). Each account's mailbox comes from its own config; there is no mailbox flag.

- Enriched the desktop notification locally. On a change with a `notify` hook, the daemon opens its own IMAP connection, `UID FETCH`es the newly-arrived messages' envelopes, and fires one notification per new message, exposing `$from`, `$from_name`, `$from_addr`, `$subject`, `$date`, and `$count` to the templates. carillon-core stays content-free and untouched — the enrichment is entirely CLI-side. The `cmd` hook is deliberately not enriched (its job is to kick a sync tool) and still receives only the raw `CARILLON_*` event.
