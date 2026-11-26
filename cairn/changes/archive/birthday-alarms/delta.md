---
cairn: delta
change: birthday-alarms
---

# Delta: birthdays and anniversaries fire a hook

Capability file: cairn/spec/daemon.md.

## ADDED Requirements

### Requirement: An addressbook fires its birthdays and anniversaries
The `carddav` and `pimdir` backends SHALL offer `on-birthday-trigger`, firing once a year per `BDAY` and per `ANNIVERSARY` (RFC 6350 §6.2.5, §6.2.6) of every card of the watched addressbook, at the hook's `at` local time (`HH:MM`, default `09:00`), `days-before` days before the date (default 0), in the machine's local zone, a vCard date being floating. It SHALL carry `$id`, `$collection` and `$addressbook`, `$name` (the card's `FN`), `$kind` (`birthday` or `anniversary`), `$date` (`1990-11-27`, or `--11-27` with no year), `$age` (the years the date completes, empty for a year-less date), `$late` (the days late, 0 on time) and `$uid` (the card's `UID`, empty when it has none). A year-less date (`--MMDD`, `--MM-DD`) SHALL fire every year with `$age` empty, and a 29 February SHALL fire on 28 February in a common year. A date given as text (`VALUE=text`) SHALL be ignored, as SHALL a date naming no day (`1990`, `1990-11`, `---27`); a time of day after a date SHALL be dropped. A date SHALL be told apart by the card's `UID` (its id when it has none), its kind and its firing instant, so a card read again under another id fires nothing it already fired. A date firing SHALL print a `birthday-fired` event before its hook runs (see the watch output). No other backend SHALL accept the hook.

#### Scenario: A birthday at nine
- **GIVEN** a CardDAV account with `carddav.hooks.on-birthday-trigger.notify.summary = "$name turns $age"`, watching an addressbook holding a card "Jane Doe" with `BDAY:1990-11-27`
- **WHEN** the local clock reaches 2026-11-27 09:00
- **THEN** one notification titled "Jane Doe turns 36" is shown

#### Scenario: A year-less birthday
- **GIVEN** a card with `BDAY:--1127`
- **WHEN** the local clock reaches 09:00 on 27 November
- **THEN** the hook fires with `$age` empty

#### Scenario: A leap-day birthday
- **GIVEN** a card with `BDAY:2000-02-29`
- **WHEN** the local clock reaches 2027-02-28 09:00
- **THEN** the hook fires

#### Scenario: A birthday hook on a calendar
- **GIVEN** an account configuring `caldav.hooks.on-birthday-trigger`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hooks a CalDAV account has

### Requirement: An edited addressbook moves its dates
A card the watch reports added or changed SHALL have its body read again and its dates replaced, and a card reported removed SHALL have its dates forgotten, before the next firing is waited for. Dates SHALL be held in memory on the alarms' horizon, beside the alarms of the same account, and a watch SHALL never fire one date twice, a restart included.

#### Scenario: A birthday corrected
- **GIVEN** a watch holding a card whose birthday fires tomorrow
- **WHEN** the card's `BDAY` is moved to the day after, and the watch polls
- **THEN** it fires the day after, and not tomorrow

### Requirement: A late birthday fires the same day only
A date reached late, the machine having slept, SHALL fire on waking when the local day is still its day, or when it is at most the hook's `late-days` (default 0) days later, carrying `$late`, the days late (0 on time); it SHALL otherwise be dropped with an info log naming its day. A queue rebuilt after a suspend longer than the horizon SHALL still reach a date its `late-days` lets fire. A watch starting SHALL fire no date whose firing is before its start, and a change read late SHALL bring a date its late rule still lets fire.

#### Scenario: Asleep over nine
- **GIVEN** a running watch holding a birthday firing at 09:00, the machine asleep from 08:00 to 11:00
- **WHEN** it wakes at 11:00
- **THEN** the hook fires, `$late` 0

