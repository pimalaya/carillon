---
cairn: delta
change: default-is-a-name
---

# Delta

Every requirement is in cairn/spec/daemon.md.

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
