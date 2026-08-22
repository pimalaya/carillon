---
cairn: spec
capability: daemon
status: current
---

# The carillon CLI daemon

carillon is the lightweight frontend of Carillon: a self-hostable daemon that watches PIM accounts and fires local consumers on each change. It owns the watch loop outright, without the network-and-trust apparatus (no HTTP listener, datastore, auth, custody, metering, or billing). It is the top of the funnel: a free local watcher that makes "let us host the watch for you" the natural next sentence.

The daemon reads a TOML config of named IMAP watches. Per watch, a thread owns the transport and reconnect and drives the one-session watch coroutine over the opened stream; every content-free ring is routed to that watch's consumers.

It signals; it never syncs. The ring says a mailbox changed and nothing else, so the daemon holds no replica, computes no per-message delta, and keeps only the opaque state token that tells one change from the last. A consumer that needs to know what changed goes and looks with its own credentials.

### Requirement: A watch runs from a TOML file with no server apparatus
The daemon SHALL read its watches from a TOML config file, resolved from an explicit path, then the XDG path, the home path, and a local carillon.toml in turn. Each watch SHALL carry the IMAP host, port, login, mailbox, a credential, and the consumers it fires. The daemon SHALL run a watch end to end on one machine with no HTTP listener, datastore, auth, custody, metering, or billing.

#### Scenario: Local watch from a config file
- **GIVEN** a carillon config describing one IMAP watch with the notify consumer
- **WHEN** the daemon runs and the mailbox changes
- **THEN** a desktop notification fires, with no network delivery and no Carillon account involved

### Requirement: The daemon watches every configured account, or a chosen subset
Bare `carillon watch` SHALL watch every account in the config concurrently, one independent watch per account under a single shared shutdown. Account names given as positional arguments SHALL narrow the watch to exactly those accounts, and the daemon SHALL fail when a named account is absent from the config. One account's watch failure SHALL be logged and recovered on its own without stalling the others. Each account's watched mailbox SHALL come from its own config (there is no mailbox flag), since accounts watch different mailboxes.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no positional argument
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its configured mailbox, and Ctrl+C stops them together

#### Scenario: Watch a chosen subset
- **GIVEN** a config with three accounts
- **WHEN** `carillon watch work personal` runs
- **THEN** only those two accounts are watched, and an unknown name is an error

### Requirement: The frontend owns transport and reconnect
The daemon SHALL own the transport: it opens the TLS connection (it trusts the user's own config, so it applies no SSRF guard) and hands the watch the stream. It SHALL open that stream with no retry strategy, taking back the not-ready failures a stream would otherwise retry away, since it arms a read deadline precisely to be woken up and poll its shutdown flag between IDLE keep-alives. It SHALL own the reconnect loop, resolving a fresh credential and opening a fresh connection on each attempt, with capped exponential backoff and jitter. The watch coroutine SHALL only run one session over the stream it is handed.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the one-session watch returns because the connection dropped
- **THEN** the daemon waits a jittered backoff and reconnects, resolving the credential again

#### Scenario: Ctrl+C during a quiet IDLE
- **GIVEN** a watch idling against a server that has nothing to say
- **WHEN** the user presses Ctrl+C
- **THEN** the next read deadline expires as a failure the daemon sees, and the watch winds down rather than waiting for the server

### Requirement: Credentials resolve in the frontend, never inside the watch
The daemon SHALL resolve a watch's credential itself and hand the watch a ready secret. It SHALL accept a password (used with `LOGIN`) or a pre-minted token (used with SASL `OAUTHBEARER`), each a config secret resolved from a raw value or a command (a keyring read), fresh on each connect. The watch coroutine SHALL never unlock a keyring, run a command, or mint or refresh OAuth. OAuth minting and refresh are out of scope for the first version.

#### Scenario: A keyring-backed password
- **GIVEN** a watch whose `imap.password` is a command such as `pass show mail/me`
- **WHEN** the daemon connects
- **THEN** it runs the command to resolve the secret and hands the watch a password credential

### Requirement: Two built-in local consumers
The daemon SHALL ship two consumers: notify (a desktop notification, enriched locally with the new message's subject, from name, from address, and date, one per new message) and exec (a shell command receiving the raw ring's fields as CARILLON_ACCOUNT, CARILLON_MAILBOX, CARILLON_SOURCE, and CARILLON_STATE). A consumer failure SHALL be logged, not propagated, so one consumer never stalls the others. The exec consumer SHALL receive no message content; it re-fetches itself if it needs any.

#### Scenario: An exec hook reacts to a ring
- **GIVEN** a watch with an exec command
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the ring's fields in the environment, and the script may re-fetch content itself

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
Because the ring is content-free, the watch SHALL NOT compute per-message deltas and SHALL keep no shadow of the mailbox. It SHALL require only IDLE: greet, authenticate, read the mailbox state by a read-only EXAMINE, then hold IDLE and, on each wake, re-EXAMINE and ring only when the state token advanced. On a CONDSTORE server the token SHALL be `UIDVALIDITY:HIGHESTMODSEQ`, read by an `EXAMINE (CONDSTORE)` and advancing on any change (new mail, flags, deletes); otherwise it SHALL be `UIDVALIDITY:UIDNEXT`, advancing on new mail only. QRESYNC SHALL NOT be required.

#### Scenario: A flag change on a CONDSTORE server
- **GIVEN** a watch on a CONDSTORE server, using the `UIDVALIDITY:HIGHESTMODSEQ` token
- **WHEN** a message flag changes but no new mail arrives
- **THEN** IDLE wakes, the re-EXAMINE finds HIGHESTMODSEQ advanced, and the daemon rings

#### Scenario: A flag change without CONDSTORE
- **GIVEN** a watch on a server without CONDSTORE, using the `UIDVALIDITY:UIDNEXT` token
- **WHEN** a message flag changes but no new mail arrives
- **THEN** IDLE wakes, the re-EXAMINE finds the token unchanged, and the daemon does not ring (new mail only, the honest limit of a CONDSTORE-less server)