#### Scenario: Asleep over the whole day
- **GIVEN** a running watch holding a birthday firing on 27 November at 09:00, the hook's `late-days = 2`, the machine asleep from 26 November 22:00 to 28 November 08:00
- **WHEN** it wakes on 28 November at 08:00
- **THEN** the hook fires, `$late` 1

#### Scenario: Asleep past the allowance
- **GIVEN** the same watch with `late-days = 0`
- **WHEN** it wakes on 28 November at 08:00
- **THEN** the hook does not fire, and an info line names the date

### Requirement: Cards are read only when a birthday hook wants them
A CardDAV watch SHALL read card bodies only when its account configures `on-birthday-trigger`. It SHALL then read, once the baseline is taken, the cards holding a `BDAY` or an `ANNIVERSARY` through one `addressbook-query` (RFC 6352 §8.6) whose two prop-filters any one matching does; a server refusing the filter SHALL be asked for every card instead. A server truncating the query (a 507 row) SHALL have the cards of the watch's picture the query did not name read through `addressbook-multiget` (RFC 6352 §8.7), 100 at a time. After that it SHALL read the cards a poll reports added or changed through `addressbook-multiget`, a card keyed by the id io-webdav's `id_from_location` reads from its href, as a calendar member is, and named by its href as served. A body that is not a vCard SHALL be logged and skipped; a report that fails SHALL end the session, the next one reading every date again.

#### Scenario: An account with no birthday hook
- **GIVEN** a CardDAV account configuring `on-card-change` alone
- **WHEN** a contact is edited
- **THEN** the hook fires, and no vCard is read

#### Scenario: A server truncating the date query
- **GIVEN** a watch with a birthday hook, on a server answering an `addressbook-query` with two cards and a 507 row, and an addressbook of three cards stating a date
- **WHEN** the session opens
- **THEN** the third card is read by `addressbook-multiget`, and its date fires as the other two do

## MODIFIED Requirements

