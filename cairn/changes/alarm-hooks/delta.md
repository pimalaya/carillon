---
cairn: delta
change: alarm-hooks
---

## ADDED Requirements

Times in scenarios are UTC unless a zone is named.

### Requirement: A calendar fires its alarms
The `caldav` and `pimdir` backends SHALL offer `on-alarm-trigger`, firing once per firing of every `VALARM` of the events and tasks of the watched calendar, at its trigger, as ical-rs resolves it (`IcalAlarms`), repetitions included. A firing SHALL be dropped when the component holding its alarm is `STATUS:CANCELLED`, when that component is a `VTODO` completed (`STATUS:COMPLETED` or a `COMPLETED` property), when its alarm's `ACKNOWLEDGED` (RFC 9074 6) is at or after its trigger, and when its `ACTION` is neither `DISPLAY` nor `AUDIO`. No other backend SHALL accept the hook.

#### Scenario: A firing at its trigger
- **GIVEN** a CalDAV account with `caldav.hook.on-alarm-trigger.notify.summary = "$summary"`, watching a calendar holding an event "Standup" at 09:00 with a `-PT15M` display alarm
- **WHEN** the clock reaches 08:45
- **THEN** one notification titled "Standup" is shown

#### Scenario: A cancelled event
- **GIVEN** an event with `STATUS:CANCELLED` holding a `-PT15M` alarm
- **WHEN** its trigger is reached
- **THEN** the hook does not fire

#### Scenario: An email alarm
- **GIVEN** an event holding an `ACTION:EMAIL` alarm
- **WHEN** its trigger is reached
- **THEN** the hook does not fire

#### Scenario: An alarm hook on an addressbook
- **GIVEN** an account configuring `carddav.hook.on-alarm-trigger`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hooks a CardDAV account has

### Requirement: Alarms are held in memory
A watch SHALL resolve the firings of the calendar it watches over a window ending a horizon after now (48 hours), and SHALL build it again when a member changes and when half the horizon has passed. Nothing SHALL be written between runs: a watch starting SHALL fire nothing whose trigger is before its start, and SHALL never fire one firing twice. A floating time SHALL resolve in the system's time zone, else as if on UTC, and zones SHALL come from the database bundled in the binary. Each account with an alarm hook SHALL wait for its next firing on a thread of its own, living as long as the account rather than its session, so the alarms a lost session read keep firing on time until the next session replaces them with what it reads. The thread SHALL be woken when its firings move and when a shutdown is requested, the watch keeping its own interval. The wait SHALL be measured on the wall clock, read again at least every 200 ms, so neither a suspend nor a clock change delays a firing by more than that.

#### Scenario: A restart does not repeat
- **GIVEN** a watch that fired an alarm at 08:45 and was stopped at 08:46
- **WHEN** it starts again at 08:50
- **THEN** that alarm does not fire again

#### Scenario: Beyond the horizon
- **GIVEN** an event starting in three days with a `-PT15M` alarm
- **WHEN** the watch starts
- **THEN** the firing is not queued, and it is once the horizon reaches it

### Requirement: A late firing fires within the grace window only
A firing reached late, the machine having slept or a change having come late, SHALL fire when it is at most the grace window late (15 minutes), and SHALL otherwise be dropped with an info log.

#### Scenario: A short suspend
- **GIVEN** a running watch holding an alarm firing at 08:45, the machine asleep from 08:40 to 08:55
- **WHEN** it wakes at 08:55
- **THEN** the hook fires for that alarm

#### Scenario: A night's suspend
- **GIVEN** a running watch holding an alarm firing at 02:00, the machine asleep from 23:00 to 07:00
- **WHEN** it wakes at 07:00
- **THEN** the hook does not fire for that alarm, and an info line names it

### Requirement: An edited calendar moves its alarms
A member the watch reports added or changed SHALL have its body read again and its firings replaced, and a member reported removed SHALL have its firings forgotten, before the next firing is waited for. Every member SHALL stay in the watch's picture whether it holds an alarm or not, so a member gaining one is seen.

#### Scenario: An edited event
- **GIVEN** a watch holding an event whose alarm fires at 08:45
- **WHEN** the event is moved an hour later on the server, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

#### Scenario: A deleted event
- **GIVEN** a watch holding an event with an alarm
- **WHEN** the event is deleted on the server, and the watch polls
- **THEN** its alarm does not fire

#### Scenario: An event gaining an alarm
- **GIVEN** a watch holding an event with no alarm
- **WHEN** a `-PT10M` alarm is added to it on the server, and the watch polls
- **THEN** the alarm fires ten minutes before the event

### Requirement: Bodies are read only when an alarm hook wants them
A CalDAV watch SHALL read member bodies only when its account configures `on-alarm-trigger`. It SHALL then read, once the baseline is taken, the members holding a `VALARM` through one `calendar-query` (RFC 4791 7.8) per component, a `VEVENT` and a `VTODO`, the comp-filters of one query all having to match (RFC 4791 9.7.1); a server refusing the filter SHALL be asked for every member instead. After that it SHALL read the members a poll reports added or changed through `calendar-multiget` (RFC 4791 7.9), keyed by resource name. A body that is not iCalendar SHALL be logged and skipped; a report that fails SHALL end the session, the next one reading every alarm again.

#### Scenario: An account with no alarm hook
- **GIVEN** a CalDAV account configuring `on-event-changed` alone
- **WHEN** an event is edited
- **THEN** the hook fires, and no event body is read

### Requirement: An alarm hook templates against its firing
`on-alarm-trigger` SHALL fill `$id`, `$collection` and `$calendar`, `$etag`, `$uid`, `$kind` (`event` or `task`), `$summary`, `$recurrenceId`, `$alarm` (the index of the `VALARM` among those of its component), `$alarmUid`, `$action` (`display` or `audio`), `$trigger` and `$at` (the start of the occurrence for an event, the due time for a task). Times SHALL be RFC 3339 with the system zone's offset. A variable the firing has no value for SHALL expand to nothing. The notification of an `AUDIO` alarm SHALL ask for the freedesktop sound `alarm-clock-elapsed`.

#### Scenario: A command given the firing
- **GIVEN** `caldav.hook.on-alarm-trigger.cmd = 'echo "$uid $trigger" >> fired.log'`
- **WHEN** an alarm of an event whose `UID` is `standup` fires at 08:45 in Europe/Paris
- **THEN** fired.log ends with `standup 2026-11-05T08:45:00+01:00`

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store, given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL poll the store's change cursor (pimdir STORAGE §4.5) every `pimdir.watch.poll.interval` (30 seconds by default), read again the items stamped above it and forget the deleted or retained ones, and read the whole collection again when the store was reset, a purge count moved or the collection itself moved. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or one holding no calendars, SHALL fail the watch and `check` naming it.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. The account block SHALL keep the shape himalaya CLI and himalaya TUI read, and unknown keys SHALL be ignored there rather than refused, so an account can be recognised across the binaries. A whole file SHALL NOT be claimed to load in all three, since every backend block is strict on both sides and each has keys the other does not know.

### Requirement: An event is named after its domain
A hook SHALL be named after what it carries. Mail SHALL be `on-message-added` and `on-message-removed`, whichever of IMAP, JMAP and Maildir reports it, and `on-message-changed` for a Microsoft Graph draft edited where it stands, which no other mail backend accepts. A CardDAV addressbook SHALL be `on-card-added`, `on-card-removed` and `on-card-changed`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. An alarm of an event or a task SHALL be `on-alarm-trigger`, named after itself as a flag is, whichever item holds it. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.
