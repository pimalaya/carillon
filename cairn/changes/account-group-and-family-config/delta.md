---
cairn: delta
change: account-group-and-family-config
---

# Delta

Every requirement is in cairn/spec/daemon.md. The modified ones change as follows: A watch runs from a TOML file (every removed-key refusal in pimalaya-config's sentence), Every configured account, or a chosen one (`account check`, `account list` taking no account), The watch method belongs to the backend (durations as pimalaya-config reads them), The account can be checked before it is watched (`account check` on pimalaya-cli's types, `check` its hidden alias), Every printed output has a published schema (the `account` keys, `carillon-check` kept), A pimdir calendar is watchable (io-pimdir 0.13), A refused command line exits 2, A failure carries a stable code and The standard commands are the family's (`account check` named). The accounts can be listed is added.

## ADDED Requirements

### Requirement: The accounts can be listed
`carillon account list` (`ls` its visible alias) SHALL print every account the configuration declares, sorted by name, opening nothing and taking no account, so it runs where no account is marked default. Its output SHALL be pimalaya-cli's `account::AccountListOutput`, `{accounts: [{name, default, backends, notBuilt}]}`, `backends` the backends the account carries a block for that the build serves and `notBuilt` the ones it leaves out, each in the order `--backend auto` tries them. Its text SHALL be one line per account with no header row: the name sanitized, the backends then each left-out one followed by ` (not built)`, comma-separated, and `default` on the default account. `account` SHALL hold `list` and `check`, as in every family binary, and no `account capability`: carillon has no unified API whose capabilities a backend would serve.

