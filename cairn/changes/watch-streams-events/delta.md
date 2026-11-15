---
cairn: delta
change: watch-streams-events
---

# Delta

Every requirement is in cairn/spec/daemon.md.

## ADDED Requirements

### Requirement: A watch streams what it observes
`watch` SHALL print one event line per thing it observes while it runs, each an object tagged `event` and naming its `account`: `session-opened` when a session starts watching (`backend`, `method` `idle`, `poll` or `push`, and its `collections`, each `{name, value}`), `session-lost` when one ends or cannot open (`code`, `error`, `sources`), `changed` for every change a backend reports, whether a hook is configured for it or not (`collection` and `change`, the change as its hook receives it, tagged `kind`: `item-added`, `item-removed`, `item-changed`, `flag-added`, `flag-removed`, with its `domain`, `id` and, for a flag, `flag`), `hook-failed` when a reaction of a hook fails (`reaction`, `code`, `error`, `sources`), and `alarm-fired` when an alarm reaches its trigger (`collection`, `id`, `uid`, `kind`, `summary`, `action`, `trigger`). The account threads SHALL hand their events to the command, which alone prints them, so no two lines interleave. Once a shutdown is requested, it SHALL stop every account, print exactly one result line, `{"stoppedBy": "signal"}` (text: `Watch stopped by a signal`), and exit 0. Its printer SHALL stream: text events reach stdout even when it is a pipe, and nothing else SHALL reach it, a hook command's output going to stderr. A stdout no one reads any more SHALL stop the watch as a signal does, and its failure SHALL be the command's.

#### Scenario: A watch piped to a status bar
- **GIVEN** a Maildir account whose `on-message-add` command exits 3
- **WHEN** `carillon --json watch` runs piped, a message arrives, then SIGINT is sent
- **THEN** the pipe receives `session-opened`, `changed` (`item-added`), `hook-failed` (`command`), then `{"stoppedBy":"signal"}`, and the exit status is 0

## MODIFIED Requirements

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

### Requirement: Ctrl+C is prompt on every path
`watch` SHALL install pimalaya-cli's `signal::Shutdown`: the first SIGINT (Ctrl+C), SIGTERM or SIGHUP SHALL request the shutdown, which the watch reads, and a second one SHALL end the process with 130. A requested shutdown SHALL be honoured within roughly a second on every path a watch can be waiting in: idling on a connection, sleeping between polls, backing off before a reconnect, or resolving an arrival's envelope. No path SHALL wait on a server that has stopped answering: every connection the daemon opens SHALL carry a read deadline and SHALL hand back the not-ready failures rather than letting the transport retry them away, since the deadline exists to be the wakeup that re-reads the flag.

#### Scenario: Ctrl+C while resolving against a silent server
- **GIVEN** a watch resolving an arrival's envelope against a server that has stopped answering
- **WHEN** the user presses Ctrl+C
- **THEN** the read deadline expires, the flag is seen, and the watch ends rather than waiting for the transport's own timeout

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification (`notify`), a command (`command`, a shell line or a program and its arguments), or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A hook command's stdout SHALL be carillon's stderr, so a watch's stdout holds its events and result alone. A hook that fails SHALL be printed as a `hook-failed` event naming the reaction that failed (`notify` or `command`), a command exiting non-zero being a failure, and left behind: neither a missing notification daemon nor a broken script SHALL end the watch. A hook SHALL NOT half-fire for a reason the configuration could have been refused for: what a template may name is settled when the file is read, so the only failures left at watch time are the ones the machine around it produced.

#### Scenario: The hook script exits non-zero
- **GIVEN** an account whose hook `command` exits with an error
- **WHEN** it fires
- **THEN** a `hook-failed` event naming `command` and the exit status is printed, and the watch keeps running

### Requirement: Every printed output has a published schema
Every command handing data to the printer SHALL return a named `*Output` type deriving `Display`, `Serialize` and `JsonSchema`, and `carillon json-schema` SHALL publish the schema of each, keyed by the command path joined with hyphens and prefixed `carillon-`. Every type reaching the printer SHALL spell its keys in camelCase, declared as `rename_all` on the type, which is the convention of the `--json` output alone and not of the TOML configuration, whose keys stay kebab-case. A command that writes files rather than data SHALL stay out of the registry. `watch` SHALL register its result under `carillon-watch` and its events under `carillon-watch.events`.

#### Scenario: A consumer reading the check payload
- **GIVEN** `carillon json-schema carillon-check`
- **WHEN** it runs
- **THEN** the JSON Schema of the `--json` payload of `carillon check` is printed on stdout

#### Scenario: Every schema at once
- **GIVEN** `carillon json-schema --dir <DIR>`
- **WHEN** it runs
- **THEN** one file per command is written there, the directory being created if it is not already

### Requirement: The JSON contract is described and versioned
`carillon json-schema` SHALL describe, under the key `carillon-error`, the failure object every command prints under `--json`, its `code` typed by the code vocabulary and pimalaya-cli's `usage-invalid`. Every generated schema SHALL carry an `$id` naming the contract version, which is the minor version while Carillon is below 1.0 (`v0.2`) and the major version after. Every registered payload SHALL be a JSON object, checked by a test. Piped `--json` output SHALL print one value per line, and nothing else SHALL reach stdout, logs included.

#### Scenario: A caller reads a failure against its schema
- **GIVEN** `carillon json-schema carillon-error`
- **WHEN** it runs
- **THEN** the schema of the JSON failure object is printed, its `$id` being `https://pimalaya.org/schemas/carillon/v0.2/carillon-error.json`

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
