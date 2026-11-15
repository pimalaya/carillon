---
cairn: delta
change: jmap-repair
---

## ADDED Requirements

### Requirement: A watch survives the change it reports
A watch SHALL keep working across the changes it reports. A connection a backend's own protocol closes as part of reporting SHALL be reopened before the next request rather than written into, and a round that fails SHALL be retried once on a fresh connection before the session is given up. A round SHALL advance no state it did not complete, so running it twice reports nothing twice.

#### Scenario: A JMAP event stream that closes after its state change
- **GIVEN** a JMAP account watching over the event stream, which the server closes after reporting one state change
- **WHEN** a message arrives
- **THEN** the round that follows runs on a fresh connection and fires the hook, rather than losing the session and re-baselining the change away

#### Scenario: A polling watch whose idle connection was closed
- **GIVEN** a JMAP account polling on an interval, whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again, and the session is given up only if that also fails

## MODIFIED Requirements

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, and sometimes what it says. A backend SHALL report an arrival together with the summary it already read, and SHALL read one it does not have only when the active backend configures the arrival hook of that domain. JMAP SHALL take its summary from the `Email/get` its round already makes, asking for the envelope properties only when a hook wants them. IMAP SHALL read one on a second connection, never the one holding the watch, an IMAP delta naming a UID and nothing more. A backend that can read no envelope SHALL leave the summary empty, and a resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no arrival hook
- **GIVEN** an account whose only hook is `imap.hook.on-flag-added`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

#### Scenario: A JMAP arrival with an envelope
- **GIVEN** a JMAP account whose `jmap.hook.on-message-added` notification names `$subject` and `$sender`
- **WHEN** a message arrives
- **THEN** both are filled from the round's own `Email/get`, with no second request

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval, and a sync token the server rejects SHALL cause a re-enumeration, which reports nothing because a re-baseline is not news. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-changed` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, no event is fired for what was already there, and the watch continues from the fresh token

### Requirement: An event is named after its domain
A hook SHALL be named after what it carries. Mail SHALL be `on-message-added` and `on-message-removed`, whichever of IMAP, JMAP and Maildir reports it. A CardDAV addressbook SHALL be `on-card-added`, `on-card-removed` and `on-card-changed`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hook.on-event-added`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-added`

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the one collection it watches, required, under the name its domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar` and `carddav.addressbook`. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the same name its backend configures, `$id` being the one variable every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hook.on-message-added` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hook.on-event-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

## REMOVED Requirements

### Requirement: The untyped DAV backend
**Reason**: It was the whole WebDAV backend before CalDAV and CardDAV were named out of it, and what remained was a collection that is neither, whose members were called items because nothing better could be said. The two domains a PIM notifier watches now have a backend each, so nothing is left for the generic one to serve.
