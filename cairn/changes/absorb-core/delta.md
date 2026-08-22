---
cairn: delta
change: absorb-core
---

## ADDED Requirements

### Requirement: The watch is I/O-free, the frontend drives it
The watch SHALL be a coroutine that performs no I/O: on each resume it returns what its driver must do next (read, write, surface a changed state, or stop). It SHALL NOT own the transport, reconnect, credential resolution, or a clock or random source, and SHALL NOT depend on an async runtime, a TLS stack or a notification library. Response parsing SHALL stay in imap-codec, reached through io-imap's coroutines, so the hostile-input surface belongs to the crates that fuzz it; what this daemon owns is the parser bound it chooses and the phase machine around the reads.

#### Scenario: The watcher survives a second frontend
- **GIVEN** the watch coroutine, driven by the daemon over a blocking socket
- **WHEN** a server frontend is revived and drives it over an async socket
- **THEN** it needs a new driver only, and no second watcher to keep in step

#### Scenario: A server announces a literal larger than the bound
- **GIVEN** a watch whose server announces a literal past the 1 MiB parser bound
- **WHEN** the driver pumps those bytes
- **THEN** the watch ends with an error rather than buffering them or stalling

### Requirement: The ring is content-free and self-addressed
The ring SHALL say only that a watched target changed: the account, the source, the mailbox, and the opaque state token the coroutine surfaces. It SHALL NOT carry a UID, a change kind, or any message content, and the daemon SHALL expose no changes or diff API, because only a consumer knows what it needs from a ring: a new-mail count, the changed senders, or nothing but a re-render.

#### Scenario: A consumer wants to know what changed
- **GIVEN** a ring saying a mailbox changed
- **WHEN** the consumer needs the sender or the subject
- **THEN** it goes and looks with its own credentials and its own client, and the daemon neither resolves nor caches that for it

### Requirement: The watch is IDLE plus EXAMINE, not a delta watcher
Folded in from the retired core spec, where it was already current truth. The watch SHALL keep no shadow of the mailbox and SHALL ring on an opaque state token only, which is what keeps a watch cheap enough for a server to hold thousands of them.

#### Scenario: A flag change on a CONDSTORE server
- **GIVEN** a watch on a CONDSTORE server, using the `UIDVALIDITY:HIGHESTMODSEQ` token
- **WHEN** a message flag changes but no new mail arrives
- **THEN** IDLE wakes, the re-EXAMINE finds HIGHESTMODSEQ advanced, and the daemon rings

## MODIFIED Requirements

### Requirement: The frontend owns transport and reconnect
The daemon SHALL own the transport: it opens the TLS connection (it trusts the user's own config, so it applies no SSRF guard) and hands the watch the stream. It SHALL own the reconnect loop, resolving a fresh credential and opening a fresh connection on each attempt, with capped exponential backoff and jitter. The watch coroutine SHALL only run one session over the stream it is handed.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the one-session watch returns because the connection dropped
- **THEN** the daemon waits a jittered backoff and reconnects, resolving the credential again

### Requirement: The notify consumer is enriched locally, the cmd consumer is not
When a watch has a notify hook, the daemon SHALL resolve the change locally before notifying: on its own IMAP connection, never the watched one, it SHALL EXAMINE the mailbox and `UID FETCH` the ENVELOPE of the newly-arrived messages, and SHALL fire one desktop notification per new message, exposing the message subject, from name, from address, and date to the notification templates. The daemon SHALL NOT enrich the cmd consumer, which keeps receiving the raw content-free event, since its role (kicking a sync tool) re-derives everything itself. Enrichment SHALL run only when a notify hook is configured, and SHALL fire no notification when a change carries no new mail.

#### Scenario: New mail pops a meaningful notification
- **GIVEN** a watch with a notify hook
- **WHEN** a new message arrives
- **THEN** the daemon opens its own connection, fetches the new message's envelope, and shows a notification naming the sender and subject
- **AND** the raw ring, still content-free, is what a cmd hook would have seen

#### Scenario: A cmd-only watch pays nothing extra
- **GIVEN** a watch with a cmd hook and no notify hook
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the `CARILLON_*` environment and opens no extra connection, resolving nothing

## REMOVED Requirements

### Requirement: The daemon hosts only outbound sources
Removed: the transport class existed so a frontend could refuse a public-callback source such as Gmail push, which only a server with a public endpoint can host. With no server frontend, the daemon watches IMAP and nothing else, so there is no class to advertise and nothing to refuse.
