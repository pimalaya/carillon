# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

First public release of mirador. The CLI is synchronous (`std::net` end to end) and watches over IMAP IDLE, a JMAP `Email/changes` poll, or a Maildir listing poll. See [MIGRATION.md](./MIGRATION.md) if you ran a pre-v0.1.0 build.

### Added

- Watch every configured account at once. Bare `mirador watch` watches all of them concurrently (one thread each, shared Ctrl+C shutdown); `-a/--account` narrows it to one. Each account's mailbox comes from its own config, so `-m/--mailbox` is refused when more than one account is watched.

- Reopen a watch that ends. A session lost to a dropped connection is retried with a capped exponential backoff that a healthy session resets, and the credential is resolved again on every attempt rather than held, so a rotated secret is picked up by the next reconnect.

- Resolve an arrival's envelope for `on-message-added`. A watch learns of a new message by its id, not its subject, so the envelope is fetched on a second connection and only when that hook is configured. Adds `$date` to the templates beside `$subject`, `$sender` and `$recipient`.

- Added `imap.sasl-ir`, forcing the RFC 4959 SASL-IR initial response on or off for providers such as Coremail (126.com, 163.com) that advertise the capability and then reject the inline form.

- Initiated the project from [Himalaya CLI](https://github.com/pimalaya/himalaya) and [Neverest CLI](https://github.com/pimalaya/neverest).
- Added the JMAP backend, polling `Email/changes` and resolving the changed ids through `Email/get` to keep the ones inside the watched mailbox (requires the `jmap` cargo feature).
- Added four watch event hooks under the `hooks.` TOML namespace: `hooks.on-message-added`, `hooks.on-message-removed`, `hooks.on-flags-added`, `hooks.on-flags-removed`. Flag hooks accept an optional `flags = [...]` filter that narrows firing to a specific IANA-classified flag (case-insensitive, with or without the leading `\` / `$`).
- Added per-protocol TLS feature flags: `rustls-ring` (default), `rustls-aws`, `native-tls`, `vendored`.
- Added a global `-b/--backend {auto,imap,jmap,maildir}` flag that pins which backend block is opened on accounts declaring more than one.

### Changed

- Removed io-email. Each backend now speaks to its own protocol crate: io-imap for the IDLE watch, io-jmap for the `Email/changes` poll, io-maildir for the listing poll. The aggregator was frozen and pinned every protocol crate to an old generation, which is what kept this tool two majors behind.

  The IMAP watch is now io-imap's own `watch::ImapMailboxWatch`, so mirador owns no watcher: it emits the same four events, and no longer needs QRESYNC, since io-imap re-reads the mailbox and diffs locally against a server that lacks it.

- Flags are reported under one set of names whatever the backend spells them as, so a filter written once fires everywhere: IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter all arrive as `Seen`.

- Bumped every Pimalaya dependency: io-imap 0.5, io-jmap 0.3, io-maildir 0.3, pimalaya-stream 0.3, pimalaya-cli 0.2, pimalaya-config 0.1.4. SASL moved out of pimalaya-stream into its own io-sasl crate; the `imap.sasl.*` config shape is unchanged.

- Switched hook placeholder syntax to shell-style `$name` / `${name}`. Notification summary/body are expanded with [subst](https://crates.io/crates/subst). Sender / recipient sub-fields are exposed as `sender_name` / `sender_address` / `recipient_name` / `recipient_address` so they form valid environment-variable names.

- Hook `cmd` is decoded by [`pimalaya_config::command`](https://github.com/pimalaya/config) and accepts both TOML shapes: a string handed to the platform shell (`/bin/sh -c` on Unix, `cmd /C` on Windows; quote placeholders as `"$subject"` so the shell expands them) or a `[program, args…]` list spawned directly with no shell. Template vars are exported as environment variables on the spawned process in both shapes.
- Dual-licensed under `MIT OR Apache-2.0`, aligning with the rest of the Pimalaya ecosystem (early prototypes were MIT-only).
- Switched to Rust edition 2024 (MSRV 1.89).
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
