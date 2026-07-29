---
cairn: spec
capability: daemon
status: current
---

# The carillon CLI daemon

carillon is the lightweight frontend of Carillon: a self-hostable daemon that watches PIM accounts and fires local consumers on each change. It hosts carillon-core, the same watch loop the server runs, without the network-and-trust apparatus (no HTTP listener, datastore, auth, custody, metering, or billing). It is the top of the funnel: a free local watcher that makes "let us host the watch for you" the natural next sentence.

The daemon reads a TOML config of named IMAP watches. Per watch, a supervisor task owns the transport and reconnect and drives core's one-session watch over the opened stream; every content-free ring is routed to that watch's consumers. See [[../../../core/cairn/spec/watch-client]] for the shared watcher this frontend hosts.

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
The daemon SHALL own the transport: it opens the TLS connection (it trusts the user's own config, so it applies no SSRF guard) and hands core the stream. It SHALL own the reconnect loop, resolving a fresh credential and opening a fresh connection on each attempt, with capped exponential backoff and jitter. carillon-core SHALL only run one session over the stream it is handed.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** core's one-session watch returns because the connection dropped
- **THEN** the daemon waits a jittered backoff and reconnects, resolving the credential again

### Requirement: Credentials resolve locally, never inside core
The daemon SHALL resolve a watch's credential itself and hand core a ready secret. It SHALL accept a password (used with `LOGIN`) or a pre-minted token (used with SASL `OAUTHBEARER`), each a config secret resolved from a raw value or a command (a keyring read), fresh on each connect. OAuth minting and refresh are out of scope for the first version.

#### Scenario: A keyring-backed password
- **GIVEN** a watch whose `imap.password` is a command such as `pass show mail/me`
- **WHEN** the daemon connects
- **THEN** it runs the command to resolve the secret and hands core a password credential

### Requirement: Two built-in local consumers
The daemon SHALL ship two consumers: notify (a desktop notification, enriched locally with the new message's subject, from name, from address, and date, one per new message) and exec (a shell command receiving the raw ring's fields as CARILLON_ACCOUNT, CARILLON_MAILBOX, CARILLON_SOURCE, and CARILLON_STATE). A consumer failure SHALL be logged, not propagated, so one consumer never stalls the others. The exec consumer SHALL receive no message content; it re-fetches itself if it needs any.

#### Scenario: An exec hook reacts to a ring
- **GIVEN** a watch with an exec command
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the ring's fields in the environment, and the script may re-fetch content itself

### Requirement: The notify consumer is enriched locally, the cmd consumer is not
When a watch has a notify hook, the daemon SHALL resolve the change locally before notifying: on its own IMAP connection (the consumer's own protocol client, never carillon-core), it SHALL EXAMINE the mailbox and `UID FETCH` the ENVELOPE of the newly-arrived messages, and SHALL fire one desktop notification per new message, exposing the message subject, from name, from address, and date to the notification templates. The daemon SHALL NOT enrich the cmd consumer, which keeps receiving the raw content-free event, since its role (kicking a sync tool) re-derives everything itself. Enrichment SHALL run only when a notify hook is configured, and SHALL fire no notification when a change carries no new mail.

#### Scenario: New mail pops a meaningful notification
- **GIVEN** a watch with a notify hook
- **WHEN** a new message arrives
- **THEN** the daemon opens its own connection, fetches the new message's envelope, and shows a notification naming the sender and subject
- **AND** the raw ring, still content-free, is what a cmd hook would have seen

#### Scenario: A cmd-only watch pays nothing extra
- **GIVEN** a watch with a cmd hook and no notify hook
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the `CARILLON_*` environment and opens no extra connection, resolving nothing

### Requirement: The daemon hosts only outbound sources
The daemon SHALL host only outbound transport classes (standing-connection now, poll later). It SHALL NOT host public-callback sources such as Gmail push, which need a public endpoint the daemon does not have. Those remain the server's.
