---
cairn: delta
change: jmap-domains
---

## ADDED Requirements

### Requirement: A JMAP account watches every domain it configures
The JMAP backend SHALL watch mail, contacts and calendar events, each under the collection key its domain uses (`jmap.mailbox`, `jmap.addressbook`, `jmap.calendar`), of which at least one SHALL be given. It SHALL fire `on-message-*` and `on-flag-*` for mail, `on-card-*` for contacts and `on-event-*` for calendar events, and SHALL NOT offer `on-task-*`, the JMAP calendars draft having no task type. A hook naming a domain the account configured no collection for SHALL be refused when the configuration is read, naming the hook and the key it would need. All the domains SHALL share one session, one connection and one event stream, that being what JMAP offers over a protocol needing an account per domain.

#### Scenario: One account, three domains
- **GIVEN** a JMAP account configuring `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`
- **WHEN** a contact is edited
- **THEN** `jmap.hook.on-card-changed` fires, over the same connection the mail watch holds

#### Scenario: A hook for a domain the account does not watch
- **GIVEN** a JMAP account configuring `jmap.mailbox` and `jmap.hook.on-card-added`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the `jmap.addressbook` it would need

## MODIFIED Requirements

### Requirement: An account watches one collection, one way
An account SHALL watch the collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection of the same domain is a second account, which is also how it gets its own hooks. A backend serving several domains MAY name one collection per domain, since the domains do not share an event name and each therefore already has hooks of its own; what they share is the connection and the credential, which is what a second account would waste.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the collection it watches, required, under the name its domain uses: `imap.mailbox`, `maildir.mailbox`, `caldav.calendar` and `carddav.addressbook`, and `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar` of which at least one. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the name the collection its event is about was configured under, `$id` being the one variable every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hook.on-message-added` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another domain's word
- **GIVEN** an account whose `jmap.hook.on-card-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a card is configured and templated as `$addressbook`

## REMOVED Requirements
