# Changelog

All notable changes to this project are documented in this file, following [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Built the daemon on carillon-core using the standard Pimalaya CLI toolkit (pimalaya-cli, pimalaya-config, pimalaya-stream). The `watch` command opens a blocking TLS stream, drives core's I/O-free watch coroutine over it, and fires the account hook on every change, reconnecting with backoff until Ctrl+C.

  Configuration is `[accounts.<name>]` with an `imap` block (`server`, `login`, a `password` or `token` secret), an optional `mailbox`, and a `hook` (a content-free desktop `notify` and/or shell `cmd`; the change fields arrive as `CARILLON_*` environment variables). Also ships `manuals` and `completions` commands. Credentials resolve locally (raw or command-backed secret); core stays I/O-free and never mints the event `id`/`ts`.
