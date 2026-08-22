---
cairn: spec
capability: daemon
status: current
---

# The watch daemon

carillon watches PIM accounts and fires local hooks on every change. It reads a TOML config of named accounts, watches each one on its own thread, and runs on one machine with no server apparatus: no HTTP listener, datastore, auth, custody, metering or billing.

It watches; it never syncs. A change is reported as it is seen (an item arrived, one left, one was edited, flags moved) and nothing is stored between runs. What a hook wants beyond that, carillon goes and reads on demand.

Each backend brings its own way of learning about a change, and the daemon translates all of them into one vocabulary, so a hook is written once: IMAP holds IDLE and reports UID-keyed deltas, JMAP polls `Email/changes`, Maildir re-lists the mailbox, WebDAV reports what a collection did since a sync token. Mail is not the boundary: a WebDAV collection is a CalDAV calendar or a CardDAV addressbook just as readily. The protocol crates own the protocols (io-imap, io-jmap, io-maildir, io-webdav); this repository owns the config, the hooks and the supervision.

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `dav`), the collection it watches, and the hooks it fires. The config schema SHALL stay compatible with himalaya CLI and himalaya TUI, so one file can back every binary, and unknown keys SHALL be ignored rather than refused.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `on-item-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: Every configured account, or a chosen one
Bare `carillon watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account, and an unknown name SHALL be an error. One account's watch failure SHALL be logged and retried on its own without stalling the others.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its own collection, and Ctrl+C stops them together

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is logged and retried for that account alone, and the other account keeps watching

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

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, flags added, flags removed. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a hook filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The hooks SHALL be named after items rather than messages, and the message-shaped names SHALL keep working as aliases.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flags-added` with the flag named `Seen`

#### Scenario: A configuration written before the rename
- **GIVEN** an account configuring `hooks.on-message-added`
- **WHEN** the daemon loads it
- **THEN** it is read as `on-item-added` and fires exactly as it did

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection, which covers CalDAV and CardDAV alike, by polling an RFC 6578 `sync-collection` report. It SHALL request `getetag` and nothing else, so a poll never carries a contact or an event; it SHALL keep an href to etag picture of the collection, so that a member it has never seen reads as an arrival and a known member whose etag moved reads as an edit. A truncated report SHALL be drained immediately rather than at the next interval, and a sync token the server rejects SHALL cause a re-enumeration, which reports nothing because a re-baseline is not news.

#### Scenario: A contact is edited
- **GIVEN** a watch on a CardDAV addressbook
- **WHEN** an existing contact is modified and its etag changes
- **THEN** `on-item-changed` fires with the member's href, and nothing is fetched of the contact itself

#### Scenario: The server forgets its history
- **GIVEN** a watch whose stored sync token is older than the server keeps
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, no event is fired for what was already there, and the watch continues from the fresh token

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, not what it says. The daemon SHALL resolve an arrival into its summary (for mail: subject, sender, recipient, date) only when the account configures an `on-item-added` hook, and SHALL do so on a second connection, never the one holding the watch. Only a backend able to read one resolves anything; the others leave the summary empty. A resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no item hook
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
`carillon check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits non-zero
