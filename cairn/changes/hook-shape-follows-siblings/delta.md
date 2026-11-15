---
cairn: delta
change: hook-shape-follows-siblings
---

# Delta

Every requirement is in cairn/spec/daemon.md. Each one naming a hook, a `hook` table or `cmd` moves to `hooks`, `command` and the bare-verb names; the ones below also change in substance: A watch runs from a TOML file (the former spellings refused), A hook failure never stops the watch (`command`), The hooks belong to the backend (`hooks`, every backend listed, `dav` gone, the envelope sentence corrected) and An event is named after its domain (the naming rule).

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI; a backend the build leaves out is such a key. A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hooks`), and the `dav` backend (`caldav` or `carddav`). A hook spelling of before 1.0 SHALL be refused the same way, naming what it became and MIGRATION.md: a backend's `hook` table (`hooks`), a hook's `cmd` (`command`), and every hook name ending in `-added`, `-removed` or `-changed` (`-add`, `-remove`, `-change`).

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hooks.on-message-add` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

#### Scenario: A misspelt account key
- **GIVEN** an account holding `defualt = true`
- **WHEN** the configuration is read
- **THEN** it is refused naming `defualt` and its line, and no watch is started

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-add.command`
- **WHEN** the configuration is read
- **THEN** it is refused naming `<backend>.hooks` and MIGRATION.md

#### Scenario: A hook named as in 0.2
- **GIVEN** an account holding `imap.hook.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused naming `hooks` and MIGRATION.md, and once that is fixed `on-message-add`, then `command`

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal. A session reopened after its connection was lost SHALL report what moved meanwhile where its backend can tell: IMAP SHALL resume from the UIDs and flags the lost watch knew, and Gmail from the history cursor the lost session had reached, both held in memory for as long as the account is watched. An IMAP session that ended on a refusal rather than a lost connection, a mailbox recreated under the same name among them, SHALL take a new baseline, as every session of the other backends does.

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

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, a flag added, a flag removed, each carrying the domain of what it is about. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit but a Microsoft Graph draft, edited where it stands, and then only where no flag moved in the same edit, its `changeKey` moving with either, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The vocabulary SHALL stay one across the backends even though the hooks configuring it are per backend and per domain, so that one hook runner serves all of them.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flag-add` with the flag named `Seen`

#### Scenario: An item that is edited where it stands
- **GIVEN** a CardDAV account watching an addressbook
- **WHEN** a contact is edited and its etag moves
- **THEN** `carddav.hooks.on-card-change` fires, an event only a Microsoft Graph mailbox also accepts a hook for among the mail backends

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval. A sync token the server rejects SHALL cause a re-enumeration read against the picture, reporting only what differs. A server refusing the report SHALL be listed with a `PROPFIND` on every poll instead, read against the picture the same way, and a truncated listing SHALL report no removal. A round that fails SHALL be run again on a fresh connection, picture and token kept, before the session is given up. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
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

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, and sometimes what it says. A backend SHALL report an arrival together with the summary it already read, and SHALL read one it does not have only when the active backend configures the arrival hook of that domain. JMAP SHALL take its summary from the `Email/get` its round already makes, asking for the envelope properties only when a hook wants them. IMAP SHALL read one on a second connection, never the one holding the watch, an IMAP delta naming a UID and nothing more. A backend that can read no envelope SHALL leave the summary empty, and a resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no arrival hook
- **GIVEN** an account whose only hook is `imap.hooks.on-flag-add`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

#### Scenario: A JMAP arrival with an envelope
- **GIVEN** a JMAP account whose `jmap.hooks.on-message-add` notification names `$subject` and `$sender`
- **WHEN** a message arrives
- **THEN** both are filled from the round's own `Email/get`, with no second request

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification (`notify`), a command (`command`, a shell line or a program and its arguments), or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A hook that fails SHALL be logged and left behind: neither a missing notification daemon nor a broken script SHALL end the watch. A hook SHALL NOT half-fire for a reason the configuration could have been refused for: what a template may name is settled when the file is read, so the only failures left at watch time are the ones the machine around it produced.

#### Scenario: The hook script exits non-zero
- **GIVEN** an account whose hook `command` exits with an error
- **WHEN** it fires
- **THEN** the failure is logged and the watch keeps running

### Requirement: The hooks belong to the backend
The hooks SHALL be configured under their backend, in its `hooks` table (`imap.hooks`, `jmap.hooks`, `maildir.hooks`, `caldav.hooks`, `carddav.hooks`, `msgraph.hooks`, `gmail.hooks`, `gcal.hooks`, `gpeople.hooks`, `pimdir.hooks`), as Comodoro configures its own. Each backend SHALL declare only the events it reports, so a hook it cannot fire is refused when the configuration is read rather than never firing. The variables a hook templates against SHALL be the ones its backend can fill, which is why the envelope names belong to the arrival hook of a backend resolving one. An account declaring more than one backend SHALL configure the hooks of each, since a hook written for one backend says nothing about what another would report.

#### Scenario: A hook the backend cannot fire
- **GIVEN** an account configuring `carddav.hooks.on-flag-add`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the events CardDAV reports, and no watch is started

#### Scenario: Two backends on one account
- **GIVEN** an account declaring both `imap` and `maildir`, with an `imap.hooks.on-message-add` summary reading `New mail from $sender`
- **WHEN** the account is watched with `-b maildir`
- **THEN** no hook fires from the IMAP table, and the Maildir table is what the watch reads

### Requirement: An event is named after its domain
A hook SHALL be named `on-<subject>-<verb>`, after what it carries and what happened to it, the verb bare (`add`, `remove`, `change`, `trigger`), as Comodoro names its hooks. Mail SHALL be `on-message-add` and `on-message-remove`, whichever of IMAP, JMAP and Maildir reports it, and `on-message-change` for a Microsoft Graph draft edited where it stands, which no other mail backend accepts. A CardDAV addressbook SHALL be `on-card-add`, `on-card-remove` and `on-card-change`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. An alarm of an event or a task SHALL be `on-alarm-trigger`, named after itself as a flag is, whichever item holds it. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hooks.on-event-add`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-add`

#### Scenario: A message edit hook on JMAP
- **GIVEN** an account configuring `jmap.hooks.on-message-change`
- **WHEN** the configuration is read
- **THEN** it is refused, a JMAP email being immutable

### Requirement: A flag hook fires once per flag
`on-flag-add` and `on-flag-remove` SHALL fire once for each flag that moved rather than once for the delta, so `$flag` SHALL always name the flag the firing is about and no plural variable SHALL be exposed. The optional `flags = [...]` filter SHALL narrow which of those firings happen, matching one flag at a time, with or without a leading `\` or `$` and without regard to case. A backend with no flags SHALL take no flag hook at all.

#### Scenario: Two flags set at once
- **GIVEN** an IMAP account with an unfiltered `imap.hooks.on-flag-add` command
- **WHEN** one STORE sets both `\Seen` and `\Flagged`
- **THEN** the command runs twice, once with `$flag` as `Seen` and once as `Flagged`

#### Scenario: A filtered flag hook
- **GIVEN** the same account filtering `flags = ["Seen"]`
- **WHEN** the same STORE sets both flags
- **THEN** the command runs once, with `$flag` as `Seen`

### Requirement: A CalDAV calendar knows its components
A CalDAV watch SHALL resolve what its collection holds from `supported-calendar-component-set` when the watch starts. A calendar advertising a single component SHALL report every member as that component, at no cost per member. A calendar advertising several SHALL read `getcontenttype` on a member it has not seen and route it by the `component` parameter RFC 4791 §10.1 allows; a member whose content type names no component SHALL be taken for the one domain the account's hooks name, or for an event when they name both. A member SHALL never be fetched to find out what it is, since a poll carrying a VEVENT is what asking for etags alone exists to avoid; reading a property is not fetching a member. The domain of each member SHALL be remembered beside its etag, since a removal leaves only an href behind.

#### Scenario: A task is deleted from a calendar holding both
- **GIVEN** a CalDAV account watching a calendar of events and tasks, with `on-task-remove` configured
- **WHEN** a VTODO is deleted
- **THEN** `on-task-remove` fires, the domain coming from what the watch remembered of that href, and `on-event-remove` does not

#### Scenario: A calendar that holds one component
- **GIVEN** a CalDAV account watching a calendar advertising `VEVENT` alone
- **WHEN** a member is added
- **THEN** `on-event-add` fires without any further request, and the account's task hooks are refused when the configuration is read

#### Scenario: A server naming no component
- **GIVEN** a calendar advertising `VEVENT` and `VTODO` whose members' content type is a bare `text/calendar`, and an account hooking `on-event-*` alone
- **WHEN** an event is added
- **THEN** `on-event-add` fires

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read, a default or not. Notifications SHALL be templated in the shell's syntax (`$name`, `${name}`, `${name:-default}`, `$$`), any other `${...}` form being refused when read, and variables SHALL be camelCase. `$id` SHALL be available to every hook, the collection SHALL be available under the name its backend configures it as and under `$collection` whatever the backend, `$flag` to a flag hook, and the envelope names (`$subject`, `$date`, `$sender`, `$senderName`, `$senderAddress`, `$recipient`, `$recipientName`, `$recipientAddress`) only to the arrival hook of a backend that resolves one. A `${name:-default}` SHALL take its default when the value is empty. A command SHALL NOT be validated, its placeholders reaching it as environment variables of the same names, where an unset name is ordinary.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hooks.on-message-remove` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hooks.on-message-add` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

#### Scenario: A template in a former spelling
- **GIVEN** an account whose `jmap.hooks.on-message-add` notification summary reads `$sender_name` or `${subject:none}`
- **WHEN** the configuration is read
- **THEN** it is refused, naming `sender_name` or `${subject:none}`

#### Scenario: One command for every domain
- **GIVEN** a `caldav` account on `work` and a `carddav` one on `default`, both running the same command
- **WHEN** an event and a contact are added
- **THEN** the command reads `collection=work` for the first and `collection=default` for the second, from the same variable

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the collection it watches, required, under the name its domain uses: `imap.mailbox`, `maildir.mailbox`, `gmail.mailbox`, `caldav.calendar`, `gcal.calendar`, `carddav.addressbook` and `gpeople.addressbook`, and for the backends serving several domains `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`, and the same three under `msgraph`, of which at least one. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the name the collection its event is about was configured under, `$id` and `$collection` being the two variables every backend means the same way.

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

### Requirement: A JMAP account watches every domain it configures
The JMAP backend SHALL watch mail, contacts and calendar events, each under the collection key its domain uses (`jmap.mailbox`, `jmap.addressbook`, `jmap.calendar`), of which at least one SHALL be given. It SHALL fire `on-message-*` and `on-flag-*` for mail, `on-card-*` for contacts and `on-event-*` for calendar events, and SHALL NOT offer `on-task-*`, the JMAP calendars draft having no task type. A hook naming a domain the account configured no collection for SHALL be refused when the configuration is read, naming the hook and the key it would need. All the domains SHALL share one session, one connection and one event stream, that being what JMAP offers over a protocol needing an account per domain.

#### Scenario: One account, three domains
- **GIVEN** a JMAP account configuring `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`
- **WHEN** a contact is edited
- **THEN** `jmap.hooks.on-card-change` fires, over the same connection the mail watch holds

#### Scenario: A hook for a domain the account does not watch
- **GIVEN** a JMAP account configuring `jmap.mailbox` and `jmap.hooks.on-card-add`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the `jmap.addressbook` it would need

### Requirement: Microsoft Graph and Google accounts are watched through their own APIs
carillon SHALL offer four bearer-authenticated backends, each polled, none offering another method. `msgraph` SHALL watch mail, contacts and calendar events under `msgraph.mailbox`, `msgraph.addressbook` and `msgraph.calendar`, at least one given, sharing one token and one connection; `contacts` SHALL name the default contact folder, and a folder or calendar SHALL be found by id wherever it sits, or by name. `gmail` SHALL watch a label under `gmail.mailbox`, `gcal` a calendar under `gcal.calendar`, by id or name, and `gpeople` a contact group under `gpeople.addressbook`, `contacts` naming every contact the account owns rather than a group. Each SHALL fire the hooks of the domains it holds and declare no other, flags being reported under the shared names (`Seen`, `Flagged`). An expired change feed SHALL NOT report what the gap hides as removals: where the collection can be listed again it SHALL be, and only what differs from the known picture reported; Gmail, whose history cannot be listed again, SHALL resume from the current cursor, as it SHALL from a history whose listing does not end.

#### Scenario: A follow-up flag in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Inbox"` with `msgraph.hooks.on-flag-add.flags = ["Flagged"]`
- **WHEN** a message in the inbox is flagged for follow-up
- **THEN** the hook fires with `$flag` set to `Flagged`

