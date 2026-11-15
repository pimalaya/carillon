---
cairn: delta
change: watch-under-backend
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: The watch method belongs to the backend
The method SHALL be configured under its backend (`imap.watch`, `jmap.watch`, `maildir.watch`, `dav.watch`) and named by its mechanism, the way a SASL mechanism and an HTTP auth scheme already are. Each backend SHALL declare only the methods it has, so a method it does not have is refused when the configuration is read rather than when the watch runs. Unset, an account SHALL watch the best way its backend has: IDLE for IMAP, a held event stream for JMAP, a poll for the backends with nothing else. Every backend SHALL offer the poll, whose interval MAY be given and otherwise takes what suits that backend.

#### Scenario: A server whose IDLE cannot be trusted
- **GIVEN** an IMAP account whose server accepts IDLE and then never speaks
- **WHEN** the account configures `imap.watch.poll.interval`
- **THEN** the watch re-reads the mailbox on that interval instead, reporting the same events

#### Scenario: A method the backend does not have
- **GIVEN** a Maildir account configuring `maildir.watch.idle`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the methods that backend has, and no watch is started

## REMOVED Requirements