### Requirement: A watch streams what it observes
`watch` SHALL print one event line per thing it observes while it runs, each an object tagged `event` and naming its `account`: `session-opened` once a session is established, connected, authenticated and its collections read, right before it first waits for a change (`backend`, `method` `idle`, `poll` or `push`, and its `collections`, each `{name, value}`), `session-lost` when one ends or cannot open (`code`, `error`, `sources`), a session failing before it is established printing `session-lost` alone and a reopened one printing `session-opened` again once established, `changed` for every change a backend reports, whether a hook is configured for it or not (`collection` and `change`, the change as its hook receives it, tagged `kind`: `item-added`, `item-removed`, `item-changed`, `flag-added`, `flag-removed`, with its `domain`, `id` and, for a flag, `flag`), `hook-failed` when a reaction of a hook fails (`hook`, the hook's name such as `on-message-add`, `on-alarm-trigger` or `on-birthday-trigger`, `id`, the item it fired for, `reaction`, `code`, `error`, `sources`), `alarm-fired` when an alarm reaches its trigger (`collection`, `id`, `uid`, `kind`, `summary`, `action`, `trigger`, an RFC 3339 instant in UTC with `Z`, as every instant the family prints; the hook variables `$trigger` and `$at` keep the system zone's offset), and `birthday-fired` when a birthday or an anniversary fires (`collection`, `id`, `uid`, `null` when the card has none, `kind`, `birthday` or `anniversary`, `name`, `null` when the card has no `FN`, `date` as `$date` prints it, `age`, an integer or `null`, `late`, the days late, and `fired`, the instant it was due, in UTC with `Z` as `alarm-fired`'s `trigger`), its text ``[<account>] addressbook `<value>`: <kind> of <name> on <date> at <fired>``, followed by `, <n> days late` when late (`, 1 day late` for one). The account threads SHALL hand their events to the command, which alone prints them, so no two lines interleave. Once a shutdown is requested, it SHALL stop every account, print exactly one result line, `{"stoppedBy": "signal"}` (text: `Watch stopped by a signal`), and exit 0. A text event SHALL be one line whatever its strings hold: what a server, a store or the configuration hands it (an account, a collection, an id, a flag, a summary, an error and its causes) SHALL pass through pimalaya-cli's `sanitize::text`, its control and bidi characters printing as U+FFFD, as the family's text listings print them. Its printer SHALL stream: text events reach stdout even when it is a pipe, and nothing else SHALL reach it, a hook command's output going to stderr. A stdout no one reads any more SHALL stop the watch as a signal does, and its failure SHALL be the command's.

#### Scenario: A watch piped to a status bar
- **GIVEN** a Maildir account whose `on-message-add` command exits 3
- **WHEN** `carillon --json watch` runs piped, a message arrives, then SIGINT is sent
- **THEN** the pipe receives `session-opened`, `changed` (`item-added`), `hook-failed` (`command`), then `{"stoppedBy":"signal"}`, and the exit status is 0

#### Scenario: A session that cannot open
- **GIVEN** a Maildir account whose mailbox the Maildir does not hold
- **WHEN** `carillon --json watch` runs, then SIGINT is sent past the first backoff
- **THEN** stdout receives `session-lost` events coded `mailbox-not-found` and the result, never `session-opened`, and the exit status is 0

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval. A sync token the server rejects SHALL cause a re-enumeration read against the picture, reporting only what differs. A server refusing the report SHALL be listed with a `PROPFIND` on every poll instead, read against the picture the same way. A re-enumeration or a listing that is truncated, or that holds a member the server answered with an error status (a 403, a 5xx), SHALL report no removal, and the error SHALL be logged as a warning; such a member in a report SHALL read as neither changed nor removed. A round that fails SHALL be run again on a fresh connection, picture and token kept, before the session is given up. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account with no `on-birthday-trigger`, watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-change` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, only what differs from the picture is reported, and the watch continues from the fresh token

#### Scenario: A server with no sync-collection
- **GIVEN** a CalDAV server answering `sync-collection` with `501 Not Implemented`
- **WHEN** an event is added, another edited and a third deleted between two polls
- **THEN** the listing that poll makes fires `on-event-add`, `on-event-change` and `on-event-remove` for them

#### Scenario: An idle connection closed between two polls
- **GIVEN** a DAV watch whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again from the same token, and the session is given up only if that also fails

### Requirement: An event is named after its domain
A hook SHALL be named `on-<subject>-<verb>`, after what it carries and what happened to it, the verb bare (`add`, `remove`, `change`, `trigger`), as Comodoro names its hooks. Mail SHALL be `on-message-add` and `on-message-remove`, whichever of IMAP, JMAP and Maildir reports it, and `on-message-change` for a Microsoft Graph draft edited where it stands, which no other mail backend accepts. A CardDAV addressbook SHALL be `on-card-add`, `on-card-remove` and `on-card-change`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. An alarm of an event or a task SHALL be `on-alarm-trigger`, named after itself as a flag is, whichever item holds it, and a birthday or an anniversary of a card `on-birthday-trigger`, one hook whose `$kind` tells the two apart. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hooks.on-event-add`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-add`

#### Scenario: A message edit hook on JMAP
- **GIVEN** an account configuring `jmap.hooks.on-message-change`
- **WHEN** the configuration is read
- **THEN** it is refused, a JMAP email being immutable

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the collection it watches, required, under the name its domain uses: `imap.mailbox`, `maildir.mailbox`, `gmail.mailbox`, `caldav.calendar`, `gcal.calendar`, `carddav.addressbook` and `gpeople.addressbook`, and for the backends serving several domains `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`, the same three under `msgraph`, and `pimdir.calendar` and `pimdir.addressbook`, of which at least one. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the name the collection its event is about was configured under, `$id` and `$collection` being the two variables every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hooks.on-message-add` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hooks.on-event-add` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

#### Scenario: A hook naming another domain's word
- **GIVEN** an account whose `jmap.hooks.on-card-add` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a card is configured and templated as `$addressbook`

### Requirement: Alarms are held in memory
A watch SHALL resolve the firings of the calendar it watches over a window ending a horizon after now (48 hours), and SHALL build it again when a member changes and when half the horizon has passed. Nothing SHALL be written between runs: a watch starting SHALL fire nothing whose trigger is before its start, and SHALL never fire one firing twice. A firing SHALL be told apart by the `UID` of the component holding its alarm, the override, the alarm's index, its repetition, its occurrence and its trigger, never by the backend's id of the item holding it, so an item read again under another id (a pimdir store synced anew, a CalDAV member moved to another href) fires nothing it already fired. A floating time SHALL resolve in the system's time zone, else as if on UTC, and zones SHALL come from the database bundled in the binary. Each account with an alarm or a birthday hook SHALL wait for its next firing, an alarm or a date alike, on one thread of its own, living as long as the account rather than its session, so the firings a lost session read keep firing on time until the next session replaces them with what it reads. The thread SHALL wait on a ceichas timer, the wall clock counting a suspend and following a clock change, and SHALL not wake before its next firing, a move of its firings or a shutdown, which wakes every alarm thread once the shutdown is requested. The watch SHALL keep its own interval.

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

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch a calendar, an addressbook or both of one pimdir store of pimdir storage-v1.0.0 as io-pimdir 1 reads it (the one neverest writes), given by `pimdir.root`, `pimdir.calendar` and `pimdir.addressbook` (the collection ids a listing of the store gives), of which at least one, and optionally `pimdir.account`, the account of the store the collections are grouped under (pimdir STORAGE §9.2), unset reading a collection whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL open the store again on every poll, every `pimdir.watch.poll.interval` (30 seconds by default), and read its change cursor (pimdir STORAGE §4.5): the items of a watched collection stamped above it are read again and the deleted or retained ones forgotten; a store of another id or incarnation, a purge count moved or a watched collection itself moved SHALL have the collections listed again, only the items new to the watch or holding another body read, the others kept and the missing ones forgotten. A store moved aside and synced anew under `pimdir.root` is so read on the next poll, and a store that poll cannot open SHALL end the session with its failure, the firings already read firing on until a session reads the store again. An item SHALL be named by its public id (pimdir STORAGE §9.1), an event's object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger`, for the calendar, and `on-birthday-trigger`, for the addressbook, and no other hook; a block naming neither collection, a collection without its hook and a hook without its collection SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `account check` coded `calendar-not-found` for `pimdir.calendar` and `addressbook-not-found` for `pimdir.addressbook`, naming it; a calendar holding no calendars (`text/calendar`), or an addressbook no contacts (`text/vcard`), SHALL fail them naming it. A store of a later version whose `read_version` is 1 SHALL be read as any other (pimdir STORAGE §4.2). A store any draft wrote, or one of a later version whose `read_version` is above 1, SHALL fail them coded `store-version`, in the wording every pimdir program shows (io-pimdir's `PimdirFormatRefusal`) naming carillon: for a store a draft wrote, which is never migrated, that its sync engine drains it until nothing is pending and a new store is synced once it is moved aside; for a later one, that carillon is to be upgraded.

#### Scenario: An addressbook beside a calendar
- **GIVEN** a pimdir account naming `pimdir.calendar`, `pimdir.addressbook`, `on-alarm-trigger` and `on-birthday-trigger`
- **WHEN** the store's owner moves the `BDAY` of a card a day later, and the watch polls
- **THEN** the card fires on its new date and not on its old one, and the alarms of the calendar fire as before, from the one watch

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

#### Scenario: A store a draft wrote
- **GIVEN** a `pimdir.root` holding a store of storage-draft-17, beside a second account watching a store of storage-v1
- **WHEN** `carillon --json watch` runs
- **THEN** the first account prints `session-lost` coded `store-version`, its error naming carillon and storage-v1 and saying to move the store aside and sync a new one, retried on its backoff, and the second account's alarms fire on time

#### Scenario: A later store a version 1 reader reads
- **GIVEN** a `pimdir.root` holding a store of storage-v2 whose `read_version` is 1
- **WHEN** `carillon --json account check` runs
- **THEN** the pimdir row reads `ok: true`

#### Scenario: A purge elsewhere in the store
- **GIVEN** a pimdir watch over a calendar of 10,000 events
- **WHEN** the store's owner purges a retained message of another collection, and the watch polls
- **THEN** the calendar is listed again and no event body is read

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in an `account check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli; nothing ran |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 whose body names no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV body naming `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon sends no request twice |
| `mailbox-not-found` | the mailbox a backend watches does not exist: an IMAP `NONEXISTENT` (RFC 5530 §3), a Maildir folder that is no directory, a JMAP, Gmail or Graph mailbox no listing names, Graph answering its direct read 404 or 400 `ErrorInvalidIdMalformed` |
| `calendar-not-found` | the calendar a backend watches does not exist: a DAV 404 on it, a DAV calendar id naming no collection, a JMAP, Google Calendar or Graph calendar not found as a mailbox is, a pimdir calendar the store does not hold or holds under another account than `pimdir.account` |
| `addressbook-not-found` | the addressbook a backend watches does not exist: a DAV 404 on it, a DAV addressbook id naming no collection, a JMAP addressbook, Google People contact group or Graph contact folder not found as a mailbox is, a pimdir addressbook the store does not hold or holds under another account than `pimdir.account` |
| `store-version` | the pimdir store is one carillon does not read: a draft's, or a later version's whose `read_version` is above 1, its message naming carillon and which of the two is the earlier; neverest syncs a new store once the old one is moved aside |
| `store-uncreated` | the pimdir root holds no store yet; its sync engine creates it |
| `store-foreign` | the pimdir root holds a database another program wrote |
| `store-corrupt` | the pimdir store's version records disagree |
| `store-not-current` | the pimdir store holds no schema yet |
| `store-hash-algo` | the pimdir store names its bodies by a hash carillon does not know |
| `store-exposed` | another user can reach the pimdir store, or a link stands where its files belong |
| `sync-version` | another sync engine version wrote the pimdir store's sync state |
| `store-busy`, `store-owned`, `store-staging`, `store-in-flight`, `store-reset`, `lock-replaced`, `item-duplicate`, `flag-invalid`, `action-malformed`, `conflict-unread`, `unsupported`, `no-performer`, `ambiguous-performer`, `source-empty`, `index-missing`, `index-outdated`, `index-version`, `index-zone-unknown`, `index-fts5-missing`, `query-refused`, `query-unanswerable`, `tag-refused`, `property-invalid` | the pimdir failure io-pimdir names so, relayed as himalaya relays it, so every pimdir app reads one vocabulary; a reader taking no lock and writing nothing meets none of them today |
| `account-missing` | no account carries the name `-a` gave, `default` included |
| `account-default-missing` | `-a` named no account and none is marked default |
| `account-default-several` | `-a` named no account and several are marked default |
| `backend-not-built` | a command reached a backend block the build leaves out |
| `capability-missing` | `-b` named a backend the account has no block for, as himalaya codes its `--backend` |

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's `session-lost` event SHALL carry the code of the failure that ended the session, `null` when it carries none.

#### Scenario: A store a draft wrote, checked
- **GIVEN** an account whose `pimdir.root` holds a store of a pimdir draft
- **WHEN** `carillon --json account check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, its error naming carillon, and the command exits 3

## REMOVED Requirements

None.
