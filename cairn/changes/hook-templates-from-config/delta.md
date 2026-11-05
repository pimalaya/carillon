---
cairn: delta
change: hook-templates-from-config
---

## ADDED Requirements

### Requirement: Notifications are shown when the build can show them
A `notify` cargo feature, on by default, SHALL decide whether hooks can show notifications. A build without it SHALL refuse a configuration carrying a `notify` table, naming the feature, and its wizard SHALL generate hooks without one.

#### Scenario: A headless build
- **GIVEN** a build without the `notify` feature
- **WHEN** a configuration carrying `imap.hook.on-message-added.notify.summary` is read
- **THEN** it is refused, naming the `notify` cargo feature

## MODIFIED Requirements

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read, a default or not. Notifications SHALL be templated in the shell's syntax (`$name`, `${name}`, `${name:-default}`, `$$`), any other `${...}` form being refused when read, and variables SHALL be camelCase. `$id` SHALL be available to every hook, the collection SHALL be available under the name its backend configures it as and under `$collection` whatever the backend, `$flag` to a flag hook, and the envelope names (`$subject`, `$date`, `$sender`, `$senderName`, `$senderAddress`, `$recipient`, `$recipientName`, `$recipientAddress`) only to the arrival hook of a backend that resolves one. A `${name:-default}` SHALL take its default when the value is empty. A command SHALL NOT be validated, its placeholders reaching it as environment variables of the same names, where an unset name is ordinary.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hook.on-message-removed` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hook.on-message-added` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

#### Scenario: A template in a former spelling
- **GIVEN** an account whose `jmap.hook.on-message-added` notification summary reads `$sender_name` or `${subject:none}`
- **WHEN** the configuration is read
- **THEN** it is refused, naming `sender_name` or `${subject:none}`

#### Scenario: One command for every domain
- **GIVEN** a `caldav` account on `work` and a `carddav` one on `default`, both running the same command
- **WHEN** an event and a contact are added
- **THEN** the command reads `collection=work` for the first and `collection=default` for the second, from the same variable

## REMOVED Requirements
