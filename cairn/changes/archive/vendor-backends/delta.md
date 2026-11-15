---
cairn: delta
change: vendor-backends
---

## ADDED Requirements

### Requirement: Microsoft Graph and Google accounts are watched through their own APIs
carillon SHALL offer four bearer-authenticated backends, each polled, none offering another method. `msgraph` SHALL watch mail, contacts and calendar events under `msgraph.mailbox`, `msgraph.addressbook` and `msgraph.calendar`, at least one given, sharing one token and one connection. `gmail` SHALL watch a label under `gmail.mailbox`, `gcal` a calendar under `gcal.calendar`, and `gpeople` a contact group under `gpeople.addressbook`. Each SHALL fire the hooks of the domains it holds and declare no other, flags being reported under the shared names (`Seen`, `Flagged`). An expired change feed SHALL NOT report what the gap hides as removals: where the collection can be listed again it SHALL be, and only what differs from the known picture reported; Gmail, whose history cannot be listed again, SHALL resume from the current cursor. The wizard SHALL offer these APIs first for a Google or Microsoft address, beside the IMAP and DAV services discovered for it, the token collected through the shared picker.

#### Scenario: A follow-up flag in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Inbox"` with `msgraph.hook.on-flag-added.flags = ["Flagged"]`
- **WHEN** a message in the inbox is flagged for follow-up
- **THEN** the hook fires with `$flag` set to `Flagged`

#### Scenario: A far-future event
- **GIVEN** an account watching `msgraph.calendar`
- **WHEN** an event two years ahead is edited
- **THEN** `msgraph.hook.on-event-changed` fires, no window hiding it

#### Scenario: An expired Google sync token
- **GIVEN** a `gcal` account whose sync token the server answers `410 Gone`
- **WHEN** the next poll runs
- **THEN** the calendar is re-read and only what differs from the known picture is reported

#### Scenario: A Google address in the wizard
- **GIVEN** a `gmail.com` address typed into the wizard
- **WHEN** discovery completes
- **THEN** the Gmail, Google Calendar and Google People APIs are listed first, the Gmail API being the default

## MODIFIED Requirements

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the collection it watches, required, under the name its domain uses: `imap.mailbox`, `maildir.mailbox`, `gmail.mailbox`, `caldav.calendar`, `gcal.calendar`, `carddav.addressbook` and `gpeople.addressbook`, and for the backends serving several domains `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`, and the same three under `msgraph`, of which at least one. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the name the collection its event is about was configured under, `$id` being the one variable every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hook.on-message-added` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hook.on-event-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

#### Scenario: A hook naming another domain's word
- **GIVEN** an account whose `jmap.hook.on-card-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a card is configured and templated as `$addressbook`

## REMOVED Requirements
