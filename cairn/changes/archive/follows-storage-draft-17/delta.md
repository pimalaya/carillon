---
cairn: delta
change: follows-storage-draft-17
---

# Delta to cairn/spec/daemon.md

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.20 reads (pimdir storage-draft-17, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), and optionally `pimdir.account`, the account of the store the calendar is grouped under (pimdir STORAGE §9.2), unset reading the calendar whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL open the store again on every poll, every `pimdir.watch.poll.interval` (30 seconds by default), and read its change cursor (pimdir STORAGE §4.5): the items stamped above it are read again and the deleted or retained ones forgotten; a store of another id or incarnation, a purge count moved or the collection itself moved SHALL have the collection listed again, only the items new to the watch or holding another body read, the others kept and the missing ones forgotten. A store moved aside and synced anew under `pimdir.root` is so read on the next poll, and a store that poll cannot open SHALL end the session with its failure, the alarms already read firing on until a session reads the store again. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `account check` coded `calendar-not-found`, naming it; one holding no calendars SHALL fail them naming it. A store of another draft SHALL fail them coded `store-version`, in the wording every pimdir program shows (io-pimdir's `PimdirFormatRefusal`) naming carillon: for a store of an earlier draft, that its sync engine drains it and a new store is synced once it is moved aside; for a later one, that carillon is to be upgraded.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

#### Scenario: A store synced anew under a running watch
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store is moved aside, neverest syncs a new one under `pimdir.root` holding the event moved an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, read from the new store, and not at 08:45

#### Scenario: A store of an earlier draft
- **GIVEN** a `pimdir.root` holding a store of storage-draft-16, beside a second account watching a store of storage-draft-17
- **WHEN** `carillon --json watch` runs
- **THEN** the first account prints `session-lost` coded `store-version`, its error naming carillon and storage-draft-17 and saying to move the store aside and sync a new one, retried on its backoff, and the second account's alarms fire on time

#### Scenario: A purge elsewhere in the store
- **GIVEN** a pimdir watch over a calendar of 10,000 events
- **WHEN** the store's owner purges a retained message of another collection, and the watch polls
- **THEN** the calendar is listed again and no event body is read

### Requirement: Alarms are held in memory
A watch SHALL resolve the firings of the calendar it watches over a window ending a horizon after now (48 hours), and SHALL build it again when a member changes and when half the horizon has passed. Nothing SHALL be written between runs: a watch starting SHALL fire nothing whose trigger is before its start, and SHALL never fire one firing twice. A firing SHALL be told apart by the `UID` of the component holding its alarm, the override, the alarm's index, its repetition, its occurrence and its trigger, never by the backend's id of the item holding it, so an item read again under another id (a pimdir store synced anew, a CalDAV member moved to another href) fires nothing it already fired. A floating time SHALL resolve in the system's time zone, else as if on UTC, and zones SHALL come from the database bundled in the binary. Each account with an alarm hook SHALL wait for its next firing on a thread of its own, living as long as the account rather than its session, so the alarms a lost session read keep firing on time until the next session replaces them with what it reads. The thread SHALL wait on a ceichas timer, the wall clock counting a suspend and following a clock change, and SHALL not wake before its next firing, a move of its firings or a shutdown, which wakes every alarm thread once the shutdown is requested. The watch SHALL keep its own interval.

#### Scenario: A restart does not repeat
- **GIVEN** a watch that fired an alarm at 08:45 and was stopped at 08:46
- **WHEN** it starts again at 08:50
- **THEN** that alarm does not fire again

#### Scenario: Beyond the horizon
- **GIVEN** an event starting in three days with a `-PT15M` alarm
- **WHEN** the watch starts
- **THEN** the firing is not queued, and it is once the horizon reaches it

#### Scenario: A store synced anew after an alarm fired
- **GIVEN** a pimdir watch that fired the 08:45 alarm of the event `standup`, item 1 of its store
- **WHEN** at 08:50 the store is synced anew, `standup` becoming item 2, and the watch reads it
- **THEN** that alarm does not fire again

### Requirement: Ctrl+C is prompt on every path
`watch` SHALL install pimalaya-cli's `signal::Shutdown`: the first SIGINT (Ctrl+C), SIGTERM or SIGHUP SHALL request the shutdown, which the watch reads, and a second one SHALL end the process with 130. A requested shutdown SHALL be honoured within roughly a second on every path a watch can be waiting in: idling on a connection, sleeping between polls, backing off before a reconnect, or resolving an arrival's envelope. A watch sleeping between two polls or backing off, and the hook runner while no command runs, SHALL wake for the next round, the next command or the shutdown alone, never on a step of their own. No path SHALL wait on a server that has stopped answering: every connection the daemon opens SHALL carry a read deadline and SHALL hand back the not-ready failures rather than letting the transport retry them away, since the deadline exists to be the wakeup that re-reads the flag. A request a DAV round or an IMAP envelope resolution sends SHALL fail once the server has sent nothing for 60 seconds, so a connection gone dead under the watch (a resume, a network change) is dropped and reopened as any failed round's is, rather than held until TCP gives up.

#### Scenario: Ctrl+C while resolving against a silent server
- **GIVEN** a watch resolving an arrival's envelope against a server that has stopped answering
- **WHEN** the user presses Ctrl+C
- **THEN** the read deadline expires, the flag is seen, and the watch ends rather than waiting for the transport's own timeout

#### Scenario: A DAV connection gone dead during a suspend
- **GIVEN** a DAV watch whose connection died while the machine slept, the server never answering on it again
- **WHEN** the next round sends its report
- **THEN** the report fails after 60 seconds of silence, and the round runs again on a fresh connection

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
| `store-version` | the pimdir store is of another storage draft than the one carillon reads (storage-draft-17), its message naming carillon and which of the two is the earlier; neverest syncs a new store once the old one is moved aside |
| `store-uncreated` | the pimdir root holds no store yet; its sync engine creates it |
| `store-foreign` | the pimdir root holds a database another program wrote |
| `store-corrupt` | the pimdir store's version records disagree |
| `store-not-current` | the pimdir store holds no schema yet |
| `store-hash-algo` | the pimdir store names its bodies by a hash carillon does not know |
| `store-exposed` | another user can reach the pimdir store, or a link stands where its files belong |
| `sync-version` | another sync engine version wrote the pimdir store's sync state |
| `store-busy`, `store-owned`, `store-staging`, `store-in-flight`, `store-reset`, `lock-replaced`, `item-duplicate`, `flag-invalid`, `action-malformed`, `conflict-unread`, `unsupported`, `no-performer`, `ambiguous-performer`, `index-missing`, `index-outdated`, `index-version`, `index-zone-unknown`, `index-fts5-missing`, `query-refused`, `query-unanswerable`, `tag-refused`, `property-invalid` | the pimdir failure io-pimdir names so, relayed as himalaya relays it, so every pimdir app reads one vocabulary; a reader taking no lock and writing nothing meets none of them today |
| `account-missing` | no account carries the name `-a` gave, `default` included |
| `account-default-missing` | `-a` named no account and none is marked default |
| `account-default-several` | `-a` named no account and several are marked default |
| `backend-not-built` | a command reached a backend block the build leaves out |
| `capability-missing` | `-b` named a backend the account has no block for, as himalaya codes its `--backend` |

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's `session-lost` event SHALL carry the code of the failure that ended the session, `null` when it carries none.

#### Scenario: A store another draft wrote
- **GIVEN** an account whose `pimdir.root` holds a store of another pimdir draft
- **WHEN** `carillon --json account check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, its error naming carillon, and the command exits 3

## REMOVED Requirements

None.
