---
cairn: delta
change: collection-under-backend
---

## ADDED Requirements

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the one collection it watches, required, under the name its domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar`, `carddav.addressbook` and `dav.collection`, the last being generic because a plain DAV collection has no domain to name. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the same name its backend configures, `$id` being the one variable every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hook.on-message-added` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hook.on-event-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `dav`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. The account block SHALL keep the shape himalaya CLI and himalaya TUI read, and unknown keys SHALL be ignored there rather than refused, so an account can be recognised across the binaries. A whole file SHALL NOT be claimed to load in all three, since every backend block is strict on both sides and each has keys the other does not know.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hook.on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: An account watches one collection, one way
An account SHALL watch the one collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection is a second account, which is also how it gets its own hooks. Every backend SHALL read its collection the same way, the DAV ones included, whose `server` names the DAV root and whose collection is the path under it.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read. `$id` SHALL be available to every hook, the collection SHALL be available under the name its backend configures it as, `$flag` to a flag hook, and the envelope names only to the arrival hook of a backend that resolves one, which is IMAP alone. A `${name:default}` SHALL keep working whatever the name, a default being how a template says it can do without the value. A command SHALL NOT be validated, its placeholders reaching it as environment variables where an unset name is ordinary.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hook.on-message-removed` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hook.on-message-added` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

## REMOVED Requirements
