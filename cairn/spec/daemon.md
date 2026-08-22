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

### Requirement: The frontend owns transport and reconnect
The daemon SHALL own the transport: it opens the TLS connection (it trusts the user's own config, so it applies no SSRF guard) and hands core the stream. It SHALL own the reconnect loop, resolving a fresh credential and opening a fresh connection on each attempt, with capped exponential backoff and jitter. carillon-core SHALL only run one session over the stream it is handed.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** core's one-session watch returns because the connection dropped
- **THEN** the daemon waits a jittered backoff and reconnects, resolving the credential again

### Requirement: Credentials resolve locally, never inside core
The daemon SHALL resolve a watch's credential itself and hand core a ready secret. It SHALL support a cleartext password (discouraged) and a password_command whose first stdout line is the password, for a keyring read. OAuth is out of scope for the first version.

#### Scenario: A keyring-backed password
- **GIVEN** a watch with a password_command such as `pass show mail/me`
- **WHEN** the daemon connects
- **THEN** it runs the command, takes the first stdout line as the password, and hands core a password credential

### Requirement: Two built-in local consumers
The daemon SHALL ship two consumers: notify (a content-free desktop notification naming the target and account) and exec (a shell command receiving the ring's fields as CARILLON_ACCOUNT, CARILLON_SOURCE, CARILLON_TARGET, and CARILLON_STATE). A consumer failure SHALL be logged, not propagated, so one consumer never stalls the others. Neither consumer SHALL emit message content, since the ring carries none.

#### Scenario: An exec hook reacts to a ring
- **GIVEN** a watch with an exec command
- **WHEN** the mailbox changes
- **THEN** the daemon runs the command with the ring's fields in the environment, and the script may re-fetch content itself

### Requirement: The daemon hosts only outbound sources
The daemon SHALL host only outbound transport classes (standing-connection now, poll later). It SHALL NOT host public-callback sources such as Gmail push, which need a public endpoint the daemon does not have. Those remain the server's.
