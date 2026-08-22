---
cairn: spec
capability: daemon
status: current
---

# The watch daemon

mirador watches PIM accounts and fires local hooks on every change. It reads a TOML config of named accounts, watches each one on its own thread, and runs on one machine with no server apparatus: no HTTP listener, datastore, auth, custody, metering or billing.

It watches; it never syncs. A change is reported as it is seen (a message arrived, one left, flags moved) and nothing is stored between runs. What a hook wants beyond that, mirador goes and reads on demand.

Each backend brings its own way of learning about a change, and the daemon translates all of them into one vocabulary, so a hook is written once: IMAP holds IDLE and reports UID-keyed deltas, JMAP polls `Email/changes`, Maildir re-lists the mailbox. The protocol crates own the protocols (io-imap, io-jmap, io-maildir); this repository owns the config, the hooks and the supervision.

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`), an optional watched mailbox, and the hooks it fires. The config schema SHALL stay compatible with himalaya CLI and himalaya TUI, so one file can back every binary, and unknown keys SHALL be ignored rather than refused.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: Every configured account, or a chosen one
Bare `mirador watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account, and an unknown name SHALL be an error. Each account's mailbox SHALL come from its own config, so accounts watching different mailboxes need no flag; `-m/--mailbox` overrides it and SHALL be refused when more than one account is watched, since it could only mean one of them.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `mirador watch` runs
- **THEN** both accounts are watched at once, each on its configured mailbox, and Ctrl+C stops them together

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is logged and retried for that account alone, and the other account keeps watching

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

### Requirement: One change vocabulary across backends
Every backend SHALL report changes as the same four events: a message added, a message removed, flags added, flags removed. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a hook filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike.

#### Scenario: A message is marked read on each backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flags-added` with the flag named `Seen`

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that a message arrived, not what it says. The daemon SHALL resolve an arrival into its envelope (subject, sender, recipient, date) only when the account configures an `on-message-added` hook, and SHALL do so on a second connection, never the one holding the watch. A resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: A cmd-only account with no message hook
- **GIVEN** an account whose only hook is `on-flags-added`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

### Requirement: Ctrl+C is prompt on every path
A requested shutdown SHALL be honoured within roughly a second on every path a watch can be waiting in: idling on a connection, sleeping between polls, backing off before a reconnect, or resolving an arrival's envelope. No path SHALL wait on a server that has stopped answering: every connection the daemon opens SHALL carry a read deadline and SHALL hand back the not-ready failures rather than letting the transport retry them away, since the deadline exists to be the wakeup that re-reads the flag.

#### Scenario: Ctrl+C while resolving against a silent server
- **GIVEN** a watch resolving an arrival's envelope against a server that has stopped answering
- **WHEN** the user presses Ctrl+C
- **THEN** the read deadline expires, the flag is seen, and the watch ends rather than waiting for the transport's own timeout

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification, a shell command, or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A hook that fails SHALL be logged and left behind: neither a missing notification daemon nor a broken script SHALL end the watch.

#### Scenario: The hook script exits non-zero
- **GIVEN** an account whose `cmd` hook exits with an error
- **WHEN** it fires
- **THEN** the failure is logged and the watch keeps running

### Requirement: The account can be checked before it is watched
`mirador check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `mirador check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits non-zero
