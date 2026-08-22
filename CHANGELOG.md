# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Renamed from mirador. The binary, the config directory, the `CARILLON_CONFIG` variable and the systemd unit all carry the new name; the configuration file itself is unchanged, so `mv ~/.config/mirador ~/.config/carillon` is the whole migration.

### Added

- Watch every configured account at once. Bare `carillon watch` watches all of them concurrently (one thread each, shared Ctrl+C shutdown); `-a/--account` narrows it to one. Each account's mailbox comes from its own config, so `-m/--mailbox` is refused when more than one account is watched.

- Reopen a watch that ends. A session lost to a dropped connection is retried with a capped exponential backoff that a healthy session resets, and the credential is resolved again on every attempt rather than held, so a rotated secret is picked up by the next reconnect.

- Resolve an arrival's envelope for `on-message-added`. A watch learns of a new item by its id, not its subject, so the envelope is fetched on a second connection, only when that hook is configured, and only over IMAP. Adds `$date` to the templates beside `$subject`, `$sender` and `$recipient`.

- Added `imap.sasl-ir`, forcing the RFC 4959 SASL-IR initial response on or off for providers such as Coremail (126.com, 163.com) that advertise the capability and then reject the inline form.

- Initiated the project from [Himalaya CLI](https://github.com/pimalaya/himalaya) and [Neverest CLI](https://github.com/pimalaya/neverest).
- Added the JMAP backend, polling `Email/changes` and resolving the changed ids through `Email/get` to keep the ones inside the watched mailbox (requires the `jmap` cargo feature).
- Added watch event hooks, under the `hook` table of the backend that fires them (`hooks` also reads). Each backend declares only the events its protocol can express, named after what it holds: `on-message-added` and `on-message-removed` over IMAP, JMAP and Maildir, `on-card-added`, `on-card-removed` and `on-card-changed` over CardDAV, the same three under `on-event-` and `on-task-` over CalDAV, and `on-item-` over a plain DAV collection. A hook a backend can never fire is refused when the configuration is read, naming the line and the events that backend has.
- Added `on-flag-added` and `on-flag-removed`, on the backends that have flags, firing once for each flag that moved so `$flag` always names the flag it fired for. An optional `flags = [...]` filter narrows them to the flags it lists, case-insensitively and with or without the leading `\` or `$`.
- Added per-protocol TLS feature flags: `rustls-ring` (default), `rustls-aws`, `native-tls`, `vendored`.
- Added a global `-b/--backend {auto,imap,jmap,maildir,caldav,carddav,dav}` flag that pins which backend block is opened on accounts declaring more than one.

### Changed

- Removed io-email. Each backend now speaks to its own protocol crate: io-imap for the IDLE watch, io-jmap for the `Email/changes` poll, io-maildir for the listing poll. The aggregator was frozen and pinned every protocol crate to an old generation, which is what kept this tool two majors behind.

  The IMAP watch is now io-imap's own `watch::ImapMailboxWatch`, so carillon owns no watcher: it emits the same events, and no longer needs QRESYNC, since io-imap re-reads the mailbox and diffs locally against a server that lacks it.

- Flags are reported under one set of names whatever the backend spells them as, so a filter written once fires everywhere: IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter all arrive as `Seen`.

- The Maildir mailbox is resolved through io-maildir's store rather than by joining the configured root with the mailbox name. Joining by hand bypassed the layout, so on a Maildir++ store a subfolder resolved to a directory that does not exist and the watch reported nothing, forever, without ever erroring. Resolving through the store also checks that `cur`, `new` and `tmp` are there, so a wrong mailbox name fails at startup; `.` and `INBOX` both name the root.

- Added three WebDAV backends, one per domain a collection can hold: `caldav` for a calendar, `carddav` for an addressbook, `dav` for a collection naming neither. All three share one shape (`server` is the DAV root, `auth` is basic, bearer or nothing, `watch.poll.interval` the gap between two RFC 6578 `sync-collection` reports, a minute by default) and differ only in the events they fire, so a card hook on a calendar is refused rather than silent. The report asks for etags only, so a poll never carries a contact or an event. A calendar is asked for its `supported-calendar-component-set` when the watch starts: one holding a single component answers for all its members at once, and one holding both events and tasks reads a member's `getcontenttype` to tell them apart, still without reading the member. Behind a `dav` cargo feature, on by default.

- One account is one backend watching one collection, one way. `mailbox` became a required `collection` (the old name still reads), `-m/--mailbox` is gone since what an account watches is its config, and each DAV backend's `server` became the DAV root with the collection as its path. Watching a second collection is a second account, which is also how it gets its own hooks.

- Added `watch` under each backend, naming how an account learns about a change: `imap.watch.idle`, `imap.watch.poll.interval`, `jmap.watch.push.ping`, `jmap.watch.poll.interval`, `maildir.watch.poll.interval`, and `watch.poll.interval` under each of `caldav`, `carddav` and `dav`. Unset takes the best that backend has. Each backend declares only the methods it has, so asking Maildir to idle is a parse error naming the line rather than a failure at watch time.

- JMAP is pushed to again, over the RFC 8620 event-source stream, asking the server to close after each state change so the same socket carries the `Email/changes` round that follows. This closes a regression: the poll that replaced it when io-email was removed is now one method among the others.

- IMAP can poll, for a server whose idle cannot be trusted. It needed io-imap, where the watch coroutine now yields a wait for its driver to honour rather than holding a connection.

- The hook variable naming what changed is `$collection`; `$mailbox` still reaches a hook under its former name.

- Ctrl+C is now honoured within about a second on every path: idling, polling, backing off, or resolving an arrival's envelope. Each connection carries a short read deadline and hands back the not-ready failures instead of letting the transport retry them away for a minute, which is what the deadline is for. Needs the unreleased io-imap watch option, so [Cargo.toml](./Cargo.toml) patches io-imap to a local path until it ships.

- Bumped every Pimalaya dependency: io-imap 0.5, io-jmap 0.3, io-maildir 0.3, pimalaya-stream 0.3, pimalaya-cli 0.2, pimalaya-config 0.1.4. SASL moved out of pimalaya-stream into its own io-sasl crate; the `imap.sasl.*` config shape is unchanged.

- Switched hook placeholder syntax to shell-style `$name` / `${name}`. Notification summary/body are expanded with [subst](https://crates.io/crates/subst). Sender / recipient sub-fields are exposed as `sender_name` / `sender_address` / `recipient_name` / `recipient_address` so they form valid environment-variable names.

- Hook `cmd` is decoded by [pimalaya-config](https://github.com/pimalaya/config) and accepts both TOML shapes: a string handed to the platform shell (`/bin/sh -c` on Unix, `cmd /C` on Windows; quote placeholders as `"$subject"` so the shell expands them) or a `[program, args…]` list spawned directly with no shell. Template vars are exported as environment variables on the spawned process in both shapes.
- Dual-licensed under `MIT OR Apache-2.0`, aligning with the rest of the Pimalaya ecosystem (early prototypes were MIT-only).
- Switched to Rust edition 2024 (MSRV 1.89).
- Rewrote the CLI on top of [pimalaya-cli](https://github.com/pimalaya/cli), [pimalaya-config](https://github.com/pimalaya/config) and the [io-*](https://github.com/pimalaya/) coroutine crates. Replaced `tokio` with `std::thread`, `color-eyre` with `anyhow` + `pimalaya_cli::error::ErrorReport`, `tracing` with `log` + `pimalaya_cli::log::Logger`, hand-rolled `clap_complete` / `clap_mangen` with `pimalaya-cli/build`.
- Reshaped the backend block to match [himalaya CLI v2](https://github.com/pimalaya/himalaya): the `[accounts.<name>.backend]` table is gone, replaced by parallel `imap.*` / `jmap.*` / `maildir.*` dotted keys under `[accounts.<name>]`. The same TOML file can back `carillon`, `himalaya` CLI v2 and `himalaya-tui`. SASL is keyed on the mechanism name (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …); JMAP auth same shape (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`).
- Renamed `--debug` and `--trace` global flags to `--log-level {off,error,warn,info,debug,trace}` (alias `--log`).
- Replaced the `--output {plain,json}` flag with `--json`.
- Renamed the early-prototype `doctor` command to `check`; aligned its shape with `himalaya account check` (per-backend report).
- Moved `-a/--account` to a global flag (placed before the subcommand: `carillon -a work watch`), matching himalaya CLI v2.
- Adopted the `mailbox` terminology (CLI flag `-m/--mailbox`, account-config key `mailbox`) consistent with the JMAP / IMAP / Maildir vocabulary and matching the `[accounts.<name>]` schema shared with `himalaya` and `himalaya-tui`.

### Removed

- Removed the `configure` command and the wizard module (plus the `wizard` cargo feature and the `io-discovery` dependency). Edit [config.sample.toml](./config.sample.toml) by hand and place the result at one of the loaded paths; carillon will consume the Pimalaya-wide wizard if/when it lands elsewhere, but does not ship one itself.
- Removed pre-v0.1.0 `email-lib`, `pimalaya-tui`, `tokio`, `async-ctrlc`, `async-trait`, `color-eyre`, `clap_complete` and `clap_mangen` dependencies.
- Removed the in-binary keyring integration. Use a shell command via `{ command = "…" }`: any CLI that prints the secret to stdout works (`secret-tool lookup …`, `pass show …`, `security find-generic-password …`, etc.).
- Removed the in-binary OAuth 2 client. OAuth tokens are produced by an external broker such as [pimalaya/ortie](https://github.com/pimalaya/ortie) and consumed as a SASL `oauthbearer` / `xoauth2` token sourced from a shell command.

[unreleased]: https://github.com/pimalaya/carillon/commits/HEAD
