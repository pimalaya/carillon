---
cairn: delta
change: graph-draft-edits
---

## MODIFIED Requirements

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, a flag added, a flag removed, each carrying the domain of what it is about. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit but a Microsoft Graph draft, edited where it stands, and then only where no flag moved in the same edit, its `changeKey` moving with either, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The vocabulary SHALL stay one across the backends even though the hooks configuring it are per backend and per domain, so that one hook runner serves all of them.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flag-added` with the flag named `Seen`

#### Scenario: An item that is edited where it stands
- **GIVEN** a CardDAV account watching an addressbook
- **WHEN** a contact is edited and its etag moves
- **THEN** `carddav.hook.on-card-changed` fires, an event only a Microsoft Graph mailbox also accepts a hook for among the mail backends

### Requirement: An event is named after its domain
A hook SHALL be named after what it carries. Mail SHALL be `on-message-added` and `on-message-removed`, whichever of IMAP, JMAP and Maildir reports it, and `on-message-changed` for a Microsoft Graph draft edited where it stands, which no other mail backend accepts. A CardDAV addressbook SHALL be `on-card-added`, `on-card-removed` and `on-card-changed`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hook.on-event-added`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-added`

#### Scenario: A message edit hook on JMAP
- **GIVEN** an account configuring `jmap.hook.on-message-changed`
- **WHEN** the configuration is read
- **THEN** it is refused, a JMAP email being immutable

### Requirement: Microsoft Graph and Google accounts are watched through their own APIs
carillon SHALL offer four bearer-authenticated backends, each polled, none offering another method. `msgraph` SHALL watch mail, contacts and calendar events under `msgraph.mailbox`, `msgraph.addressbook` and `msgraph.calendar`, at least one given, sharing one token and one connection; `contacts` SHALL name the default contact folder, and a folder or calendar SHALL be found by id wherever it sits, or by name. `gmail` SHALL watch a label under `gmail.mailbox`, `gcal` a calendar under `gcal.calendar`, by id or name, and `gpeople` a contact group under `gpeople.addressbook`, `contacts` naming every contact the account owns rather than a group. Each SHALL fire the hooks of the domains it holds and declare no other, flags being reported under the shared names (`Seen`, `Flagged`). An expired change feed SHALL NOT report what the gap hides as removals: where the collection can be listed again it SHALL be, and only what differs from the known picture reported; Gmail, whose history cannot be listed again, SHALL resume from the current cursor. The wizard SHALL offer these APIs first for a Google or Microsoft address, beside the IMAP and DAV services discovered for it, the token collected through the shared picker.

#### Scenario: A follow-up flag in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Inbox"` with `msgraph.hook.on-flag-added.flags = ["Flagged"]`
- **WHEN** a message in the inbox is flagged for follow-up
- **THEN** the hook fires with `$flag` set to `Flagged`

#### Scenario: A draft edited in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Drafts"` with `msgraph.hook.on-message-changed`
- **WHEN** a draft's subject, body or recipients are edited
- **THEN** the hook fires with the draft's `$id`, which the handler reads back itself

#### Scenario: A far-future event
- **GIVEN** an account watching `msgraph.calendar`
- **WHEN** an event two years ahead is edited
- **THEN** `msgraph.hook.on-event-changed` fires, no window hiding it

#### Scenario: An expired Google sync token
- **GIVEN** a `gcal` account whose sync token the server answers `410 Gone`
- **WHEN** the next poll runs
- **THEN** the calendar is re-read and only what differs from the known picture is reported

#### Scenario: Every Google contact
- **GIVEN** a `gpeople` account watching `contacts`
- **WHEN** a contact belonging to no group is added
- **THEN** `gpeople.hook.on-card-added` fires

#### Scenario: A Google address in the wizard
- **GIVEN** a `gmail.com` address typed into the wizard
- **WHEN** discovery completes
- **THEN** the Gmail, Google Calendar and Google People APIs are listed first, the Gmail API being the default

