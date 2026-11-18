# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added a stable `code` to the failures a caller acts on, under `--json` in the failure object and in each `check` row, `null` for a failure carrying none: `permission-denied`, `quota-exceeded` and `rate-limited` for a server's refusal (an IMAP `NOPERM` or `OVERQUOTA`, a DAV 403, 429 or 507, a Graph, Gmail, Google Calendar, Google People or JMAP 403, 429 or 507), io-pimdir's codes for a pimdir store it cannot read (`store-version`, `store-uncreated`, `store-foreign`, `store-corrupt`, `store-not-current`, `store-hash-algo`, `store-exposed`, `sync-version`), `account-missing`, `account-default-missing` and `account-default-several` for an account `-a` could not take, and `backend-not-built` for a backend the build leaves out. A watch's `session-lost` event carries the code too, so a reader tells a refused account from a network blip.
- Added `$collection` to every hook, templates and command environment alike: the collection as the account configured it, whatever the backend calls it, so one command serves every domain.
- Added `msgraph.hooks.on-message-change`, fired when a draft is edited in place (its `changeKey` moving while no flag does).
- Added the `notify` cargo feature, on by default, gating desktop notifications. A build without it refuses a `notify` table, naming the feature, and needs no D-Bus.
- Added `caldav.hooks.on-alarm-trigger`, fired when an alarm of an event or a task of the watched calendar reaches its trigger ([pimalaya/calendula#29](https://github.com/pimalaya/calendula/discussions/29)).

  Alarms are resolved over the next 48 hours and kept current as the calendar changes, the bodies read only when this hook is set. They wait on the system's wall-clock timer through ceichas, so time asleep counts and a clock change is followed. The hook reads `$summary`, `$kind`, `$trigger`, `$at`, `$uid`, `$etag`, `$recurrenceId`, `$alarm`, `$alarmUid` and `$action`. Nothing is stored: an alarm due while carillon was not running never fires, and one reached more than 15 minutes late (a machine asleep) is dropped with a log line. Cancelled events, completed tasks, acknowledged alarms and `EMAIL` alarms never fire.

- Added the `pimdir` backend, on by default, firing the alarms of one calendar of a local pimdir store another tool keeps in sync: `pimdir.root`, `pimdir.calendar`, `pimdir.watch.poll.interval` and `pimdir.hooks.on-alarm-trigger`. The store is read and never written. It reads stores of pimdir draft-14, the ones neverest writes (io-pimdir 0.17), and refuses a store of another draft, draft-13 and draft-11 included (`store-version`), naming its root.
- Added `mailbox-not-found`, `calendar-not-found` and `addressbook-not-found`, coding a watched collection that does not exist wherever the backend can tell: an IMAP `NONEXISTENT`, a Maildir folder that is no directory, a DAV 404 on the collection, a JMAP, Gmail, Google Calendar, Google People or Microsoft Graph collection no listing names (Graph answering its direct read 404 or 400 `ErrorInvalidIdMalformed`), a pimdir calendar the store does not hold. A watch's `session-lost` event and a `check` row carry them.
- Added the pimdir codes himalaya relays to the failure codes: `store-busy`, `store-owned`, `store-staging`, `store-in-flight`, `store-reset`, `lock-replaced`, `item-duplicate`, `flag-invalid`, `action-malformed`, `conflict-unread`, `unsupported`, `no-performer`, `ambiguous-performer`, `index-missing`, `index-outdated`, `index-version`, `index-zone-unknown`, `index-fts5-missing`, `query-refused`, `query-unanswerable` and `tag-refused`, so the `carillon-error` schema names io-pimdir's vocabulary as every pimdir app does.
- Added `caldav.home` and `carddav.home`, the home set pinned as cardamum and calendula take it: discovery is skipped and the connection opens on it, `server` being optional and unread when `home` is set. A block naming neither is refused when read.
- Added `pimdir.account`, the account of the store the calendar is grouped under, as himalaya reads it: a calendar grouped under another account is `calendar-not-found`. Unset, the calendar is read whichever account it is grouped under, as before.
- Added the `account` group every family binary carries, on pimalaya-cli's shared outputs. `account check` checks an account as `check` did, and `check` runs on as its hidden alias, its `carillon-check` schema kept beside `carillon-account-check`. `account list` (alias `ls`) prints every account sorted by name with the backends it declares, `backends` the ones the build serves and `notBuilt` the ones it leaves out, marked `(not built)` in the text, opening nothing and taking no account.
- Added units to the watch durations, as pimalaya-config reads a duration: `<backend>.watch.poll.interval`, `imap.watch.idle.timeout` and `jmap.watch.push.ping` take `"90s"`, `"5m"`, `"2h"`, `"1d"` or `"1w"` beside integer seconds, which read as before.
- Added `timeout` to every hook, beside its command, read as pimalaya-config's `Timeout` (`"30s"`, `"5m"`, `90`, zero refused): a command still running once it passed is sent SIGTERM, then SIGKILL five seconds later, and printed as a `hook-failed` naming the timeout. Unset, a command runs until it ends, as before.

### Changed

- Changed config.sample.toml to the compact shape of himalaya's and neverest's: one commented key per line with a trailing note, grouped by backend, the keys every backend shares (hooks, TLS, ALPN, proxy) shown once. Every hook name is in it, and it loads as written.
- Changed `main` to install the detected SIMD CPU token first (simd-cpu 0.1), as the other Pimalaya binaries do, so the base64 and quoted-printable codecs mime-rs runs leave their portable baseline.
- **BREAKING**: Changed hook commands to run off the watch, where a running command held it: a command runs with no stdin, one instance per command of an account, the firings meanwhile waiting their turn in order (an exact duplicate dropped, 1000 waiting at most), and a shutdown sends SIGTERM to the ones running, then SIGKILL after a second. A slow hook no longer delays the next change nor drops the alarms due meanwhile.
- **BREAKING**: Changed a host that is not ASCII to be refused everywhere: url builds its IDNA without a Unicode back end (`idna_adapter` pinned to 1.0.0, as Himalaya, Cardamum, Calendula and Ortie do), so 21 crates (the ICU4X ones among them) leave the tree, and a DAV `server` or `home` or a JMAP `server` naming such a host fails with "Host `bücher.de` is not ASCII: write it in its Punycode form (xn--...)", where it was turned into its Punycode form silently. MIGRATION.md has the before and after.
- **BREAKING**: Changed `watch` to print what it observes on stdout, one event per line as it comes (`session-opened` once a session is connected, authenticated and its collections read, `session-lost`, `changed`, `hook-failed`, `alarm-fired`), then one result line once SIGINT, SIGTERM or SIGHUP stops it (`{"stoppedBy":"signal"}`, exit 0), where it printed nothing but logs and a Ctrl+C notice on stderr. Text events reach a pipe too, for a status bar. A lost session and a failed hook are events rather than `warn` log lines, a hook command exiting non-zero is now a failed hook, a hook command's stdout goes to carillon's stderr, and a second signal ends the process at once with 130. `json-schema` describes them as `carillon-watch` and `carillon-watch.events` (pimalaya-cli 51ce8b1).
- **BREAKING**: Changed the hooks to the shape Comodoro's take: the table under each backend is `hooks`, a hook's command is `command`, and every event is named with a bare verb, `on-<subject>-add`, `-remove` and `-change` (`on-message-add`, `on-flag-remove`, `on-card-change`, `on-event-add`, `on-task-remove`…). The former spellings are refused naming what replaced them and MIGRATION.md, where 0.2 read them: `<backend>.hook` and `on-message-added`, `on-message-removed`, `on-message-changed`, `on-flag-added`, `on-flag-removed`, `on-card-added`, `on-card-removed`, `on-card-changed`, `on-event-added`, `on-event-removed`, `on-event-changed`, `on-task-added`, `on-task-removed` and `on-task-changed`. `on-alarm-trigger` keeps its name, and `cmd` still reads as the alias of `command`, as everywhere the family configures a command.
- **BREAKING**: Changed the configuration to refuse a key it does not read at the root and in an account, which it ignored, as every backend table already refused one: a file is no longer shared with himalaya's account keys, and a backend the build leaves out, which 0.2 skipped, loads but fails coded `backend-not-built` naming its cargo feature when `watch` or `check` reaches it, so one file serves every build. `watch` stops such an account for the run rather than retrying it, the others watching on, and fails with that code, exit 1, when no account is left. The keys carillon moved under each backend before 0.1 are refused naming where they went and MIGRATION.md: the account-level `folder`, `mailbox`, `collection`, `watch` and `hooks`, and the `dav` backend.
- **BREAKING**: Changed a command line the grammar refuses under `--json` to print the failure object coded `usage-invalid` on stdout, nothing on stderr, still exiting 2; without `--json`, clap's message stays on stderr.
- **BREAKING**: Changed `check` to exit 3 when a backend failed, once its report is printed, and each of its rows to carry `code` beside `backend`, `ok` and `error` (pimalaya-cli's backend check row).
- **BREAKING**: Changed `-c` to take one path per flag, as pimalaya-cli's `ConfigFlags` do: repeat it to merge several files. A value is never split on `:`, so `-c a:b` now names one file called `a:b`, and the `CARILLON_CONFIG` environment variable is no longer read.
- **BREAKING**: Changed `-a` to resolve one way in `watch` and `check`, through pimalaya-config 0.5: `-a default` names the account called `default` like any other name, where `check` took the account marked default (omit `-a` for that), `check` naming an unknown account lists the ones the configuration holds, and several accounts marked default are refused naming them, where one of them was picked.
- **BREAKING**: Changed a bare `carillon` finding no configuration to fail as any command does, exit 1 naming the path it looked at and the sample (the welcome at a terminal), as himalaya's does, where it printed the help and exited 0; with a configuration, a broken one included, it still prints the help. `manual` and `completion` given no argument fail as `json-schema` and himalaya's do, exit 1 saying to use `--dir`, where they printed their help on stderr and exited 2.
- **BREAKING**: Changed the `dav` cargo feature into `caldav` and `carddav`, each named as the configuration block it builds, as cardamum and calendula name theirs: `caldav` builds the `caldav` block and the alarms of a calendar, `carddav` the `carddav` block, both on by default. A packager building with `--features dav` builds with `--features caldav,carddav` (see MIGRATION.md); no configuration key changes, and a build leaving one out names that feature when a command reaches its block.
- Changed the text of `check` to the layout of every family `account check`: `Account:` and one `<backend>:` field per backend, aligned past the longest label, where the backend lines were indented.
- Changed the text events of `watch` to sanitize what a server, a store or the configuration hands them (an account, a collection, an id, a flag, a summary, an error) through pimalaya-cli's `sanitize::text`, so a control or bidi character prints as U+FFFD and an event stays one line.
- Changed the abouts of `completion`, `manual` and `json-schema` to pimalaya-cli's and their order to himalaya's (`completion`, `manual`, `json-schema`).
- Changed the help to print each paragraph on one line whatever the terminal width, and without colour, clap taking the feature set of the other Pimalaya CLIs, where it wrapped at the terminal width (100 columns with none) and coloured itself on a terminal.
- Changed the `account` and `account list` abouts to himalaya's: "Manage accounts defined in the TOML configuration file" and "List all accounts declared in the configuration".
- Changed every refusal of a removed key or table to pimalaya-config's sentence, "The `<name>` key is removed, <why>: <what to write instead>, see MIGRATION.md" (`table` for a table), where carillon said "moved under each backend as" or "is now"; the keys refused are the same.
- Changed the default features to include `vendored`, as every family binary's do: a default build bundles SQLite and D-Bus, where it linked the system libsqlite3 and libdbus and failed to start where they were missing.
- **BREAKING**: Changed hook notifications to pimalaya-config's templates, in the shell's syntax shared with hook commands.

  `${name:default}` becomes `${name:-default}`, and the envelope variables become camelCase: `$sender_name`, `$sender_address`, `$recipient_name` and `$recipient_address` become `$senderName`, `$senderAddress`, `$recipientName` and `$recipientAddress`, in notifications and in the environment of commands alike. A notification in the former spelling is refused when the configuration is read, naming what to rewrite. A default no longer excuses a variable the hook does not have.

- Bumped crypto-bigint to 0.7.10, ctutils to 0.4.6, rustls-webpki to 0.103.16 and webpki-root-certs to 1.0.11, fixing RUSTSEC-2026-0382, RUSTSEC-2026-0383, RUSTSEC-2026-0385 and RUSTSEC-2026-0387.
- Changed `caldav.calendar` and `carddav.addressbook` named by one path segment to be looked up under the calendar or addressbook home set the server's principal names, which is the id a listing of the account gives. Absolute paths and other relative paths read as before.

- Changed `gpeople.addressbook = "contacts"` to watch every contact the account owns, and `msgraph.addressbook = "contacts"` to name the default contact folder first; a Graph contact folder or calendar is also found by id beyond the first listing page, nested contact folders included.

- Bumped pimalaya-stream to 0.4, io-gcal to 0.5, io-gmail to 0.9, io-gpeople to 0.8, io-http to 0.8, io-imap to 0.11, io-jmap to 0.10, io-maildir to 0.5, io-msgraph to 0.9, io-pimdir to 0.13, io-sasl to 0.2, io-webdav to 0.11 and pimalaya-cli to 0.5.

  A WebDAV multistatus is parsed 3.4 to 9 times faster, and an answer that is not one whole multistatus fails the round instead of reading as a shorter listing. `json-schema` now also describes `completion`, `manual` and `json-schema`.

  io-webdav 0.10 and later read a listed id percent-decoded, so the family's DAV listings print `My Book` where they printed `My%20Book`: `caldav.calendar` and `carddav.addressbook` take either. A DAV hook's `$id` keeps the spelling of the href the server serves, `%20` included.

- Bumped pimalaya-config to 0.5.2: an unknown key's error lists no removed key.

- Bumped pimalaya-cli to 0.6.0, whose listing frame carillon does not print: no output moves.

- Bumped io-imap to 0.11.2: an IMAP watch takes the QRESYNC path only when the server's `ENABLED` names QRESYNC, and diffs the whole mailbox otherwise, where a server answering its `ENABLE` with a bare `OK` had every arrival, flag change and expunge lost.

- Bumped io-pimdir to 0.16.0 (pimdir draft-13): a pimdir watch reads the stores the current neverest writes, and refuses a draft-11 one (`store-version`).

  A pimdir watch no longer reads a sync's commit half way.

- Bumped io-pimdir to 0.17.0 (pimdir draft-14): a pimdir watch reads the stores neverest writes on it, and refuses a draft-13 one (`store-version`), its message saying which of the store and carillon is the earlier.

- Bumped pimalaya-config to 0.6.0, pimalaya-cli to 0.6.1 and io-jmap to 0.10.1.

  A hook notification receives its variables as text: `&`, `<`, `>`, `"` and `'` are escaped where the daemon reads markup (every Unix but macOS), a line break or a tab turns into a space, and other control and bidirectional override characters are dropped, so `Alice <alice@example.org>` shows as written rather than as a broken tag. A failing secret command ends its message with ", run `<command>` yourself to see why", and a TOML parse error no longer prints the source line, which may hold a secret. A text error report sanitizes its chain line by line, and `--log-file` creates its file at 0600, refusing a symbolic link.

- Bumped io-gcal to 0.5.1, io-gpeople to 0.8.1 and io-msgraph to 0.9.2: their changes write series and update persons, which a watch never does, so no watch moves.

- Changed an IMAP watch reopened after a lost connection to resume from what the lost one knew.

  What moved in the mailbox during the outage is reported, where the reopened watch used to take a new baseline. A mailbox recreated meanwhile still takes one.

- Changed the Microsoft Graph mail and contacts deltas to ask for 1000 items a page (`Prefer: odata.maxpagesize`), where Graph's default of 10 made the first listing of a 20,000-message folder about 2,000 requests.

- Changed a Gmail watch reopened after a lost session to read its history from the cursor the lost one had reached.

- Changed the JMAP event stream to be reopened with the id of the last event read.

  A server keeping its events pushes what moved between two streams.

- Changed piped `--json` output to print one value per line, each schema of `json-schema` to carry an `$id` naming the contract version (`v0.2`), and added `carillon-error`, the schema of the JSON failure object (pimalaya-cli `55b6693`).

### Removed

- **BREAKING**: Removed the wizard: the `configure` command (alias `wizard`), and the offer a bare `carillon` or a command finding no configuration made.

  A configuration is written by hand from config.sample.toml. A command finding no configuration, a bare `carillon` included, fails naming the path it looked at and the sample, a person at a terminal being shown the commands creating the file from the sample instead (pimalaya-cli's welcome).

### Fixed

- Fixed `install.sh` saving an HTTP error page as the archive: the download now fails on an HTTP error, so `tar` never reads a 404 page.

- Fixed the Nix package and the release archives shipping no config.sample.toml, which the first-run welcome looks for beside the binary: package.nix installs it under share/doc/carillon/, and install.sh copies it from the archive, as neverest's do.
- Fixed install.sh fetching from soywod/carillon and asking `arm64-linux`, `x86_64-macos` and `arm64-macos`, which no release publishes: it fetches from pimalaya/carillon the `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin` archives, and installs on i686, armv6l and armv7l Linux too, as neverest's does.
- Fixed `-b` naming a backend the account has no block for failing uncoded, and `watch` retrying it for ever: `account check` and `watch` fail coded `capability-missing`, "Account `a` has no `imap` block", as himalaya refuses its `--backend`, and `watch` stops that account for the run.
- Fixed a JMAP event stream gone dead holding a push watch: one that sent nothing for three pings is opened again.
- Fixed the sample's `on-flag-add` command notifying the literal `$flag set on $id`: a list runs as written, its variables in its environment only, so the sample hands it to `sh -c`, and says so.
- Fixed a DAV watch or an IMAP envelope resolution holding a dead connection until TCP gave up, after a resume or a network change: a request the server sends nothing back to for 60 seconds fails, and the connection is reopened.
- Fixed a Gmail arrival reading `$recipient` from `To` cut at its first comma, so `"Doe, Jane" <jane@example.org>, bob@example.org` gave `"Doe`: `To` and `From` are read as an address list, their first mailbox taken as on the other backends.
- Fixed a Gmail arrival handing its hook RFC 2047 encoded words, were the API to leave any: `$subject` and the names are decoded as on IMAP.
- Fixed a CalDAV server truncating the alarm `calendar-query` leaving the alarms of the members it left out unfired for the session: they are read by `calendar-multiget`.
- Fixed an IMAP arrival handing its hook the subject and names as RFC 2047 encoded words (`=?UTF-8?Q?R=C3=A9union?=`): `$subject`, `$senderName`, `$recipientName`, `$sender` and `$recipient` are decoded, as the other mail backends and himalaya read them.
- Fixed a JMAP collection larger than one page being watched as its first page: the baseline is listed page by page, and read again when its count does not end at the server's `total`.
- Fixed a JMAP item moving while its collection was first listed being lost: the state is read before the listing, so the first round reports it.
- Fixed a JMAP watch presenting its first credential after a reconnect: a round retried on a fresh connection and every event stream present the credential resolved for them, so a rotated token no longer loses the session and the change that came with its expiry.
- Fixed a DAV server answering 401 to a block with no `auth` failing with the bare `WebDAV server returned HTTP 401`: the watch and `account check` say the server asks for credentials and the account configures no `auth`, still uncoded.
- Fixed the Ctrl+C notice of `watch` printed on stdout, which `--json` reserves for values: it goes to stderr.
- Fixed a credential command prompting for a password hanging: it reads the terminal again (pimalaya-config 0.4.1).
- Fixed a DAV watch losing what moved when the server closed its idle connection: the round is run again on a fresh connection.
- Fixed a DAV server with no `sync-collection` being unwatchable: the collection is listed on every poll instead.
- Fixed a rejected DAV sync token hiding what moved meanwhile: the collection is read again against what the watch knew.
- Fixed a calendar holding events and tasks skipping every member whose content type names no component.
- Fixed a DAV collection id naming no collection reaching the server, where `""`, `/`, `..` or `work/x/..` addressed the home set, the server root, the home set's parent or the calendar `work`: an id that is empty, `/` alone, or holds an empty, `.` or `..` segment (percent-encoded or `\`-separated forms included) is refused by `watch` and `account check` before connecting, coded `calendar-not-found` or `addressbook-not-found` (io-webdav 0.9.1's `check_collection_id`).
- Fixed a relative DAV collection id holding a byte a path segment cannot hold raw being sent as given, a `\` read as a separator: each such byte is percent-encoded as io-webdav 0.9.1 encodes one, an escape already there kept, and the collection path ends with a slash.
- Fixed `-b` leaving `pimdir` out of the order its help gives and refusing an unknown backend without naming the others: its help and its refusal list every backend as `[possible values: auto, imap, jmap, maildir, caldav, carddav, msgraph, gmail, gcal, gpeople, pimdir]`.
- Fixed `imap.sasl.scram-sha-256` failing as an unsupported mechanism: io-imap is built with its `scram` feature.
- Fixed an IMAP arrival resolving to the data of another message when the server adds a FETCH of its own to the answer.
- Fixed a Gmail history listing that does not end holding the watch: it stops on a repeated page token or past 1000 pages, and the watch resumes from now.
- Fixed a hostile or broken server reaching the watch through its protocol libraries, now released. io-http (e181530, in 0.6) caps response heads at 64 KiB and bodies at 256 MiB, fails on a body cut short instead of spinning forever, and no longer panics on a huge chunk size. io-imap (b1ab38e, in 0.8) no longer panics on a greeting carrying a literal, scans FETCH lines incrementally under a cap, and keeps passwords and SASL responses out of trace logs. io-webdav (0.7) parses a multistatus in linear time and caps its nesting at 64, where a deep one overflowed the stack.

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

[unreleased]: https://github.com/pimalaya/carillon/compare/v0.2.0..HEAD
[0.2.0]: https://github.com/pimalaya/carillon/compare/v0.1.0..v0.2.0
[0.1.0]: https://github.com/pimalaya/carillon/compare/root..v0.1.0
