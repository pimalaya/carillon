# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **BREAKING**: renamed `completions` and `manuals` to `completion` and `manual`, the plural staying as a hidden alias.
- Spawned a credential command once per checked account, rather than once per backend naming it.

  An account whose CalDAV and CardDAV tables read the same `pass` or `gpg` entry unlocked its store twice; `carillon check` now resolves the whole account through one resolver, so the key unlocks once.

## [0.1.0] - 2026-08-22

First release, renamed from mirador. The binary, the config directory, the `CARILLON_CONFIG` variable and the systemd unit all carry the new name; a prototype configuration only needs `mv ~/.config/mirador ~/.config/carillon`.

### Added

- Watched an IMAP mailbox, over a held RFC 2177 idle connection or a poll.

  The watch is io-imap's own, so carillon owns no watcher, and it needs no QRESYNC: io-imap re-reads the mailbox and diffs locally against a server that lacks it.

  `imap.sasl` takes one of `plain`, `login`, `anonymous`, `oauthbearer`, `xoauth2` and `scram-sha-256`, and the RFC 4959 SASL-IR and RFC 2971 ID quirks are configurable for the providers that need them.

- Watched a JMAP mailbox, over the RFC 8620 event stream or an `Email/changes` poll.

  The stream runs on a connection of its own, since a stream asked to close after each state change is the connection the server hangs up. A round that fails is retried once on a fresh connection.

- Watched a local Maildir mailbox, re-listing it on an interval.

  The mailbox is resolved through io-maildir's store, so a Maildir++ subfolder resolves and a wrong name fails at startup rather than watching a directory that does not exist. `.` and `INBOX` both name the root.

- Watched a CalDAV calendar and a CardDAV addressbook, polling an RFC 6578 `sync-collection` report.

  The report asks for etags only, so a poll carries no event and no contact. A calendar is asked which components it holds when the watch starts, so hooks it can never fire are refused there.

- Fired a desktop notification, a shell command, or both, on every change.

  Hooks live under the `hook` table of the backend that fires them (`hooks` also reads), named after what that backend holds: `on-message-*` over IMAP, JMAP and Maildir, `on-card-*` over CardDAV, `on-event-*` and `on-task-*` over CalDAV, and `on-flag-*` wherever flags exist.

  A `cmd` is a string handed to the platform shell or a `[program, args…]` list spawned directly, and the placeholders reach it as environment variables either way.

- Checked every hook against what its backend and its event can carry, when the configuration is read.

  A hook a backend can never fire is refused, and so is a notification naming a variable its event cannot fill.

  `$id` is available everywhere, the collection under the name its backend configures it as (`$mailbox`, `$calendar`, `$addressbook`), `$flag` on a flag hook, and the envelope names on the IMAP and JMAP arrival hooks alone. Write `${name:default}` where a hook can do without the value.

- Reported flags under one set of names whatever the backend spells them as.

  IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter all arrive as `Seen`, so a filter written once fires everywhere. A flag hook fires once per flag that moved, and `flags = [...]` narrows it to the flags it lists.

- Resolved an arrival's envelope for `on-message-added`, only when that hook is configured.

  IMAP fetches it on a second connection, never the one holding the watch. JMAP takes it from the `Email/get` its round already makes, so an arrival costs no second request there.

- Watched every configured account at once, one thread each under a shared Ctrl+C shutdown.

  `-a/--account` narrows the watch to one. A session that ends is reopened after a capped exponential backoff a healthy session resets, and the credential is resolved again on every attempt, so a rotated secret is picked up by the next reconnect.

- Generated a working account from one prompt with `carillon configure` (alias `wizard`).

  It discovers the services reachable from an email address, asks which one to watch and how to authenticate against it, collects the credential through the OS keyring and OAuth-broker picker, and tests the connection before writing anything.

  The IMAP mechanism menu is read from the server's own `CAPABILITY`, and the watch method is never asked: an account takes the best one its backend has. A DAV account is completed from the server, its calendars or addressbooks listed under the home-set.

  The account is then saved, appended to the configuration already there, or printed on stdout.

- Offered that wizard from a bare `carillon`, and from any command finding no configuration.

  The welcome names the file that is missing. Nothing prompts when stdin is not a terminal or when `--json` is set: both get an error naming the file and the command that would create it.

- Validated an account against every backend it declares with `carillon check`, opening each one the way a watch would.

- Read every secret from a command, so nothing has to be stored in the configuration.

  Any CLI printing it on stdout works, such as `secret-tool lookup`, `pass show` or an OAuth 2.0 broker like [ortie](https://github.com/pimalaya/ortie).

- Shared the `[accounts.<name>]` shape with [himalaya](https://github.com/pimalaya/himalaya) and [himalaya-tui](https://github.com/pimalaya/himalaya-tui).

  An account block is written the same way in the three, but one file does not load in all of them: every backend block is strict on each side and carries keys the others do not know.

- Gated every backend behind its own cargo feature (`imap`, `jmap`, `maildir`, `dav`, all on by default), and every TLS provider behind `rustls-ring` (default), `rustls-aws`, `native-tls` and `vendored`.

- Generated the man pages and the shell completions, and shipped a systemd user unit.

[unreleased]: https://github.com/pimalaya/carillon/compare/v0.1.0..HEAD
[0.1.0]: https://github.com/pimalaya/carillon/compare/root..v0.1.0
