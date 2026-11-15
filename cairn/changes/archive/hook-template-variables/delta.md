---
cairn: delta
change: hook-template-variables
---

## ADDED Requirements

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read. `id` and `collection` SHALL be available to every hook, `flag` to a flag hook, and the envelope names only to the arrival hook of a backend that resolves one, which is IMAP alone. A `${name:default}` SHALL keep working whatever the name, a default being how a template says it can do without the value. A command SHALL NOT be validated, its placeholders reaching it as environment variables where an unset name is ordinary.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hook.on-message-removed` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hook.on-message-added` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

## MODIFIED Requirements

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification, a shell command, or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A hook that fails SHALL be logged and left behind: neither a missing notification daemon nor a broken script SHALL end the watch. A hook SHALL NOT half-fire for a reason the configuration could have been refused for: what a template may name is settled when the file is read, so the only failures left at watch time are the ones the machine around it produced.

#### Scenario: The hook script exits non-zero
- **GIVEN** an account whose `cmd` hook exits with an error
- **WHEN** it fires
- **THEN** the failure is logged and the watch keeps running

## REMOVED Requirements
