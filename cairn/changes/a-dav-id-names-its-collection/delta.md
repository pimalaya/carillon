---
cairn: delta
change: a-dav-id-names-its-collection
---

# Delta

Both requirements are in cairn/spec/daemon.md. An account watches one collection, one way gains the encoded path, the trailing slash and the refused id, its scenarios the trailing slash and two new ones; A failure carries a stable code names the refused id under `calendar-not-found` and `addressbook-not-found`.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: An account watches one collection, one way
An account SHALL watch the collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection of the same domain is a second account, which is also how it gets its own hooks. A backend serving several domains MAY name one collection per domain, since the domains do not share an event name and each therefore already has hooks of its own; what they share is the connection and the credential, which is what a second account would waste. Every backend SHALL name its collection by the id a listing of the account gives it, so a collection picked from a listing is watched without translation. A DAV block SHALL name `server`, the URL its principal is discovered from, or `home`, the calendar or addressbook home set pinned, or both, and SHALL be refused when read naming both when it names neither; `home` set, discovery is skipped, the connection opens on it and `server` is not read. A DAV collection named by one path segment SHALL be looked up under the pinned `home`, or else under the calendar or addressbook home set the server's principal names, read under `server` when the server names none; an absolute path SHALL be taken as it stands, and any other relative path read under the URL the connection opens on, `home` or `server`. A relative path SHALL be sent as io-webdav's `collection_path` composes one, each byte a path segment cannot hold raw percent-encoded and an escape already there kept, and every collection path SHALL end with a slash. A DAV collection id naming no collection (empty, `/` alone, or holding an empty, `.` or `..` segment, percent-encoded or split on `\` alike, as io-webdav's `check_collection_id` reads it) SHALL be refused by `watch` and `account check` before a connection is opened, coded `calendar-not-found` or `addressbook-not-found`: it would address the server root, the home set or a parent of either.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

#### Scenario: A calendar named by its listed id
- **GIVEN** a CalDAV server whose principal names the home set `/dav/cal/alice/`, holding `/dav/cal/alice/default/`
- **WHEN** an account sets `caldav.calendar = "default"`
- **THEN** the watch reads `/dav/cal/alice/default/`, whatever path `server` names

#### Scenario: A pinned home set
- **GIVEN** an account setting `caldav.home = "https://dav.example.org/dav/cal/alice/"` and `caldav.calendar = "default"`, and no `server`
- **WHEN** it is watched or checked
- **THEN** `/dav/cal/alice/default/` is read, and no principal or home set is asked for

#### Scenario: A collection id climbing out of the home set
- **GIVEN** an account setting `caldav.calendar = ".."`
- **WHEN** `carillon --json account check` runs
- **THEN** the caldav row reads `ok: false` and `code: "calendar-not-found"`, nothing reaches the server, and the command exits 3

#### Scenario: A collection id holding a space
- **GIVEN** an account setting `carddav.addressbook = "My Book"` under the home set `/dav/books/alice/`
- **WHEN** it is watched
- **THEN** `/dav/books/alice/My%20Book/` is read

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in an `account check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli; nothing ran |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 whose body names no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV body naming `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon sends no request twice |
| `mailbox-not-found` | the mailbox a backend watches does not exist: an IMAP `NONEXISTENT` (RFC 5530 §3), a Maildir folder that is no directory, a JMAP, Gmail or Graph mailbox no listing names, Graph answering its direct read 404 or 400 `ErrorInvalidIdMalformed` |
| `calendar-not-found` | the calendar a backend watches does not exist: a DAV 404 on it, a DAV calendar id naming no collection, a JMAP, Google Calendar or Graph calendar not found as a mailbox is, a pimdir collection the store does not hold or holds under another account than `pimdir.account` |
| `addressbook-not-found` | the addressbook a backend watches does not exist: a DAV 404 on it, a DAV addressbook id naming no collection, a JMAP addressbook, Google People contact group or Graph contact folder not found as a mailbox is |
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

## REMOVED Requirements

None.
