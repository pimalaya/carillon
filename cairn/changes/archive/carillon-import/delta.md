---
cairn: delta
change: carillon-import
---

## ADDED Requirements

### Requirement: Every configured account, or a chosen one
Bare `mirador watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account. Each account's mailbox SHALL come from its own config; `-m/--mailbox` overrides it and SHALL be refused when more than one account is watched.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `mirador watch` runs
- **THEN** both accounts are watched at once, each on its configured mailbox, and Ctrl+C stops them together

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL reopen a session that ended for any reason other than a requested shutdown, after a capped exponential backoff that a healthy session resets. Credentials SHALL be resolved per attempt rather than held.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

### Requirement: One change vocabulary across backends
Every backend SHALL report the same four events, with flags named the same way whatever the backend spells them as.

#### Scenario: A message is marked read on each backend
- **GIVEN** three accounts watching over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flags-added` with the flag named `Seen`

### Requirement: Arrivals are resolved only when a hook wants them
The daemon SHALL fetch an arrival's envelope only when an `on-message-added` hook is configured, on a second connection, and SHALL degrade to an unresolved event when that fails.

#### Scenario: A cmd-only account with no message hook
- **GIVEN** an account whose only hook is `on-flags-added`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`), an optional watched mailbox, and its hooks. The schema SHALL stay compatible with himalaya CLI and himalaya TUI, and unknown keys SHALL be ignored rather than refused.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

## REMOVED Requirements

### Requirement: One watch surface across backends via io-email
Removed with io-email. The shared `EmailClientStd::watch_mailbox` was the reason a frozen aggregator sat under this tool, pinning every protocol crate to an old generation. Each backend now speaks to its own crate, and the sharing happens at the event vocabulary instead.
