---
cairn: delta
change: cli-alignment-2026-11-15
---

# Delta

Every requirement is in cairn/spec/daemon.md. The modified ones change as follows: A watch runs from a TOML file (`cmd` read as the alias of a hook's `command`, no longer refused), An account watches one collection, one way (DAV `home`), The account can be checked before it is watched (the family's text layout), A watch streams what it observes (text events sanitized, the session that cannot open coded), A pimdir calendar is watchable (`pimdir.account`, `calendar-not-found`), A first run points at the sample (a bare run with no configuration fails), A failure carries a stable code (the not-found codes, himalaya's pimdir relay table).

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI. A backend the build leaves out SHALL NOT be such a key: its block SHALL load unread, so one file serves every build, and a command reaching it SHALL fail coded `backend-not-built`, naming the cargo feature to build with (`check` in that backend's row, `watch` as one lost session of that account, never retried). A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hooks`), and the `dav` backend (`caldav` or `carddav`). A hook spelling of before 1.0 SHALL be refused the same way, naming what it became and MIGRATION.md: a backend's `hook` table (`hooks`), and every hook name ending in `-added`, `-removed` or `-changed` (`-add`, `-remove`, `-change`). A hook's `command` SHALL also be read under `cmd`, its alias wherever the family configures a command, a secret's included.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hooks.on-message-add` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

#### Scenario: A misspelt account key
- **GIVEN** an account holding `defualt = true`
- **WHEN** the configuration is read
- **THEN** it is refused naming `defualt` and its line, and no watch is started

#### Scenario: A backend the build leaves out
- **GIVEN** a build without the `imap` feature and an account holding an `imap` block
- **WHEN** `carillon check` runs
- **THEN** the configuration loads, the imap row reads `ok: false`, `code: "backend-not-built"` and names `--features imap`, and the command exits 3, while a misspelt `imapp` block is still refused when read

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-add.command`
- **WHEN** the configuration is read
- **THEN** it is refused naming `<backend>.hooks` and MIGRATION.md

#### Scenario: A hook named as in 0.2
- **GIVEN** an account holding `imap.hook.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused naming `hooks` and MIGRATION.md, and once that is fixed `on-message-add`, whose `cmd` then reads as its `command`

#### Scenario: A hook command spelt `cmd`
- **GIVEN** an account holding `imap.hooks.on-message-add.cmd = "true"`
- **WHEN** the configuration is read
- **THEN** it loads as `imap.hooks.on-message-add.command = "true"`, and a hook naming both is refused

### Requirement: An account watches one collection, one way
An account SHALL watch the collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection of the same domain is a second account, which is also how it gets its own hooks. A backend serving several domains MAY name one collection per domain, since the domains do not share an event name and each therefore already has hooks of its own; what they share is the connection and the credential, which is what a second account would waste. Every backend SHALL name its collection by the id a listing of the account gives it, so a collection picked from a listing is watched without translation. A DAV block SHALL name `server`, the URL its principal is discovered from, or `home`, the calendar or addressbook home set pinned, or both, and SHALL be refused when read naming both when it names neither; `home` set, discovery is skipped, the connection opens on it and `server` is not read. A DAV collection named by one path segment SHALL be looked up under the pinned `home`, or else under the calendar or addressbook home set the server's principal names, read under `server` when the server names none; an absolute path SHALL be taken as it stands, and any other relative path read under the URL the connection opens on, `home` or `server`.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

#### Scenario: A calendar named by its listed id
- **GIVEN** a CalDAV server whose principal names the home set `/dav/cal/alice/`, holding `/dav/cal/alice/default/`
- **WHEN** an account sets `caldav.calendar = "default"`
- **THEN** the watch reads `/dav/cal/alice/default`, whatever path `server` names

#### Scenario: A pinned home set
- **GIVEN** an account setting `caldav.home = "https://dav.example.org/dav/cal/alice/"` and `caldav.calendar = "default"`, and no `server`
- **WHEN** it is watched or checked
- **THEN** `/dav/cal/alice/default` is read, and no principal or home set is asked for

### Requirement: The account can be checked before it is watched
`carillon check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one. Each row SHALL be the backend check row every pimalaya binary prints, `{backend, ok, code, error}`, `code` `null` when the backend passed or its failure carries none; a backend the build leaves out SHALL be a failed row coded `backend-not-built`. When any backend failed, the command SHALL still print the report and exit 3, pimalaya-cli's partial outcome. Its text SHALL be the layout of every family `account check`: an `Account` field, then one field per backend reading `OK` or `FAIL (<error>)`, the values aligned past the longest label (pimalaya-cli's `Fields`), the account and a failure's text passed through pimalaya-cli's `sanitize::text`. It SHALL resolve the whole account through one secret resolver, so a credential command named by two of its backends is spawned once rather than once per backend, and a `pass` or `gpg` entry unlocks its store once. The resolver SHALL live no longer than the account it was built for.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits 3

#### Scenario: One credential named by two backends
- **GIVEN** an account whose `caldav` and `carddav` tables read the same `pass` entry
- **WHEN** `carillon check` runs
- **THEN** the command is spawned once, both backends are opened with its value, and the key is unlocked once

### Requirement: A watch streams what it observes
`watch` SHALL print one event line per thing it observes while it runs, each an object tagged `event` and naming its `account`: `session-opened` once a session is established, connected, authenticated and its collections read, right before it first waits for a change (`backend`, `method` `idle`, `poll` or `push`, and its `collections`, each `{name, value}`), `session-lost` when one ends or cannot open (`code`, `error`, `sources`), a session failing before it is established printing `session-lost` alone and a reopened one printing `session-opened` again once established, `changed` for every change a backend reports, whether a hook is configured for it or not (`collection` and `change`, the change as its hook receives it, tagged `kind`: `item-added`, `item-removed`, `item-changed`, `flag-added`, `flag-removed`, with its `domain`, `id` and, for a flag, `flag`), `hook-failed` when a reaction of a hook fails (`reaction`, `code`, `error`, `sources`), and `alarm-fired` when an alarm reaches its trigger (`collection`, `id`, `uid`, `kind`, `summary`, `action`, `trigger`). The account threads SHALL hand their events to the command, which alone prints them, so no two lines interleave. Once a shutdown is requested, it SHALL stop every account, print exactly one result line, `{"stoppedBy": "signal"}` (text: `Watch stopped by a signal`), and exit 0. A text event SHALL be one line whatever its strings hold: what a server, a store or the configuration hands it (an account, a collection, an id, a flag, a summary, an error and its causes) SHALL pass through pimalaya-cli's `sanitize::text`, its control and bidi characters printing as U+FFFD, as the family's text listings print them. Its printer SHALL stream: text events reach stdout even when it is a pipe, and nothing else SHALL reach it, a hook command's output going to stderr. A stdout no one reads any more SHALL stop the watch as a signal does, and its failure SHALL be the command's.

#### Scenario: A watch piped to a status bar
- **GIVEN** a Maildir account whose `on-message-add` command exits 3
- **WHEN** `carillon --json watch` runs piped, a message arrives, then SIGINT is sent
- **THEN** the pipe receives `session-opened`, `changed` (`item-added`), `hook-failed` (`command`), then `{"stoppedBy":"signal"}`, and the exit status is 0

#### Scenario: A session that cannot open
- **GIVEN** a Maildir account whose mailbox the Maildir does not hold
- **WHEN** `carillon --json watch` runs, then SIGINT is sent past the first backoff
- **THEN** stdout receives `session-lost` events coded `mailbox-not-found` and the result, never `session-opened`, and the exit status is 0

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.12 reads (pimdir draft-11, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), and optionally `pimdir.account`, the account of the store the calendar is grouped under (pimdir STORAGE §9.2), unset reading the calendar whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL poll the store's change cursor (pimdir STORAGE §4.5) every `pimdir.watch.poll.interval` (30 seconds by default), read again the items stamped above it and forget the deleted or retained ones, and read the whole collection again when the store was reset, a purge count moved or the collection itself moved. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `check` coded `calendar-not-found`, naming it; one holding no calendars, or a store of another draft, SHALL fail them naming it.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

### Requirement: A first run points at the sample
Running `carillon` with no command SHALL print the help and exit 0 when a configuration exists, one that fails to parse included, its error left to the first command reading it, and SHALL otherwise meet the missing configuration as any command does, as himalaya's does: carillon writes no configuration, and an account is written by hand against the documented sample, config.sample.toml. A command that finds no configuration SHALL fail, exit 1, naming the path it looked at (the first `-c` path when given, else the default one) and the sample; a person at a terminal SHALL meet pimalaya-cli's welcome in its place, naming the same path and sample and exiting 1, as Himalaya and Comodoro do, a script or a `--json` caller the failure alone. A configuration that fails to parse SHALL be an error rather than read as absent.

#### Scenario: A newcomer
- **GIVEN** no configuration file
- **WHEN** `carillon watch` runs
- **THEN** it exits 1 naming the path it looked at and config.sample.toml, and writes nothing

#### Scenario: A bare run with no configuration
- **GIVEN** no configuration file, and stdout and stderr no terminal
- **WHEN** `carillon` runs with no command
- **THEN** it exits 1 naming the path it looked at and config.sample.toml, as `carillon watch` does, and prints the help once a configuration exists

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in a `check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli; nothing ran |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 whose body names no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV body naming `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon sends no request twice |
| `mailbox-not-found` | the mailbox a backend watches does not exist: an IMAP `NONEXISTENT` (RFC 5530 §3), a Maildir folder that is no directory, a JMAP, Gmail or Graph mailbox no listing names, Graph answering its direct read 404 or 400 `ErrorInvalidIdMalformed` |
| `calendar-not-found` | the calendar a backend watches does not exist: a DAV 404 on it, a JMAP, Google Calendar or Graph calendar not found as a mailbox is, a pimdir collection the store does not hold or holds under another account than `pimdir.account` |
| `addressbook-not-found` | the addressbook a backend watches does not exist: a DAV 404 on it, a JMAP addressbook, Google People contact group or Graph contact folder not found as a mailbox is |
| `store-version` | the pimdir store is of another draft than the one carillon reads; neverest reinstalls it |
| `store-uncreated` | the pimdir root holds no store yet; its sync engine creates it |
| `store-foreign` | the pimdir root holds a database another program wrote |
| `store-corrupt` | the pimdir store's version records disagree |
| `store-not-current` | the pimdir store holds no schema yet |
| `store-hash-algo` | the pimdir store names its bodies by a hash carillon does not know |
| `store-exposed` | another user can reach the pimdir store, or a link stands where its files belong |
| `sync-version` | another sync engine version wrote the pimdir store's sync state |
| `store-busy`, `store-owned`, `store-staging`, `store-in-flight`, `store-reset`, `lock-replaced`, `item-duplicate`, `flag-invalid`, `action-malformed`, `conflict-unread`, `unsupported`, `no-performer`, `ambiguous-performer`, `index-missing`, `index-outdated`, `index-version`, `index-zone-unknown`, `index-fts5-missing`, `query-refused`, `query-unanswerable`, `tag-refused` | the pimdir failure io-pimdir names so, relayed as himalaya relays it, so every pimdir app reads one vocabulary; a reader taking no lock and writing nothing meets none of them today |
| `account-missing` | no account carries the name `-a` gave, `default` included |
| `account-default-missing` | `-a` named no account and none is marked default |
| `account-default-several` | `-a` named no account and several are marked default |
| `backend-not-built` | a command reached a backend block the build leaves out |

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's `session-lost` event SHALL carry the code of the failure that ended the session, `null` when it carries none.

#### Scenario: A store another draft wrote
- **GIVEN** an account whose `pimdir.root` holds a store of another pimdir draft
- **WHEN** `carillon --json check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, and the command exits 3

## ADDED Requirements

### Requirement: The standard commands are the family's
carillon SHALL offer `completion`, `manual` and `json-schema` in himalaya's order and under the abouts pimalaya-cli gives them, `completions`, `manuals` and `json-schemas` staying hidden aliases. Given no argument, each SHALL fail as himalaya's does: exit 1, the failure saying to use `--dir` (under `--json`, the failure object, `code` `null`), a directory being what one page, script or schema per command or shell goes to. carillon SHALL offer no `repl` and no `mcp`: both hold a session of commands over one account, and carillon has nothing to call twice in one: `watch` already holds every account for as long as it runs and streams what it sees, and `check` is run once before it.

#### Scenario: A man page directory left out
- **WHEN** `carillon manual` runs
- **THEN** it exits 1, its failure on stdout naming `--dir`, and nothing reaches stderr

### Requirement: A default build runs anywhere
The default cargo features SHALL include `vendored`, as every family binary's do, which builds SQLite (rusqlite's `bundled`, through io-pimdir) and D-Bus from source: a default build SHALL link neither from the system, so the binary starts outside the shell it was built in.

#### Scenario: A default build out of its devshell
- **GIVEN** `cargo build` with the default features
- **WHEN** the binary runs where no libsqlite3 and no libdbus are installed
- **THEN** it starts
