---
cairn: delta
change: hook-under-backend
---

## ADDED Requirements

### Requirement: The hooks belong to the backend
The hooks SHALL be configured under their backend (`imap.hook`, `jmap.hook`, `maildir.hook`, `caldav.hook`, `carddav.hook`, `dav.hook`), singular, with `hooks` accepted as an alias. Each backend SHALL declare only the events it reports, so a hook it cannot fire is refused when the configuration is read rather than never firing. The variables a hook templates against SHALL be the ones its backend can fill, which is why the envelope names belong to the IMAP table alone. An account declaring more than one backend SHALL configure the hooks of each, since a hook written for one backend says nothing about what another would report.

#### Scenario: A hook the backend cannot fire
- **GIVEN** an account configuring `carddav.hook.on-flag-added`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the events CardDAV reports, and no watch is started

#### Scenario: Two backends on one account
- **GIVEN** an account declaring both `imap` and `maildir`, with an `imap.hook.on-message-added` summary reading `New mail from $sender`
- **WHEN** the account is watched with `-b maildir`
- **THEN** no hook fires from the IMAP table, and the Maildir table is what the watch reads

### Requirement: An event is named after its domain
A hook SHALL be named after what it carries. Mail SHALL be `on-message-added` and `on-message-removed`, whichever of IMAP, JMAP and Maildir reports it. A CardDAV addressbook SHALL be `on-card-*`, a CalDAV calendar `on-event-*` and `on-task-*`, and a DAV collection with no domain to name SHALL keep `on-item-*`. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hook.on-event-added`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-added`

### Requirement: A flag hook fires once per flag
`on-flag-added` and `on-flag-removed` SHALL fire once for each flag that moved rather than once for the delta, so `$flag` SHALL always name the flag the firing is about and no plural variable SHALL be exposed. The optional `flags = [...]` filter SHALL narrow which of those firings happen, matching one flag at a time, with or without a leading `\` or `$` and without regard to case. A backend with no flags SHALL take no flag hook at all.

#### Scenario: Two flags set at once
- **GIVEN** an IMAP account with an unfiltered `imap.hook.on-flag-added` command
- **WHEN** one STORE sets both `\Seen` and `\Flagged`
- **THEN** the command runs twice, once with `$flag` as `Seen` and once as `Flagged`

#### Scenario: A filtered flag hook
- **GIVEN** the same account filtering `flags = ["Seen"]`
- **WHEN** the same STORE sets both flags
- **THEN** the command runs once, with `$flag` as `Seen`

### Requirement: A CalDAV calendar knows its components
A CalDAV watch SHALL resolve what its collection holds from `supported-calendar-component-set` when the watch starts. A calendar advertising a single component SHALL report every member as that component, at no cost per member. A calendar advertising several SHALL read `getcontenttype` on a member it has not seen and route it by the `component` parameter RFC 4791 §10.1 allows, falling back to the components the calendar advertises when the server sends no parameter. A member SHALL never be fetched to find out what it is, since a poll carrying a VEVENT is what asking for etags alone exists to avoid; reading a property is not fetching a member. The domain of each member SHALL be remembered beside its etag, since a removal leaves only an href behind.

#### Scenario: A task is deleted from a calendar holding both
- **GIVEN** a CalDAV account watching a calendar of events and tasks, with `on-task-removed` configured
- **WHEN** a VTODO is deleted
- **THEN** `on-task-removed` fires, the domain coming from what the watch remembered of that href, and `on-event-removed` does not

#### Scenario: A calendar that holds one component
- **GIVEN** a CalDAV account watching a calendar advertising `VEVENT` alone
- **WHEN** a member is added
- **THEN** `on-event-added` fires without any further request, and the account's task hooks are refused when the configuration is read
## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `dav`) and the collection it watches, and each backend block SHALL carry the hooks that backend fires. The config schema SHALL stay compatible with himalaya CLI and himalaya TUI, so one file can back every binary, and unknown keys SHALL be ignored rather than refused.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.hook.on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, a flag added, a flag removed, each carrying the domain of what it is about. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The vocabulary SHALL stay one across the backends even though the hooks configuring it are per backend and per domain, so that one hook runner serves all of them.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flag-added` with the flag named `Seen`

#### Scenario: An item that is edited where it stands
- **GIVEN** a CardDAV account watching an addressbook
- **WHEN** a contact is edited and its etag moves
- **THEN** `carddav.hook.on-card-changed` fires, an event no mail backend accepts a hook for

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav`, `carddav` and `dav` names the domain it holds, all three sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval, and a sync token the server rejects SHALL cause a re-enumeration, which reports nothing because a re-baseline is not news.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-changed` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, no event is fired for what was already there, and the watch continues from the fresh token

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, not what it says. The daemon SHALL resolve an arrival into its summary (for mail: subject, sender, recipient, date) only when the active backend configures the arrival hook of that domain, and SHALL do so on a second connection, never the one holding the watch. Only a backend able to read one resolves anything; the others neither resolve nor offer the envelope variables. A resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no arrival hook
- **GIVEN** an account whose only hook is `imap.hook.on-flag-added`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

## REMOVED Requirements
