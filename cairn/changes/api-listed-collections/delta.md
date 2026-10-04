---
cairn: delta
change: api-listed-collections
---

## MODIFIED Requirements

### Requirement: Microsoft Graph and Google accounts are watched through their own APIs
carillon SHALL offer four bearer-authenticated backends, each polled, none offering another method. `msgraph` SHALL watch mail, contacts and calendar events under `msgraph.mailbox`, `msgraph.addressbook` and `msgraph.calendar`, at least one given, sharing one token and one connection; `contacts` SHALL name the default contact folder, and a folder or calendar SHALL be found by id wherever it sits, or by name. `gmail` SHALL watch a label under `gmail.mailbox`, `gcal` a calendar under `gcal.calendar`, by id or name, and `gpeople` a contact group under `gpeople.addressbook`, `contacts` naming every contact the account owns rather than a group. Each SHALL fire the hooks of the domains it holds and declare no other, flags being reported under the shared names (`Seen`, `Flagged`). An expired change feed SHALL NOT report what the gap hides as removals: where the collection can be listed again it SHALL be, and only what differs from the known picture reported; Gmail, whose history cannot be listed again, SHALL resume from the current cursor. The wizard SHALL offer these APIs first for a Google or Microsoft address, beside the IMAP and DAV services discovered for it, the token collected through the shared picker.

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

#### Scenario: Every Google contact
- **GIVEN** a `gpeople` account watching `contacts`
- **WHEN** a contact belonging to no group is added
- **THEN** `gpeople.hook.on-card-added` fires

#### Scenario: A Google address in the wizard
- **GIVEN** a `gmail.com` address typed into the wizard
- **WHEN** discovery completes
- **THEN** the Gmail, Google Calendar and Google People APIs are listed first, the Gmail API being the default