#### Scenario: A draft edited in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Drafts"` with `msgraph.hooks.on-message-change`
- **WHEN** a draft's subject, body or recipients are edited
- **THEN** the hook fires with the draft's `$id`, which the handler reads back itself

#### Scenario: A far-future event
- **GIVEN** an account watching `msgraph.calendar`
- **WHEN** an event two years ahead is edited
- **THEN** `msgraph.hooks.on-event-change` fires, no window hiding it

#### Scenario: An expired Google sync token
- **GIVEN** a `gcal` account whose sync token the server answers `410 Gone`
- **WHEN** the next poll runs
- **THEN** the calendar is re-read and only what differs from the known picture is reported

#### Scenario: Every Google contact
- **GIVEN** a `gpeople` account watching `contacts`
- **WHEN** a contact belonging to no group is added
- **THEN** `gpeople.hooks.on-card-add` fires

### Requirement: Notifications are shown when the build can show them
A `notify` cargo feature, on by default, SHALL decide whether hooks can show notifications. A build without it SHALL refuse a configuration carrying a `notify` table, naming the feature.

#### Scenario: A headless build
- **GIVEN** a build without the `notify` feature
- **WHEN** a configuration carrying `imap.hooks.on-message-add.notify.summary` is read
- **THEN** it is refused, naming the `notify` cargo feature