#### Scenario: A build leaving a backend out
- **GIVEN** a build without the `imap` feature and an account `a`, marked default, holding an `imap` block
- **WHEN** `carillon --json account list` runs
- **THEN** it prints `{"accounts":[{"name":"a","default":true,"backends":[],"notBuilt":["imap"]}]}` and exits 0, and its text reads `a  imap (not built)  default`

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI. A backend the build leaves out SHALL NOT be such a key: its block SHALL load unread, so one file serves every build, and a command reaching it SHALL fail coded `backend-not-built`, naming the cargo feature to build with (`account check` in that backend's row, `watch` as one lost session of that account, never retried). A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hooks`), and the `dav` backend (`caldav` or `carddav`). A hook spelling of before 1.0 SHALL be refused the same way, naming what it became and MIGRATION.md: a backend's `hook` table (`hooks`), and every hook name ending in `-added`, `-removed` or `-changed` (`-add`, `-remove`, `-change`). Every such refusal SHALL read pimalaya-config's sentence for a removed key (`toml::Removed`): "The `<name>` key is removed, <why>: <what to write instead>, see MIGRATION.md", `table` in place of `key` for a table. A hook's `command` SHALL also be read under `cmd`, its alias wherever the family configures a command, a secret's included.

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
- **WHEN** `carillon account check` runs
- **THEN** the configuration loads, the imap row reads `ok: false`, `code: "backend-not-built"` and names `--features imap`, and the command exits 3, while a misspelt `imapp` block is still refused when read

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-add.command`
- **WHEN** the configuration is read
- **THEN** it is refused as "The `hooks` table is removed, the hooks being their backend's: write `<backend>.hooks` instead (`imap.hooks`), see MIGRATION.md"

#### Scenario: A hook named as in 0.2
- **GIVEN** an account holding `imap.hook.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused naming `hooks` and MIGRATION.md, and once that is fixed `on-message-add`, whose `cmd` then reads as its `command`

#### Scenario: A hook command spelt `cmd`
- **GIVEN** an account holding `imap.hooks.on-message-add.cmd = "true"`
- **WHEN** the configuration is read
- **THEN** it loads as `imap.hooks.on-message-add.command = "true"`, and a hook naming both is refused

### Requirement: Every configured account, or a chosen one
Bare `carillon watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account. `-a` SHALL take the account of that name, `default` being a name like any other, and a command needing one account and given no `-a` (`account check`) SHALL take the account marked `default = true`; `account list` takes none. A name no account carries SHALL be an error listing the accounts the configuration does hold, and a command needing one account with no default to pick SHALL name both ways of choosing one, so a failure to resolve an account always says what to do next; the name `default` failing SHALL add that omitting `-a` takes the account marked default. One account's watch failure SHALL be printed as its `session-lost` event and retried on its own without stalling the others, but for a backend the build leaves out: its `session-lost` event, coded `backend-not-built`, SHALL be printed once and that account stopped for the run, the others watching on. When every account stopped so, nothing is left to watch, and the watch SHALL fail with that failure, coded `backend-not-built`, exiting 1.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its own collection, and Ctrl+C stops them together

#### Scenario: A name no account carries
- **GIVEN** a configuration holding the accounts `perso` and `work`
- **WHEN** `carillon -a wrok watch` runs
- **THEN** it fails naming `wrok` and listing `perso, work`

#### Scenario: `-a default`
- **GIVEN** a configuration holding `perso`, marked `default = true`, and no account named `default`
- **WHEN** `carillon --json -a default account check` runs
- **THEN** it fails coded `account-missing`, saying to omit `-a` to take the account marked default, and `carillon account check` takes `perso`

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is printed as a `session-lost` event of that account and retried for that account alone, and the other account keeps watching

### Requirement: The watch method belongs to the backend
The method SHALL be configured under its backend (`imap.watch`, `jmap.watch`, `maildir.watch`, and `watch` under each of `caldav` and `carddav`) and named by its mechanism, the way a SASL mechanism and an HTTP auth scheme already are. Each backend SHALL declare only the methods it has, so a method it does not have is refused when the configuration is read rather than when the watch runs. Unset, an account SHALL watch the best way its backend has: IDLE for IMAP, a held event stream for JMAP, a poll for the backends with nothing else. Every backend SHALL offer the poll, whose interval MAY be given and otherwise takes what suits that backend. Every duration of a method (the poll's `interval`, IDLE's `timeout`, the push `ping`) SHALL be read as pimalaya-config's duration: an integer of seconds, or a string holding a number and its unit (`s`, `m`, `h`, `d`, `w`); any other value SHALL be refused naming it.

#### Scenario: A duration with its unit
- **GIVEN** an account configuring `imap.watch.poll.interval = "5m"`
- **WHEN** the configuration is read
- **THEN** the poll runs every 300 seconds, as `imap.watch.poll.interval = 300` would, and `"5 minutes"` is refused naming it

#### Scenario: A server whose IDLE cannot be trusted
- **GIVEN** an IMAP account whose server accepts IDLE and then never speaks
- **WHEN** the account configures `imap.watch.poll.interval`
- **THEN** the watch re-reads the mailbox on that interval instead, reporting the same events

#### Scenario: A method the backend does not have
- **GIVEN** a Maildir account configuring `maildir.watch.idle`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the methods that backend has, and no watch is started

### Requirement: The account can be checked before it is watched
`carillon account check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one. `carillon check`, its name before the account group, SHALL run it too, hidden from the help. The report SHALL be pimalaya-cli's `account::AccountCheckOutput`, each row its `account::BackendCheck`, `{backend, ok, code, error}`, `code` typed by carillon's vocabulary and `null` when the backend passed or its failure carries none, read as every carillon failure is (a coded error in the chain, else a typed cause a backend's reader knows); a backend the build leaves out SHALL be a failed row coded `backend-not-built`. When any backend failed, the command SHALL still print the report and exit 3, pimalaya-cli's partial outcome. Its text SHALL be the layout of every family `account check`: an `Account` field, then one field per backend reading `OK` or `FAIL (<error>)`, the values aligned past the longest label (pimalaya-cli's `Fields`), the account and a failure's text passed through pimalaya-cli's `sanitize::text`. It SHALL resolve the whole account through one secret resolver, so a credential command named by two of its backends is spawned once rather than once per backend, and a `pass` or `gpg` entry unlocks its store once. The resolver SHALL live no longer than the account it was built for.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon account check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits 3

#### Scenario: One credential named by two backends
- **GIVEN** an account whose `caldav` and `carddav` tables read the same `pass` entry
- **WHEN** `carillon account check` runs
- **THEN** the command is spawned once, both backends are opened with its value, and the key is unlocked once

#### Scenario: A script calling `check`
- **GIVEN** a script running `carillon --json check`
- **WHEN** it runs
- **THEN** it prints what `carillon --json account check` prints and exits as it does, and `carillon --help` lists `account` but not `check`

### Requirement: Every printed output has a published schema
Every command handing data to the printer SHALL return a named `*Output` type deriving `Display`, `Serialize` and `JsonSchema`, and `carillon json-schema` SHALL publish the schema of each, keyed by the command path joined with hyphens and prefixed `carillon-`. Every type reaching the printer SHALL spell its keys in camelCase, declared as `rename_all` on the type, which is the convention of the `--json` output alone and not of the TOML configuration, whose keys stay kebab-case. A command that writes files rather than data SHALL stay out of the registry. `watch` SHALL register its result under `carillon-watch` and its events under `carillon-watch.events`. `account list` and `account check` SHALL be registered under `carillon-account-list` and `carillon-account-check`, and the hidden `check` SHALL keep `carillon-check`, describing the same payload.

#### Scenario: A consumer reading the check payload
- **GIVEN** `carillon json-schema carillon-account-check`
- **WHEN** it runs
- **THEN** the JSON Schema of the `--json` payload of `carillon account check` is printed on stdout, and `carillon-check` describes the same payload

#### Scenario: Every schema at once
- **GIVEN** `carillon json-schema --dir <DIR>`
- **WHEN** it runs
- **THEN** one file per command is written there, the directory being created if it is not already

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.13 reads (pimdir draft-11, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), and optionally `pimdir.account`, the account of the store the calendar is grouped under (pimdir STORAGE §9.2), unset reading the calendar whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL poll the store's change cursor (pimdir STORAGE §4.5) every `pimdir.watch.poll.interval` (30 seconds by default), read again the items stamped above it and forget the deleted or retained ones, and read the whole collection again when the store was reset, a purge count moved or the collection itself moved. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `account check` coded `calendar-not-found`, naming it; one holding no calendars, or a store of another draft, SHALL fail them naming it.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

### Requirement: A refused command line exits 2
The command line SHALL be parsed through pimalaya-cli's `clap::parse`: help and version print as clap prints them, and a command line the grammar refuses (an unknown flag or command, a missing argument, a bad value) SHALL exit 2 before any configuration is read. With a `--json` word before any `--`, it SHALL print on stdout the failure object coded `usage-invalid`, its `error` clap's message without its `error: ` prefix nor styles, `sources` empty and `backtrace` null, and nothing on stderr; otherwise clap's rendering on stderr and nothing on stdout. Exit 1 stays the failure object and 3 a printed result that is not a full success, an `account check` with a failed backend.

#### Scenario: An unknown flag under JSON
- **WHEN** `carillon --json watch --nope` runs
- **THEN** it prints one failure object coded `usage-invalid`, its `error` naming `--nope`, on stdout and exits 2

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in an `account check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

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
- **WHEN** `carillon --json account check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, and the command exits 3

### Requirement: The standard commands are the family's
carillon SHALL offer `completion`, `manual` and `json-schema` in himalaya's order and under the abouts pimalaya-cli gives them, `completions`, `manuals` and `json-schemas` staying hidden aliases. Given no argument, each SHALL fail as himalaya's does: exit 1, the failure saying to use `--dir` (under `--json`, the failure object, `code` `null`), a directory being what one page, script or schema per command or shell goes to. carillon SHALL offer no `repl` and no `mcp`: both hold a session of commands over one account, and carillon has nothing to call twice in one: `watch` already holds every account for as long as it runs and streams what it sees, and `account check` is run once before it.

#### Scenario: A man page directory left out
- **WHEN** `carillon manual` runs
- **THEN** it exits 1, its failure on stdout naming `--dir`, and nothing reaches stderr

## REMOVED Requirements

None.
