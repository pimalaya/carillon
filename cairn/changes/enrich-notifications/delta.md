---
cairn: delta
id: enrich-notifications
---

## ADDED Requirements

### Requirement: The notify consumer is enriched locally, the cmd consumer is not
When a watch has a notify hook, the daemon SHALL resolve the change locally
before notifying: on its own IMAP connection (the consumer's own protocol
client, never carillon-core), it SHALL EXAMINE the mailbox and `UID FETCH` the
ENVELOPE of the newly-arrived messages, and SHALL fire one desktop
notification per new message, exposing the message subject, from name, from
address, and date to the notification templates. The daemon SHALL NOT enrich
the cmd consumer, which keeps receiving the raw content-free event, since its
role (kicking a sync tool) re-derives everything itself. Enrichment SHALL run
only when a notify hook is configured, and SHALL fire no notification when a
change carries no new mail.

#### Scenario: New mail pops a meaningful notification
- **GIVEN** a watch with a notify hook
- **WHEN** a new message arrives
- **THEN** the daemon opens its own connection, fetches the new message's
  envelope, and shows a notification naming the sender and subject
- **AND** the raw ring, still content-free, is what a cmd hook would have seen

#### Scenario: A cmd-only watch pays nothing extra
- **GIVEN** a watch with a cmd hook and no notify hook
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the `CARILLON_*` environment and
  opens no extra connection, resolving nothing

## MODIFIED Requirements

### Requirement: Two built-in local consumers
The daemon SHALL ship two consumers: notify (a desktop notification, enriched
locally with the new message's subject, from name, from address, and date, one
per new message) and exec (a shell command receiving the raw ring's fields as
CARILLON_ACCOUNT, CARILLON_MAILBOX, CARILLON_SOURCE, and CARILLON_STATE). A
consumer failure SHALL be logged, not propagated, so one consumer never stalls
the others. The exec consumer SHALL receive no message content; it re-fetches
itself if it needs any.

#### Scenario: An exec hook reacts to a ring
- **GIVEN** a watch with an exec command
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the ring's fields in the
  environment, and the script may re-fetch content itself
