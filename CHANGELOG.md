# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-25

Carillon 1 is the first stable release: a local watcher firing hooks on what changes in mailboxes, addressbooks and calendars, over IMAP, JMAP, Maildir, CalDAV, CardDAV, Microsoft Graph, the Google APIs and the pimdir store Neverest syncs, alarms of events and tasks included. It shares the configuration and hook shapes of the other Pimalaya CLIs, and prints what it observes as events, `--json` feeding status bars and scripts.

A 0.x configuration is not carried over: the hooks are renamed, their notifications take another template syntax, and unknown keys are refused. Start over from config.sample.toml.

### Added

- Added the `pimdir` backend, on by default, firing the alarms of one calendar of a local pimdir store that another tool, such as neverest, keeps in sync.

  It reads `pimdir.root`, `pimdir.calendar`, `pimdir.account` (the account the calendar is grouped under, any when unset), `pimdir.watch.poll.interval` and `pimdir.hooks.on-alarm-trigger`. The store is never written, and is opened again on every poll.

  It reads stores of pimdir storage-v1.0.0 through io-pimdir 1.0.0, a later store too while its `read_version` is 1, and refuses a draft's store coded `store-version`, naming its root.

- Added `caldav.hooks.on-alarm-trigger`, fired when an alarm of an event or a task of the watched calendar reaches its trigger ([pimalaya/calendula#29](https://github.com/pimalaya/calendula/discussions/29)).

  Alarms are resolved 48 hours ahead and follow the calendar as it changes. They wait on the wall clock, so time asleep counts and a clock change is followed. The hook reads `$summary`, `$kind`, `$trigger`, `$at`, `$uid`, `$etag`, `$recurrenceId`, `$alarm`, `$alarmUid` and `$action`.

  Nothing is stored: an alarm due while carillon was not running never fires, and one reached more than 15 minutes late is dropped with a log line. Cancelled events, completed tasks, acknowledged alarms and `EMAIL` alarms never fire.

- Added `msgraph.hooks.on-message-change`, fired when a draft is edited in place.
- Added `$collection` to every hook, in notifications and command environments: the collection as the account configured it, whatever the backend calls it.
- Added `timeout` to every hook (`"30s"`, `"5m"`, `90`, zero refused).

  A command still running past it is sent SIGTERM, then SIGKILL five seconds later, and reported as a `hook-failed` naming the timeout. Unset, a command runs until it ends.

- Added `account list` (alias `ls`), printing every account sorted by name with the backends it declares.

  Under `--json`, `backends` holds the ones the build serves and `notBuilt` the ones it leaves out, which the text marks `(not built)`. It opens nothing and takes no `-a`.

- Added `account check`, replacing `check`, which stays as its hidden alias with its `carillon-check` schema beside `carillon-account-check`.
- Added a stable `code` to the `--json` failure object and to each `account check` row, `null` for a failure carrying none.

  `permission-denied`, `quota-exceeded` and `rate-limited` code a server's refusal (an IMAP `NOPERM` or `OVERQUOTA`, an HTTP 403, 429 or 507). `mailbox-not-found`, `calendar-not-found` and `addressbook-not-found` code a watched collection that does not exist, wherever the backend can tell.

  `account-missing`, `account-default-missing` and `account-default-several` code an account `-a` could not take. A pimdir store carillon cannot read gets io-pimdir's codes: `store-version`, `store-uncreated`, `store-foreign`, `store-corrupt`, `store-not-current`, `store-hash-algo`, `store-exposed` and `sync-version`.

  A watch's `session-lost` event carries the code too, so a reader tells a refused account from a network blip.

- Added the rest of io-pimdir's codes to the `carillon-error` schema, as every pimdir app names them.
- Added `caldav.home` and `carddav.home`, pinning the home set.

  Discovery is skipped and `server` becomes optional, unread when `home` is set. A block naming neither is refused when read.

- Added an account-level `tls` table, inherited field by field by each backend's `tls` (`provider`, `rustls.crypto`, `cert`, and `client` as one).
- Added `tls.client.cert` and `tls.client.key` on the account and every network backend, a PEM client certificate and its key for mutual TLS.
- Added units to `<backend>.watch.poll.interval`, `imap.watch.idle.timeout` and `jmap.watch.push.ping`: `"90s"`, `"5m"`, `"2h"`, `"1d"` or `"1w"`, beside integer seconds.
- Added the `notify` cargo feature, on by default, gating desktop notifications.

  A build without it needs no D-Bus and refuses a `notify` table, naming the feature.

- Added the `carillon-error` schema of the failure object to `json-schema`, and those of `completion`, `manual` and `json-schema` itself.

  Each schema carries an `$id` naming the contract version (`v1`).

- Added config.sample.toml to the release archives and the Nix package, as share/doc/carillon/config.sample.toml, which install.sh copies under its prefix.

### Changed

- **BREAKING**: Changed the hooks to the shape Comodoro's take: the table is `<backend>.hooks` and a hook's command is `command`, `cmd` staying its alias.

  Every event is named with a bare verb: `on-<subject>-added`, `-removed` and `-changed` become `on-<subject>-add`, `-remove` and `-change` (`on-message-add`, `on-flag-remove`, `on-event-change`). The former spellings, `<backend>.hook` included, are refused as unknown keys. `on-alarm-trigger` keeps its name.

- **BREAKING**: Changed hook notifications to pimalaya-config's templates, in the shell syntax hook commands use.

  `${name:default}` becomes `${name:-default}`. `$sender_name`, `$sender_address`, `$recipient_name` and `$recipient_address` become `$senderName`, `$senderAddress`, `$recipientName` and `$recipientAddress`, in command environments too.

  The former spelling is refused when the configuration is read, naming what to rewrite. A default no longer excuses a variable the hook does not have.

- **BREAKING**: Changed hook commands to run off the watch, with no stdin, one instance per command of an account.

  A slow hook no longer delays the next change nor drops the alarms due meanwhile. Firings wait their turn in order, an exact duplicate dropped, 1000 waiting at most. A shutdown sends SIGTERM to the running commands, then SIGKILL after a second.

- **BREAKING**: Changed `watch` to print what it observes on stdout, one event per line, then one result line once SIGINT, SIGTERM or SIGHUP stops it (`{"stoppedBy":"signal"}`, exit 0).

  The events are `session-opened`, `session-lost`, `changed`, `hook-failed` and `alarm-fired`, described by the `carillon-watch` and `carillon-watch.events` schemas. Text events reach a pipe too, for a status bar, a control or bidi character a server or store handed them printing as U+FFFD.

  A lost session and a failed hook are events rather than `warn` log lines, and a hook command exiting non-zero is a failed hook. A hook command's stdout goes to carillon's stderr. A second signal ends the process at once with 130.

  `hook-failed` names the `hook` and the item `id`. `alarm-fired` gives its `trigger` in UTC (`2026-11-20T15:22:22Z`), while `$trigger` and `$at` keep the system zone's offset.

- **BREAKING**: Changed the configuration to refuse an unknown key at the root and in an account, as backend tables already did.

  A file is no longer shared with himalaya's account keys. The keys moved under each backend before 0.1 (the account-level `folder`, `mailbox`, `collection`, `watch` and `hooks`, and the `dav` backend) are refused as any unknown key.

- **BREAKING**: Changed a backend the build leaves out to fail coded `backend-not-built` when a command reaches it, where 0.2 skipped it.

  The configuration still loads, so one file serves every build. `watch` stops such an account for the run, the others watching on, and fails with exit 1 when none is left.

- **BREAKING**: Changed the `dav` cargo feature into `caldav` and `carddav`, both on by default.

  A packager building with `--features dav` builds with `--features caldav,carddav`. No configuration key changes.

- **BREAKING**: Changed a host that is not ASCII to be refused.

  A DAV `server` or `home` or a JMAP `server` naming one fails asking for its Punycode form (`xn--...`), where it was converted silently.

- **BREAKING**: Changed `-c` to take one path per flag, repeated to merge several files.

  A value is no longer split on `:`, and the `CARILLON_CONFIG` environment variable is no longer read.

- **BREAKING**: Changed `-a` to resolve one way in `watch` and `account check`.

  `-a default` names the account called `default`, where `check` took the account marked default (omit `-a` for that). An unknown name lists the accounts the configuration holds, and several accounts marked default are refused, where one of them was picked.

- **BREAKING**: Changed a bare `carillon` finding no configuration to fail with exit 1, naming the path it looked at and the sample, where it printed the help and exited 0.

  With a configuration, a broken one included, it still prints the help. Under `--json` it is refused coded `usage-invalid`, exit 2.

- **BREAKING**: Changed `manual`, `completion` and `json-schema` given no argument to be refused naming `--dir`, exit 2, where `manual` and `completion` printed their help.
- **BREAKING**: Changed a command line refused under `--json` to print the failure object coded `usage-invalid` on stdout, nothing on stderr, still exiting 2.
- **BREAKING**: Changed `account check` to exit 3 when a backend failed, once its report is printed.
- **BREAKING**: Changed the `account check` output to pimalaya-cli's list of accounts.

  `--json` prints `{"accounts": [{"account", "backends"}]}`, each backend row carrying `backend`, `ok`, `error`, `code` and `tls`. The text prints `Account:` then one aligned `<backend>:` field per backend.

  `tls` is the TLS the backend resolved to, `{provider, crypto, cert, clientCert}`, or `null` for a local backend or a cleartext connection. The text appends `, TLS <parts>` to the row.

- Changed piped `--json` output to print one value per line.
- Changed the default features to include `vendored`: a default build bundles SQLite and D-Bus, where it linked the system libraries and failed to start without them.
- Changed `caldav.calendar` and `carddav.addressbook` naming one path segment to be looked up under the home set the server's principal names, the id an account listing gives.

  Absolute paths and other relative paths read as before.

- Changed `caldav.calendar` and `carddav.addressbook` to take a listed id percent-decoded or not (`My Book` or `My%20Book`).

  A DAV hook's `$id` keeps the spelling of the href the server serves, `%20` included.

- Changed `gpeople.addressbook = "contacts"` to watch every contact the account owns, and `msgraph.addressbook = "contacts"` to name the default contact folder.
- Changed an IMAP or Gmail watch reopened after a lost session to report what moved during the outage, where it took a new baseline.

  An IMAP mailbox recreated meanwhile still takes one. The JMAP event stream reopens from the last event read, so a server keeping its events pushes what moved between two streams.

- Changed config.sample.toml to Himalaya's shape: one commented key per line with a short note, a section per backend, flat `proxy` and `tls` keys, every hook listed; it loads as written and with each alternative uncommented.
- Changed the help to print each paragraph on one line whatever the terminal width, and without colour.

### Removed

- **BREAKING**: Removed the wizard: the `configure` command (alias `wizard`) and the offer to run it when no configuration is found.

  A configuration is written by hand from config.sample.toml. A command finding none at a terminal shows the commands creating it from the sample.

### Fixed

- Fixed `account check` passing a `maildir` block whose root is no Maildir or lacks the mailbox: the row opens and lists the mailbox as a watch does, failing coded `mailbox-not-found`.
- Fixed an IMAP watch losing every change against a server answering its `ENABLE` with a bare `OK`: it takes the QRESYNC path only when `ENABLED` names QRESYNC.
- Fixed IMAP flags reaching `$flag` and the `--json` events as sent (`\Seen`, `$Forwarded`).

  A system flag now reads without its backslash and `$Forwarded` reads `Passed`, as on JMAP and Maildir. Flag filters matched either spelling and are unchanged.

- Fixed IMAP and Gmail arrivals handing their hook RFC 2047 encoded words (`=?UTF-8?Q?R=C3=A9union?=`).

  `$subject`, `$sender`, `$senderName`, `$recipient` and `$recipientName` are decoded.

- Fixed a Gmail arrival reading `$recipient` from `To` cut at its first comma, so `"Doe, Jane" <jane@example.org>` gave `"Doe`.
- Fixed an IMAP arrival resolving to another message's data when the server adds a FETCH of its own to the answer.
- Fixed `imap.sasl.scram-sha-256` failing as an unsupported mechanism.
- Fixed a Gmail history listing that never ends holding the watch: it stops on a repeated page token or past 1000 pages, and the watch resumes from now.
- Fixed `-b` naming a backend the account has no block for failing uncoded, `watch` retrying it for ever.

  `account check` and `watch` fail coded `capability-missing`, and `watch` stops that account for the run.

- Fixed a JMAP round of more items than the server's `maxObjectsInGet` being refused, so none of them fired a hook.
- Fixed a JMAP round reading one `/changes` answer only, the rest of a `hasMoreChanges` batch waiting for the next push or poll.
- Fixed a JMAP watch missing what moved while its server answered `cannotCalculateChanges`: the collection is listed again and only what differs is reported.
- Fixed a JMAP collection larger than one page being watched as its first page.
- Fixed a JMAP item moving while its collection was first listed being lost.
- Fixed a JMAP watch presenting its first credential after a reconnect, so a rotated token lost the session and the change that came with its expiry.
- Fixed a dead JMAP event stream holding a push watch: one that sent nothing for three pings is opened again.
- Fixed a DAV watch or an IMAP envelope resolution holding a dead connection after a resume or a network change until TCP gave up: a request unanswered for 60 seconds fails and the connection is reopened.
- Fixed a DAV watch losing what moved when the server closed its idle connection: the round is run again on a fresh connection.
- Fixed a DAV server with no `sync-collection` being unwatchable: the collection is listed on every poll instead.
- Fixed a rejected DAV sync token hiding what moved meanwhile.
- Fixed a DAV listing reading a member the server answered with an error (a 403, a 5xx) as removed: no removal is reported and a warning is logged.
- Fixed a DAV answer that is not one whole multistatus reading as a shorter listing: the round fails.
- Fixed a calendar holding events and tasks skipping every member whose content type names no component.
- Fixed a DAV collection id naming no collection reaching the server (`""`, `/`, `..`, `work/x/..`).

  An id that is empty, `/` alone, or holds an empty, `.` or `..` segment, encoded forms included, is refused before connecting, coded `calendar-not-found` or `addressbook-not-found`.

- Fixed a relative DAV collection id holding a byte a path segment cannot hold raw being sent as given: it is percent-encoded, `\` included.
- Fixed a DAV server answering 401 to a block with no `auth` failing with a bare `WebDAV server returned HTTP 401`: the error says the account configures no `auth`.
- Fixed a Microsoft Graph contact folder or calendar past the first listing page not being found by id, nested contact folders included.
- Fixed a `gpeople` watch failing on a sync-token page whose `totalItems` is negative.
- Fixed hook notification variables read as markup where the daemon reads it: `Alice <alice@example.org>` shows as written.

  `&`, `<`, `>`, `"` and `'` are escaped, a line break or a tab turns into a space, and other control and bidirectional override characters are dropped.

- Fixed a credential command prompting for a password hanging: it reads the terminal again.
- Fixed install.sh fetching from soywod/carillon archive names no release publishes: it fetches the `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin` archives from pimalaya/carillon, and installs on i686, armv6l and armv7l Linux too.
- Fixed install.sh saving an HTTP error page as the archive: the download fails on an HTTP error.

### Security

- Fixed RUSTSEC-2026-0382, RUSTSEC-2026-0383, RUSTSEC-2026-0385 and RUSTSEC-2026-0387 by bumping crypto-bigint to 0.7.10, ctutils to 0.4.6, rustls-webpki to 0.103.16 and webpki-root-certs to 1.0.11.
- Fixed a hostile or broken server crashing, stalling or exhausting the watch.

  HTTP response heads are capped at 64 KiB and bodies at 256 MiB, a body cut short fails, and a huge chunk size no longer panics. An IMAP greeting carrying a literal no longer panics, FETCH lines are scanned under a cap, and a WebDAV multistatus is parsed in linear time, its nesting capped at 64 where a deep one overflowed the stack.

- Fixed passwords and SASL responses reaching IMAP trace logs.
- Fixed a TOML parse error printing its source line, which may hold a secret.
- Fixed a text error report printing raw the control characters a server handed it: its chain is sanitized line by line.
- Changed `--log-file` to create its file readable by its owner alone (0600) and to refuse a symbolic link.

## [0.2.0] - 2026-10-02

### Added

- Added `imap.alpn`, `jmap.alpn`, `caldav.alpn` and `carddav.alpn`.

  The ALPN identifiers offered during the TLS handshake, `[]` skipping negotiation; unset keeps the protocol default.

- Added `proxy` and `<backend>.proxy`.

  A SOCKS5 or HTTP proxy for every network backend, `all_proxy` and `https_proxy` being read when unset.

- Added `jmap.addressbook` and `jmap.calendar`.

  One JMAP account watches contacts and calendar events beside its mail, over one connection, firing `on-card-*` and `on-event-*`.

- Added the `msgraph` backend.

  Watches a Microsoft Graph mail folder, contact folder and calendar over one connection.

- Added the `gmail` backend.

  Watches a Gmail label through its history.

- Added the `gcal` backend.

  Watches a Google calendar through its sync token.

- Added the `gpeople` backend.

  Watches a Google contact group through its sync token.

- Added `carillon json-schema`.

  Generates the JSON Schema of a command's `--json` output.

### Changed

- Renamed `completions` and `manuals` to `completion` and `manual`, the plural staying as a hidden alias.

- Made `jmap.mailbox` optional, at least one JMAP collection being required.

- Offered the Microsoft Graph and Google APIs first in the wizard for a Microsoft or Google address.

- Spawned a credential command once per checked account rather than once per backend.

### Fixed

- Fixed the leading tilde and shell variables of `<backend>.tls.cert` not being expanded.

- Fixed the JMAP event stream never waking the watch up.

  The stream is now dropped as soon as it pushes a state the watch does not hold, Fastmail never closing it.

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

[unreleased]: https://github.com/pimalaya/carillon/compare/v1.0.0..HEAD
[1.0.0]: https://github.com/pimalaya/carillon/compare/v0.2.0..v1.0.0
[0.2.0]: https://github.com/pimalaya/carillon/compare/v0.1.0..v0.2.0
[0.1.0]: https://github.com/pimalaya/carillon/compare/root..v0.1.0