Times in the alarm scenarios below are UTC unless a zone is named.

### Requirement: A calendar fires its alarms
The `caldav` and `pimdir` backends SHALL offer `on-alarm-trigger`, firing once per firing of every `VALARM` of the events and tasks of the watched calendar, at its trigger, as ical-rs resolves it (`IcalAlarms`), repetitions included. A firing SHALL be dropped when the component holding its alarm is `STATUS:CANCELLED`, when that component is a `VTODO` completed (`STATUS:COMPLETED` or a `COMPLETED` property), when its alarm's `ACKNOWLEDGED` (RFC 9074 6) is at or after its trigger, and when its `ACTION` is neither `DISPLAY` nor `AUDIO`. No other backend SHALL accept the hook.

#### Scenario: A firing at its trigger
- **GIVEN** a CalDAV account with `caldav.hooks.on-alarm-trigger.notify.summary = "$summary"`, watching a calendar holding an event "Standup" at 09:00 with a `-PT15M` display alarm
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
- **GIVEN** an account configuring `carddav.hooks.on-alarm-trigger`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hooks a CardDAV account has

### Requirement: Bodies are read only when an alarm hook wants them
A CalDAV watch SHALL read member bodies only when its account configures `on-alarm-trigger`. It SHALL then read, once the baseline is taken, the members holding a `VALARM` through one `calendar-query` (RFC 4791 7.8) per component, a `VEVENT` and a `VTODO`, the comp-filters of one query all having to match (RFC 4791 9.7.1); a server refusing the filter SHALL be asked for every member instead. After that it SHALL read the members a poll reports added or changed through `calendar-multiget` (RFC 4791 7.9), keyed by resource name. A body that is not iCalendar SHALL be logged and skipped; a report that fails SHALL end the session, the next one reading every alarm again.

#### Scenario: An account with no alarm hook
- **GIVEN** a CalDAV account configuring `on-event-change` alone
- **WHEN** an event is edited
- **THEN** the hook fires, and no event body is read

### Requirement: An alarm hook templates against its firing
`on-alarm-trigger` SHALL fill `$id`, `$collection` and `$calendar`, `$etag`, `$uid`, `$kind` (`event` or `task`), `$summary`, `$recurrenceId`, `$alarm` (the index of the `VALARM` among those of its component), `$alarmUid`, `$action` (`display` or `audio`), `$trigger` and `$at` (the start of the occurrence for an event, the due time for a task). Times SHALL be RFC 3339 with the system zone's offset. A variable the firing has no value for SHALL expand to nothing. The notification of an `AUDIO` alarm SHALL ask for the freedesktop sound `alarm-clock-elapsed`.

#### Scenario: A command given the firing
- **GIVEN** `caldav.hooks.on-alarm-trigger.command = 'echo "$uid $trigger" >> fired.log'`
- **WHEN** an alarm of an event whose `UID` is `standup` fires at 08:45 in Europe/Paris
- **THEN** fired.log ends with `standup 2026-11-05T08:45:00+01:00`

