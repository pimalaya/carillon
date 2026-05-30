# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

First public release of mirador. The CLI is synchronous (`std::net` end to end) and ships the JMAP push backend alongside the IMAP IDLE and Maildir fsnotify ones. See [MIGRATION.md](./MIGRATION.md) if you ran a pre-v0.1.0 build.

### Added

- Initiated the project from [Himalaya CLI](https://github.com/pimalaya/himalaya) and [Neverest CLI](https://github.com/pimalaya/neverest).
- Added the JMAP backend, driven by [RFC 8620 §7.2 EventSource](https://datatracker.ietf.org/doc/html/rfc8620#section-7.2) push (requires the `jmap` cargo feature).
- Added four watch event hooks: `on-message-added`, `on-message-removed`, `on-flags-added`, `on-flags-removed`. Flag hooks accept an optional `flags = [...]` filter that narrows firing to a specific IANA-classified flag (case-insensitive, with or without the leading `\` / `$`).
- Added per-protocol TLS feature flags: `rustls-ring` (default), `rustls-aws`, `native-tls`, `vendored`.
- Added a global `-b/--backend {auto,imap,jmap,maildir}` flag that pins which backend block is opened on accounts declaring more than one.

### Changed

- Switched hook placeholder syntax to shell-style `$name` / `${name}`. Notification summary/body are expanded with [subst](https://crates.io/crates/subst); shell-command hooks receive the placeholders as environment variables and let the shell itself expand them (quote as `"$subject"` for safe whitespace handling). Sender / recipient sub-fields are exposed as `sender_name` / `sender_address` / `recipient_name` / `recipient_address` so they form valid environment-variable names.
- Switched to license AGPL-3.0-only with a per-file header (was MIT in early prototypes).
- Switched to Rust edition 2024 (MSRV 1.88).
- Rewrote the CLI on top of [pimalaya-cli](https://github.com/pimalaya/cli), [pimalaya-config](https://github.com/pimalaya/config) and the [io-*](https://github.com/pimalaya/) coroutine crates. Replaced `tokio` with `std::thread`, `color-eyre` with `anyhow` + `pimalaya_cli::error::ErrorReport`, `tracing` with `log` + `pimalaya_cli::log::Logger`, hand-rolled `clap_complete` / `clap_mangen` with `pimalaya-cli/build`.
- Reshaped the backend block to match [himalaya CLI v2](https://github.com/pimalaya/himalaya): the `[accounts.<name>.backend]` table is gone, replaced by parallel `imap.*` / `jmap.*` / `maildir.*` dotted keys under `[accounts.<name>]`. The same TOML file can back `mirador`, `himalaya` CLI v2 and `himalaya-tui`. SASL is keyed on the mechanism name (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …); JMAP auth same shape (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`).
- Renamed `--debug` and `--trace` global flags to `--log-level {off,error,warn,info,debug,trace}` (alias `--log`).
- Replaced the `--output {plain,json}` flag with `--json`.
- Renamed the early-prototype `doctor` command to `check`; aligned its shape with `himalaya account check` (per-backend report).
- Moved `-a/--account` to a global flag (placed before the subcommand: `mirador -a work watch`), matching himalaya CLI v2.
- Adopted the `mailbox` terminology (CLI flag `-m/--mailbox`, account-config key `mailbox`) consistent with the JMAP / IMAP / Maildir vocabulary and matching the `[accounts.<name>]` schema shared with `himalaya` and `himalaya-tui`.

### Removed

- Removed the `configure` command and the wizard module (plus the `wizard` cargo feature and the `io-discovery` dependency). Edit [config.sample.toml](./config.sample.toml) by hand and place the result at one of the loaded paths; mirador will consume the Pimalaya-wide wizard if/when it lands elsewhere, but does not ship one itself.
- Removed pre-v0.1.0 `email-lib`, `pimalaya-tui`, `tokio`, `async-ctrlc`, `async-trait`, `color-eyre`, `clap_complete` and `clap_mangen` dependencies.
- Removed the in-binary keyring integration. Use a shell command via `{ command = "…" }`: any CLI that prints the secret to stdout works (`secret-tool lookup …`, `pass show …`, `security find-generic-password …`, etc.).
- Removed the in-binary OAuth 2 client. OAuth tokens are produced by an external broker such as [pimalaya/ortie](https://github.com/pimalaya/ortie) and consumed as a SASL `oauthbearer` / `xoauth2` token sourced from a shell command.

[unreleased]: https://github.com/pimalaya/mirador/commits/HEAD
