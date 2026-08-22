# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Renamed from mirador. The binary, the config directory, the `CARILLON_CONFIG` variable and the systemd unit all carry the new name; the configuration file itself is unchanged, so `mv ~/.config/mirador ~/.config/carillon` is the whole migration.

### Added

- Initiated the project from [Himalaya CLI](https://github.com/pimalaya/himalaya) and [Neverest CLI](https://github.com/pimalaya/neverest).

- Added the JMAP backend, watching over the RFC 8620 event stream and polling `Email/changes` where a stream cannot be held.

  The stream runs on a connection of its own, since a stream asked to close after each state change is the connection the server hangs up. A round that fails is retried once on a fresh connection, which is what a polling watch needs when its server closed the connection it slept on. Behind a `jmap` cargo feature.

- Added the CalDAV and CardDAV backends, watching a calendar or an addressbook.

  Both poll an RFC 6578 `sync-collection` report asking for etags only, so a poll carries no event and no contact. They share one shape (`server` is the DAV root, `auth` is basic, bearer or nothing) and differ only in the events they fire, so a card hook on a calendar is refused rather than staying silent. A calendar is asked which components it holds when the watch starts, so hooks it can never fire are refused there. Behind a `dav` cargo feature.

- Added watch event hooks, under the `hook` table of the backend that fires them (`hooks` also reads).

  Each backend declares only the events its protocol can express, named after what it holds: `on-message-added` and `on-message-removed` over IMAP, JMAP and Maildir, `on-card-*` over CardDAV, `on-event-*` and `on-task-*` over CalDAV. A hook a backend can never fire is refused when the configuration is read, naming the line and the events that backend has.

- Added `on-flag-added` and `on-flag-removed` on the backends that have flags, firing once for each flag that moved.

  `$flag` always names the flag it fired for, and an optional `flags = [...]` filter narrows them to the flags it lists, case-insensitively and with or without the leading `\` or `$`.

- A hook's notification is checked against the variables its event carries, when the configuration is read.

  `$id` is available everywhere, the collection under the name its backend configures it as (`$mailbox`, `$calendar`, `$addressbook`), and `$flag` to a flag hook. The envelope names reach the IMAP and JMAP arrival hooks alone, those being the two that read one. Anything else is refused, naming the hook and what it may use instead; write `${name:default}` where a hook can do without the value. A variable a hook may use but one item lacks expands to nothing rather than dropping the notification.

- Resolved an arrival's envelope for `on-message-added`, only when that hook is configured.

  A watch learns of a new item by its id, not its subject, so IMAP fetches the envelope on a second connection, never the one holding the watch. JMAP takes it from the `Email/get` its round already makes, asking for `subject`, `receivedAt`, `from` and `to` only when a hook consumes them, so an arrival costs no second request there.

- Watched every configured account at once, one thread each under a shared Ctrl+C shutdown.

  `-a/--account` narrows the watch to one. Each account watches its own collection, so nothing is passed on the command line.

- Reopened a watch that ends, after a capped exponential backoff a healthy session resets.

  The credential is resolved again on every attempt rather than held, so a rotated secret is picked up by the next reconnect.

- Added `carillon configure` (alias `wizard`), generating a working account from one prompt.

  It discovers the IMAP, JMAP, CalDAV and CardDAV services reachable from an email address (a bare domain, a `scheme://` URL and a local folder path also work), asks which one to watch and how to authenticate against it, collects the credential through the shared keyring and OAuth-broker picker, and tests the connection before writing anything. The IMAP mechanism menu is read from the server's own `CAPABILITY`, so a provider is never offered a mechanism it does not implement.

  The watch method is never asked: an account takes the best one its backend has, and writes one only when the server cannot serve it. A DAV account is completed from the server, its calendars or addressbooks listed under the home-set, and its hooks written for the components the chosen collection advertises.

  The account is then saved to the configuration file, appended to the one already there, or printed on stdout.

- A bare `carillon`, and any command finding no configuration, offers to run that wizard.

  The welcome names the file that is missing. Nothing prompts when stdin is not a terminal or when `--json` is set: both get an error naming the file and the command that would create it.

- Added the shared `--help` footer, carrying the bug tracker and the sponsoring links.

- Added `imap.sasl-ir`, forcing the RFC 4959 SASL-IR initial response on or off.

  Providers such as Coremail (126.com, 163.com) advertise the capability and then reject the inline form.

- Added a global `-b/--backend {auto,imap,jmap,maildir,caldav,carddav}` flag, pinning which block is opened on an account declaring more than one.

- Added per-protocol TLS feature flags: `rustls-ring` (default), `rustls-aws`, `native-tls`, `vendored`.

### Changed

- Removed io-email: each backend now speaks to its own protocol crate.

  io-imap holds the idle watch, io-jmap the changes poll, io-maildir the listing poll, io-webdav the collection poll. The aggregator was frozen and pinned every protocol crate to an old generation, which is what kept this tool two majors behind. The IMAP watch is io-imap's own `ImapMailboxWatch`, so carillon owns no watcher, and QRESYNC is no longer needed since io-imap re-reads and diffs locally against a server that lacks it.

- One account is one backend watching one collection, one way, and all three are its configuration.

  The collection lives under the backend that watches it, under the name its own domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar` and `carddav.addressbook`, all required. There is no account-level collection key and no `-m/--mailbox`. Watching a second collection is a second account, which is also how it gets its own hooks.

- Added `watch` under each backend, naming how an account learns about a change.

  `imap.watch.idle`, `imap.watch.poll.interval`, `jmap.watch.push.ping`, `jmap.watch.poll.interval`, `maildir.watch.poll.interval`, and `watch.poll.interval` under each of `caldav` and `carddav`. Unset takes the best method that backend has. Each backend declares only the methods it has, so asking Maildir to idle is a parse error naming the line rather than a failure at watch time.

- Flags are reported under one set of names whatever the backend spells them as.

  IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter all arrive as `Seen`, so a filter written once fires everywhere.

- The Maildir mailbox is resolved through io-maildir's store rather than by joining the root with the mailbox name.

  Joining by hand bypassed the layout, so on a Maildir++ store a subfolder resolved to a directory that does not exist and the watch reported nothing, forever, without ever erroring. The store also checks that `cur`, `new` and `tmp` are there, so a wrong mailbox name fails at startup; `.` and `INBOX` both name the root.

- Ctrl+C is honoured within about a second on every path: idling, polling, backing off, or resolving an envelope.

  Each connection carries a short read deadline and hands back the not-ready failures instead of letting the transport retry them away for a minute.

- One file no longer loads in `himalaya`, `himalaya-tui` and carillon alike, and the claim that it did has been dropped.

  An account block is still written the same way, but every backend block is strict on each side and carries keys the others do not know: carillon has `mailbox`, `watch` and `hook` under `imap`, himalaya has `alpn` and `sort`.

- Switched hook placeholders to shell-style `$name` and `${name}`, expanded with [subst](https://crates.io/crates/subst).

  Sender and recipient sub-fields are exposed as `sender_name`, `sender_address`, `recipient_name` and `recipient_address`, so they form valid environment-variable names.

- Hook `cmd` accepts both TOML shapes, a shell string or a `[program, args…]` list.

  A string is handed to the platform shell (`/bin/sh -c` on Unix, `cmd /C` on Windows; quote placeholders as `"$subject"` so the shell expands them), a list is spawned directly with no shell. Template variables are exported as environment variables on the spawned process in both shapes.

- Reshaped the account block to match [himalaya CLI v2](https://github.com/pimalaya/himalaya).

  The `[accounts.<name>.backend]` table is gone, replaced by parallel `imap.*`, `jmap.*` and `maildir.*` dotted keys under `[accounts.<name>]`. SASL is keyed on the mechanism name (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …), and JMAP auth takes the same shape (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`).

