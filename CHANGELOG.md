# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Switched hook placeholder syntax from `{name}` to shell-style `$name` / `${name}`. Notification summary/body are expanded with [subst](https://crates.io/crates/subst); shell-command hooks receive the placeholders as environment variables and let the shell itself expand them (quote as `"$subject"` for safe whitespace handling). Sender / recipient sub-fields were renamed from `sender.name` / `sender.address` / `recipient.name` / `recipient.address` to `sender_name` / `sender_address` / `recipient_name` / `recipient_address` so they form valid environment-variable names.

- Renamed `doctor` to `check`; aligned its shape with `himalaya account check` (per-backend report, no `list mailboxes` step). The `doctor` name and its `check` alias are gone.

- Moved `-a/--account` to a global flag (placed before the subcommand: `mirador -a work watch`), matching himalaya CLI v2. Per-subcommand `-a` flags were removed.

### Removed

- Removed the `configure` command. Edit [config.sample.toml](./config.sample.toml) by hand and place the result at one of the loaded paths; mirador no longer ships a bootstrap step.

- Removed the wizard module, the `wizard` cargo feature, and the `io-discovery` dependency. Mirador will consume the Pimalaya-wide wizard if/when it lands elsewhere; it does not ship one itself.

## [2.0.0-rc] - 2026-05-28

Full rewrite on top of the I/O-free Pimalaya `io-*` ecosystem. The CLI is now synchronous (`std::net` end to end) and ships the JMAP push backend alongside the IMAP IDLE and Maildir fsnotify ones. The configuration schema is incompatible with v1; see [MIGRATION.md](./MIGRATION.md).

### Added

- Added the JMAP backend, driven by [RFC 8620 §7.2 EventSource](https://datatracker.ietf.org/doc/html/rfc8620#section-7.2) push (requires the `jmap` cargo feature).
- Added three new watch event hooks: `on-message-removed`, `on-flags-added`, `on-flags-removed`. Flag hooks accept an optional `flags = [...]` filter that narrows firing to a specific IANA-classified flag (case-insensitive, with or without the leading `\` / `$`).
- Added per-protocol TLS feature flags: `rustls-ring` (default), `rustls-aws`, `native-tls`, `vendored`.
- Added a global `-b/--backend {auto,imap,jmap,maildir}` flag that pins which backend block is opened on accounts declaring more than one.

### Changed

- Switched to license AGPL-3.0-only with a per-file header (was MIT).
- Switched to Rust edition 2024 (MSRV 1.87).
- Rewrote the CLI on top of [pimalaya-cli](https://github.com/pimalaya/cli), [pimalaya-config](https://github.com/pimalaya/config) and the [io-*](https://github.com/pimalaya/) coroutine crates. Replaced `tokio` with `std::thread`, `color-eyre` with `anyhow` + `pimalaya_cli::error::ErrorReport`, `tracing` with `log` + `pimalaya_cli::log::Logger`, hand-rolled `clap_complete` / `clap_mangen` with `pimalaya-cli/build`.
- Reshaped the backend block to match [himalaya CLI v2](https://github.com/pimalaya/himalaya): the `[accounts.<name>.backend]` table is gone, replaced by parallel `imap.*` / `jmap.*` / `maildir.*` dotted keys under `[accounts.<name>]`. The same TOML file can back `mirador`, `himalaya` CLI v2 and `himalaya-tui`. SASL is keyed on the mechanism name (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …); JMAP auth same shape (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`).
- Renamed `--debug` and `--trace` global flags to `--log-level {off,error,warn,info,debug,trace}` (alias `--log`).
- Replaced the `--output {plain,json}` flag with `--json`.
- Sample envelope template placeholders gained `{mailbox}`, `{flag}` and `{flags}` for use by hook strings.

### Removed

- Removed the v1 `email-lib`, `pimalaya-tui`, `tokio`, `async-ctrlc`, `async-trait`, `color-eyre`, `clap_complete` and `clap_mangen` dependencies.
- Removed the in-binary keyring integration. Use a shell command via `{ command = "secret-tool lookup …" }` (or [pimalaya/mimosa](https://github.com/pimalaya/mimosa), `pass`, `gopass`, …) as the secret source.
- Removed the in-binary OAuth 2 client. OAuth tokens are produced by an external broker such as [pimalaya/ortie](https://github.com/pimalaya/ortie) and consumed as a SASL `oauthbearer` / `xoauth2` token sourced from a shell command.

## [1.0.0] - 2025-04-09

Last release on top of `email-lib`. Subsequent development happens on the v2 branch above.

### Added

- Initiated the project from [Himalaya CLI](https://github.com/pimalaya/himalaya) and [Neverest CLI](https://github.com/pimalaya/neverest).

[unreleased]: https://github.com/pimalaya/mirador/compare/v2.0.0-rc...HEAD
[2.0.0-rc]: https://github.com/pimalaya/mirador/compare/v1.0.0...v2.0.0-rc
[1.0.0]: https://github.com/pimalaya/mirador/releases/tag/v1.0.0
