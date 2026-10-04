---
cairn: delta
change: dav-listed-collections
---

## MODIFIED Requirements

### Requirement: An account watches one collection, one way
An account SHALL watch the collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection of the same domain is a second account, which is also how it gets its own hooks. A backend serving several domains MAY name one collection per domain, since the domains do not share an event name and each therefore already has hooks of its own; what they share is the connection and the credential, which is what a second account would waste. Every backend SHALL name its collection by the id a listing of the account gives it, so a collection picked from a listing is watched without translation. A DAV collection named by one path segment SHALL be looked up under the calendar or addressbook home set the server's principal names, read under `server` when the server names none; an absolute path SHALL be taken as it stands, and any other relative path read under `server`.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

#### Scenario: A calendar named by its listed id
- **GIVEN** a CalDAV server whose principal names the home set `/dav/cal/alice/`, holding `/dav/cal/alice/default/`
- **WHEN** an account sets `caldav.calendar = "default"`
- **THEN** the watch reads `/dav/cal/alice/default`, whatever path `server` names

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval. A sync token the server rejects SHALL cause a re-enumeration read against the picture, reporting only what differs. A server refusing the report SHALL be listed with a `PROPFIND` on every poll instead, read against the picture the same way, and a truncated listing SHALL report no removal. A round that fails SHALL be run again on a fresh connection, picture and token kept, before the session is given up. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-changed` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, only what differs from the picture is reported, and the watch continues from the fresh token

#### Scenario: A server with no sync-collection
- **GIVEN** a CalDAV server answering `sync-collection` with `501 Not Implemented`
- **WHEN** an event is added, another edited and a third deleted between two polls
- **THEN** the listing that poll makes fires `on-event-added`, `on-event-changed` and `on-event-removed` for them

#### Scenario: An idle connection closed between two polls
- **GIVEN** a DAV watch whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again from the same token, and the session is given up only if that also fails

### Requirement: A CalDAV calendar knows its components
A CalDAV watch SHALL resolve what its collection holds from `supported-calendar-component-set` when the watch starts. A calendar advertising a single component SHALL report every member as that component, at no cost per member. A calendar advertising several SHALL read `getcontenttype` on a member it has not seen and route it by the `component` parameter RFC 4791 §10.1 allows; a member whose content type names no component SHALL be taken for the one domain the account's hooks name, or for an event when they name both. A member SHALL never be fetched to find out what it is, since a poll carrying a VEVENT is what asking for etags alone exists to avoid; reading a property is not fetching a member. The domain of each member SHALL be remembered beside its etag, since a removal leaves only an href behind.

#### Scenario: A task is deleted from a calendar holding both
- **GIVEN** a CalDAV account watching a calendar of events and tasks, with `on-task-removed` configured
- **WHEN** a VTODO is deleted
- **THEN** `on-task-removed` fires, the domain coming from what the watch remembered of that href, and `on-event-removed` does not

#### Scenario: A calendar that holds one component
- **GIVEN** a CalDAV account watching a calendar advertising `VEVENT` alone
- **WHEN** a member is added
- **THEN** `on-event-added` fires without any further request, and the account's task hooks are refused when the configuration is read

#### Scenario: A server naming no component
- **GIVEN** a calendar advertising `VEVENT` and `VTODO` whose members' content type is a bare `text/calendar`, and an account hooking `on-event-*` alone
- **WHEN** an event is added
- **THEN** `on-event-added` fires