- Rewrote the CLI on top of [pimalaya-cli](https://github.com/pimalaya/cli), [pimalaya-config](https://github.com/pimalaya/config) and the [io-*](https://github.com/pimalaya) coroutine crates.

  Replaced `tokio` with `std::thread`, `color-eyre` with `anyhow` plus the shared error report, `tracing` with `log` plus the shared logger, and hand-rolled `clap_complete` and `clap_mangen` with the shared build helpers.

- Renamed the `--debug` and `--trace` flags to `--log-level {off,error,warn,info,debug,trace}` (alias `--log`), and replaced `--output {plain,json}` with `--json`.

- Renamed the early-prototype `doctor` command to `check`, aligning its shape with `himalaya account check`.

- Moved `-a/--account` to a global flag, placed before the subcommand, matching himalaya CLI v2.

- Dual-licensed under MIT OR Apache-2.0, aligning with the rest of the Pimalaya ecosystem (early prototypes were MIT-only).

- Switched to Rust edition 2024 (MSRV 1.89).

### Removed

- Removed the in-binary keyring integration.

  A secret comes from a shell command instead: any CLI printing it on stdout works, such as `secret-tool lookup`, `pass show` or `security find-generic-password`.

- Removed the in-binary OAuth 2.0 client.

  Tokens are produced by an external broker such as [ortie](https://github.com/pimalaya/ortie) and consumed as a SASL `oauthbearer` or `xoauth2` token sourced from a command.

- Removed the pre-v0.1.0 `email-lib`, `pimalaya-tui`, `tokio`, `async-ctrlc`, `async-trait`, `color-eyre`, `clap_complete` and `clap_mangen` dependencies.

[unreleased]: https://github.com/pimalaya/carillon/commits/HEAD
