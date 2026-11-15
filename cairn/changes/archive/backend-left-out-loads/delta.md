---
cairn: delta
change: backend-left-out-loads
---

# Delta

Every requirement is in cairn/spec/daemon.md.

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI. A backend the build leaves out SHALL NOT be such a key: its block SHALL load unread, so one file serves every build, and a command reaching it SHALL fail coded `backend-not-built`, naming the cargo feature to build with (`check` in that backend's row, `watch` as one lost session of that account, never retried). A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hooks`), and the `dav` backend (`caldav` or `carddav`). A hook spelling of before 1.0 SHALL be refused the same way, naming what it became and MIGRATION.md: a backend's `hook` table (`hooks`), a hook's `cmd` (`command`), and every hook name ending in `-added`, `-removed` or `-changed` (`-add`, `-remove`, `-change`).

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
- **THEN** it is refused naming `hooks` and MIGRATION.md, and once that is fixed `on-message-add`, then `command`

### Requirement: Every configured account, or a chosen one
Bare `carillon watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account. `-a` SHALL take the account of that name, `default` being a name like any other, and a command needing one account and given no `-a` (`check`) SHALL take the account marked `default = true`. A name no account carries SHALL be an error listing the accounts the configuration does hold, and a command needing one account with no default to pick SHALL name both ways of choosing one, so a failure to resolve an account always says what to do next; the name `default` failing SHALL add that omitting `-a` takes the account marked default. One account's watch failure SHALL be printed as its `session-lost` event and retried on its own without stalling the others, but for a backend the build leaves out: its `session-lost` event, coded `backend-not-built`, SHALL be printed once and that account stopped for the run, the others watching on. When every account stopped so, nothing is left to watch, and the watch SHALL fail with that failure, coded `backend-not-built`, exiting 1.

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
- **WHEN** `carillon --json -a default check` runs
- **THEN** it fails coded `account-missing`, saying to omit `-a` to take the account marked default, and `carillon check` takes `perso`

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is printed as a `session-lost` event of that account and retried for that account alone, and the other account keeps watching

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown or a backend the build leaves out, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal. A session reopened after its connection was lost SHALL report what moved meanwhile where its backend can tell: IMAP SHALL resume from the UIDs and flags the lost watch knew, and Gmail from the history cursor the lost session had reached, both held in memory for as long as the account is watched. An IMAP session that ended on a refusal rather than a lost connection, a mailbox recreated under the same name among them, SHALL take a new baseline, as every session of the other backends does.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

#### Scenario: A message arrives during an outage
- **GIVEN** an IMAP watch whose connection was lost
- **WHEN** a message arrives before the session is reopened
- **THEN** the reopened session fires `on-message-add` for it

#### Scenario: A mailbox recreated during an outage
- **GIVEN** an IMAP watch whose connection was lost, and whose mailbox was deleted and created again meanwhile
- **WHEN** the session is reopened and the server names another UIDVALIDITY
- **THEN** that session ends, and the next one takes a new baseline

### Requirement: The account can be checked before it is watched
`carillon check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one. Each row SHALL be the backend check row every pimalaya binary prints, `{backend, ok, code, error}`, `code` `null` when the backend passed or its failure carries none; a backend the build leaves out SHALL be a failed row coded `backend-not-built`. When any backend failed, the command SHALL still print the report and exit 3, pimalaya-cli's partial outcome. It SHALL resolve the whole account through one secret resolver, so a credential command named by two of its backends is spawned once rather than once per backend, and a `pass` or `gpg` entry unlocks its store once. The resolver SHALL live no longer than the account it was built for.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits 3

#### Scenario: One credential named by two backends
- **GIVEN** an account whose `caldav` and `carddav` tables read the same `pass` entry
- **WHEN** `carillon check` runs
- **THEN** the command is spawned once, both backends are opened with its value, and the key is unlocked once

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in a `check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli; nothing ran |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 whose body names no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV body naming `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon sends no request twice |
| `store-version` | the pimdir store is of another draft than the one carillon reads; neverest reinstalls it |
| `store-uncreated` | the pimdir root holds no store yet; its sync engine creates it |
| `store-foreign` | the pimdir root holds a database another program wrote |
| `store-corrupt` | the pimdir store's version records disagree |
| `store-not-current` | the pimdir store holds no schema yet |
| `store-hash-algo` | the pimdir store names its bodies by a hash carillon does not know |
| `store-exposed` | another user can reach the pimdir store, or a link stands where its files belong |
| `sync-version` | another sync engine version wrote the pimdir store's sync state |
| `account-missing` | no account carries the name `-a` gave, `default` included |
| `account-default-missing` | `-a` named no account and none is marked default |
| `account-default-several` | `-a` named no account and several are marked default |
| `backend-not-built` | a command reached a backend block the build leaves out |

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's `session-lost` event SHALL carry the code of the failure that ended the session, `null` when it carries none.

#### Scenario: A store another draft wrote
- **GIVEN** an account whose `pimdir.root` holds a store of another pimdir draft
- **WHEN** `carillon --json check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, and the command exits 3
