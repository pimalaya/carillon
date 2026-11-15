---
cairn: delta
change: one-collection-one-method
---

## ADDED Requirements

### Requirement: An account watches one collection, one way
An account SHALL name the one collection it watches, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection is a second account, which is also how it gets its own hooks. Every backend SHALL read the collection the same way, the DAV one included, whose `server` names the DAV root and whose collection is the path under it.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

### Requirement: A backend refuses a method it does not have
The watch method SHALL be named by its mechanism (`watch.idle`, `watch.push`, `watch.poll`), the way a SASL mechanism and an HTTP auth scheme already are. Unset, an account SHALL watch the best way its backend has: IDLE for IMAP, a held event stream for JMAP, a poll for the backends with nothing else. Every backend SHALL offer the poll, whose interval MAY be given and otherwise takes what suits that backend. A backend asked for a method it cannot honour SHALL refuse to start, naming what it offers, rather than quietly using another one.

#### Scenario: A server whose IDLE cannot be trusted
- **GIVEN** an IMAP account whose server accepts IDLE and then never speaks
- **WHEN** the account configures `watch.poll.interval`
- **THEN** the watch re-reads the mailbox on that interval instead, reporting the same events

#### Scenario: A method the backend does not have
- **GIVEN** a Maildir account configuring `watch.idle`
- **WHEN** the watch starts
- **THEN** it fails, saying the maildir backend offers poll, rather than polling anyway

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `dav`), the collection it watches, and the hooks it fires. The config schema SHALL stay compatible with himalaya CLI and himalaya TUI, so one file can back every binary, and unknown keys SHALL be ignored rather than refused.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `on-item-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: Every configured account, or a chosen one
Bare `mirador watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account, and an unknown name SHALL be an error. One account's watch failure SHALL be logged and retried on its own without stalling the others.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `mirador watch` runs
- **THEN** both accounts are watched at once, each on its own collection, and Ctrl+C stops them together

## REMOVED Requirements
