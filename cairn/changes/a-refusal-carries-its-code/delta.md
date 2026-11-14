---
cairn: delta
change: a-refusal-carries-its-code
---

# Delta

Every requirement is in cairn/spec/daemon.md.

## ADDED Requirements

### Requirement: A refused command line exits 2
The command line SHALL be parsed through pimalaya-cli's `clap::parse`: help and version print as clap prints them, and a command line the grammar refuses (an unknown flag or command, a missing argument, a bad value) SHALL exit 2 before any configuration is read. With a `--json` word before any `--`, it SHALL print on stdout the failure object coded `usage-invalid`, its `error` clap's message without its `error: ` prefix nor styles, `sources` empty and `backtrace` null, and nothing on stderr; otherwise clap's rendering on stderr and nothing on stdout. Exit 1 stays the failure object and 3 a printed result that is not a full success, a `check` with a failed backend.

#### Scenario: An unknown flag under JSON
- **WHEN** `carillon --json watch --nope` runs
- **THEN** it prints one failure object coded `usage-invalid`, its `error` naming `--nope`, on stdout and exits 2

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in a `check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

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

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's "session lost" warning SHALL name the code of the failure that ended the session when it carries one.

#### Scenario: A store another draft wrote
- **GIVEN** an account whose `pimdir.root` holds a store of another pimdir draft
- **WHEN** `carillon --json check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, and the command exits 3

## MODIFIED Requirements

### Requirement: The account can be checked before it is watched
`carillon check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one. Each row SHALL be the backend check row every pimalaya binary prints, `{backend, ok, code, error}`, `code` `null` when the backend passed or its failure carries none. When any backend failed, the command SHALL still print the report and exit 3, pimalaya-cli's partial outcome. It SHALL resolve the whole account through one secret resolver, so a credential command named by two of its backends is spawned once rather than once per backend, and a `pass` or `gpg` entry unlocks its store once. The resolver SHALL live no longer than the account it was built for.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits 3

#### Scenario: One credential named by two backends
- **GIVEN** an account whose `caldav` and `carddav` tables read the same `pass` entry
- **WHEN** `carillon check` runs
- **THEN** the command is spawned once, both backends are opened with its value, and the key is unlocked once

### Requirement: The JSON contract is described and versioned
`carillon json-schema` SHALL describe, under the key `carillon-error`, the failure object every command prints under `--json`, its `code` typed by the code vocabulary and pimalaya-cli's `usage-invalid`. Every generated schema SHALL carry an `$id` naming the contract version, which is the minor version while Carillon is below 1.0 (`v0.2`) and the major version after. Every registered payload SHALL be a JSON object, checked by a test. Piped `--json` output SHALL print one value per line, and nothing else SHALL reach stdout, a watch's shutdown notice included.

#### Scenario: A caller reads a failure against its schema
- **GIVEN** `carillon json-schema carillon-error`
- **WHEN** it runs
- **THEN** the schema of the JSON failure object is printed, its `$id` being `https://pimalaya.org/schemas/carillon/v0.2/carillon-error.json`

## REMOVED Requirements

None.
