---
cairn: spec
capability: daemon
status: current
---

# The watch daemon

carillon watches PIM accounts and fires local hooks on every change, and on every alarm of a watched calendar. It reads a TOML config of named accounts, watches each one on its own thread, and runs on one machine with no server apparatus: no HTTP listener, datastore, auth, custody, metering or billing.

It watches; it never syncs. A change is reported as it is seen (an item arrived, one left, one was edited, a flag moved) and nothing is stored between runs. What a hook wants beyond that, carillon goes and reads on demand.

Each backend brings its own way of learning about a change, and the daemon translates all of them into one vocabulary, so one hook runner serves them all: IMAP holds IDLE and reports UID-keyed deltas, JMAP polls `Email/changes`, Maildir re-lists the mailbox, and the DAV backends report what a collection did since a sync token. Mail is not the boundary: CalDAV holds events and tasks and CardDAV holds cards, and each is configured and named for what it holds rather than reduced to a word true of everything. The protocol crates own the protocols (io-imap, io-jmap, io-maildir, io-webdav); this repository owns the config, the hooks and the supervision.

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI. A backend the build leaves out SHALL NOT be such a key: its block SHALL load unread, so one file serves every build, and a command reaching it SHALL fail coded `backend-not-built`, naming the cargo feature to build with, each backend built by the feature named as its block (`caldav` and `carddav` apart, no `dav` feature, `caldav` alone building the alarms of a calendar) (`account check` in that backend's row, `watch` as one lost session of that account, never retried). A key carillon read before 1.0 (the account-level `folder`, `mailbox`, `collection`, `watch`, `hooks`, the `dav` backend, a backend's `hook` table, a hook name ending in `-added`, `-removed` or `-changed`) SHALL be refused by serde alone, as any key it does not read, never by name. A hook's `command` SHALL also be read under `cmd`, its alias wherever the family configures a command, a secret's included.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hooks.on-message-add` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

#### Scenario: A misspelt account key
- **GIVEN** an account holding `defualt = true`
- **WHEN** the configuration is read
- **THEN** it is refused naming `defualt` and its line, and no watch is started

#### Scenario: A backend the build leaves out
- **GIVEN** a build without the `imap` feature and an account holding an `imap` block
- **WHEN** `carillon account check` runs
- **THEN** the configuration loads, the imap row reads `ok: false`, `code: "backend-not-built"` and names `--features imap`, and the command exits 3, while a misspelt `imapp` block is still refused when read

#### Scenario: A DAV backend the build leaves out
- **GIVEN** a build with neither the `caldav` nor the `carddav` feature, and an account holding a `caldav` block
- **WHEN** `carillon account check` runs
- **THEN** the caldav row reads `code: "backend-not-built"` and names `--features caldav`, not `--features dav`; with a `carddav` block, the row names `--features carddav`

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-add.command`
- **WHEN** the configuration is read
- **THEN** it is refused as the unknown field `hooks`, pointing at no migration guide

#### Scenario: A hook named as in 0.2
- **GIVEN** an account holding `imap.hook.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused as the unknown field `hook`, and once that is fixed as the unknown field `on-message-added`

#### Scenario: A hook command spelt `cmd`
- **GIVEN** an account holding `imap.hooks.on-message-add.cmd = "true"`
- **WHEN** the configuration is read
- **THEN** it loads as `imap.hooks.on-message-add.command = "true"`, and a hook naming both is refused

### Requirement: Every configured account, or a chosen one
Bare `carillon watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account. `-a` SHALL take the account of that name, `default` being a name like any other, and a command needing one account and given no `-a` (`account check`) SHALL take the account marked `default = true`; `account list` takes none. A name no account carries SHALL be an error listing the accounts the configuration does hold, and a command needing one account with no default to pick SHALL name both ways of choosing one, so a failure to resolve an account always says what to do next; the name `default` failing SHALL add that omitting `-a` takes the account marked default. One account's watch failure SHALL be printed as its `session-lost` event and retried on its own without stalling the others, but for a backend the build leaves out: its `session-lost` event, coded `backend-not-built`, SHALL be printed once and that account stopped for the run, the others watching on. When every account stopped so, nothing is left to watch, and the watch SHALL fail with that failure, coded `backend-not-built`, exiting 1.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its own collection, and Ctrl+C stops them together

#### Scenario: A name no account carries
- **GIVEN** a configuration holding the accounts `perso` and `work`
- **WHEN** `carillon -a wrok watch` runs
- **THEN** it fails naming `wrok` and listing `perso, work`

#### Scenario: `-a default`
- **GIVEN** a configuration holding `perso`, marked `default = true`, and no account named `default`
- **WHEN** `carillon --json -a default account check` runs
- **THEN** it fails coded `account-missing`, saying to omit `-a` to take the account marked default, and `carillon account check` takes `perso`

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is printed as a `session-lost` event of that account and retried for that account alone, and the other account keeps watching

### Requirement: An account watches one collection, one way
An account SHALL watch the collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection of the same domain is a second account, which is also how it gets its own hooks. A backend serving several domains MAY name one collection per domain, since the domains do not share an event name and each therefore already has hooks of its own; what they share is the connection and the credential, which is what a second account would waste. Every backend SHALL name its collection by the id a listing of the account gives it, so a collection picked from a listing is watched without translation. A DAV block SHALL name `server`, the URL its principal is discovered from, or `home`, the calendar or addressbook home set pinned, or both, and SHALL be refused when read naming both when it names neither; `home` set, discovery is skipped, the connection opens on it and `server` is not read. A DAV collection named by one path segment SHALL be looked up under the pinned `home`, or else under the calendar or addressbook home set the server's principal names, read under `server` when the server names none; an absolute path SHALL be taken as it stands, and any other relative path read under the URL the connection opens on, `home` or `server`. A relative path SHALL be sent as io-webdav's `collection_path` composes one, each byte a path segment cannot hold raw percent-encoded and an escape already there kept, and every collection path SHALL end with a slash. A DAV collection id naming no collection (empty, `/` alone, or holding an empty, `.` or `..` segment, percent-encoded or split on `\` alike, as io-webdav's `check_collection_id` reads it) SHALL be refused by `watch` and `account check` before a connection is opened, coded `calendar-not-found` or `addressbook-not-found`: it would address the server root, the home set or a parent of either.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

#### Scenario: A calendar named by its listed id
- **GIVEN** a CalDAV server whose principal names the home set `/dav/cal/alice/`, holding `/dav/cal/alice/default/`
- **WHEN** an account sets `caldav.calendar = "default"`
- **THEN** the watch reads `/dav/cal/alice/default/`, whatever path `server` names

#### Scenario: A pinned home set
- **GIVEN** an account setting `caldav.home = "https://dav.example.org/dav/cal/alice/"` and `caldav.calendar = "default"`, and no `server`
- **WHEN** it is watched or checked
- **THEN** `/dav/cal/alice/default/` is read, and no principal or home set is asked for

#### Scenario: A collection id climbing out of the home set
- **GIVEN** an account setting `caldav.calendar = ".."`
- **WHEN** `carillon --json account check` runs
- **THEN** the caldav row reads `ok: false` and `code: "calendar-not-found"`, nothing reaches the server, and the command exits 3

#### Scenario: A collection id holding a space
- **GIVEN** an account setting `carddav.addressbook = "My Book"` under the home set `/dav/books/alice/`
- **WHEN** it is watched
- **THEN** `/dav/books/alice/My%20Book/` is read

### Requirement: The watch method belongs to the backend
The method SHALL be configured under its backend (`imap.watch`, `jmap.watch`, `maildir.watch`, and `watch` under each of `caldav` and `carddav`) and named by its mechanism, the way a SASL mechanism and an HTTP auth scheme already are. Each backend SHALL declare only the methods it has, so a method it does not have is refused when the configuration is read rather than when the watch runs. Unset, an account SHALL watch the best way its backend has: IDLE for IMAP, a held event stream for JMAP, a poll for the backends with nothing else. Every backend SHALL offer the poll, whose interval MAY be given and otherwise takes what suits that backend. Every duration of a method (the poll's `interval`, IDLE's `timeout`, the push `ping`) SHALL be read as pimalaya-config's duration: an integer of seconds, or a string holding a number and its unit (`s`, `m`, `h`, `d`, `w`); any other value SHALL be refused naming it.

#### Scenario: A duration with its unit
- **GIVEN** an account configuring `imap.watch.poll.interval = "5m"`
- **WHEN** the configuration is read
- **THEN** the poll runs every 300 seconds, as `imap.watch.poll.interval = 300` would, and `"5 minutes"` is refused naming it

#### Scenario: A server whose IDLE cannot be trusted
- **GIVEN** an IMAP account whose server accepts IDLE and then never speaks
- **WHEN** the account configures `imap.watch.poll.interval`
- **THEN** the watch re-reads the mailbox on that interval instead, reporting the same events

#### Scenario: A method the backend does not have
- **GIVEN** a Maildir account configuring `maildir.watch.idle`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the methods that backend has, and no watch is started

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown or a backend the build leaves out, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal: a connection opened inside a session (a round retried on a fresh connection, a JMAP event stream) SHALL present the credential resolved for it, never the one the session started with. A session reopened after its connection was lost SHALL report what moved meanwhile where its backend can tell: IMAP SHALL resume from the UIDs and flags the lost watch knew, and Gmail from the history cursor the lost session had reached, both held in memory for as long as the account is watched. An IMAP session that ended on a refusal rather than a lost connection, a mailbox recreated under the same name among them, SHALL take a new baseline, as every session of the other backends does.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

#### Scenario: A token rotated while a JMAP session holds it
- **GIVEN** a JMAP watch whose server, once the session is open, refuses its token with 401 and takes the one the token command now prints
- **WHEN** the next round is retried on a fresh connection, or the event stream is reopened
- **THEN** it presents the new token, and the change that came meanwhile fires its hook rather than losing the session

#### Scenario: A message arrives during an outage
- **GIVEN** an IMAP watch whose connection was lost
- **WHEN** a message arrives before the session is reopened
- **THEN** the reopened session fires `on-message-add` for it

#### Scenario: A mailbox recreated during an outage
- **GIVEN** an IMAP watch whose connection was lost, and whose mailbox was deleted and created again meanwhile
- **WHEN** the session is reopened and the server names another UIDVALIDITY
- **THEN** that session ends, and the next one takes a new baseline

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, a flag added, a flag removed, each carrying the domain of what it is about. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. The shared names SHALL be `Seen`, `Answered`, `Flagged`, `Deleted`, `Draft` and `Passed` (IMAP `$Forwarded`, JMAP `$forwarded`, the Maildir `P` letter), any other keyword reported as the backend sends it, and `$flag` and the `--json` events SHALL carry the shared name. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit but a Microsoft Graph draft, edited where it stands, and then only where no flag moved in the same edit, its `changeKey` moving with either, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The vocabulary SHALL stay one across the backends even though the hooks configuring it are per backend and per domain, so that one hook runner serves all of them.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flag-add` with the flag named `Seen`

#### Scenario: An item that is edited where it stands
- **GIVEN** a CardDAV account watching an addressbook
- **WHEN** a contact is edited and its etag moves
- **THEN** `carddav.hooks.on-card-change` fires, an event only a Microsoft Graph mailbox also accepts a hook for among the mail backends

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval. A sync token the server rejects SHALL cause a re-enumeration read against the picture, reporting only what differs. A server refusing the report SHALL be listed with a `PROPFIND` on every poll instead, read against the picture the same way. A re-enumeration or a listing that is truncated, or that holds a member the server answered with an error status (a 403, a 5xx), SHALL report no removal, and the error SHALL be logged as a warning; such a member in a report SHALL read as neither changed nor removed. A round that fails SHALL be run again on a fresh connection, picture and token kept, before the session is given up. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-change` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, only what differs from the picture is reported, and the watch continues from the fresh token

#### Scenario: A server with no sync-collection
- **GIVEN** a CalDAV server answering `sync-collection` with `501 Not Implemented`
- **WHEN** an event is added, another edited and a third deleted between two polls
- **THEN** the listing that poll makes fires `on-event-add`, `on-event-change` and `on-event-remove` for them

#### Scenario: An idle connection closed between two polls
- **GIVEN** a DAV watch whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again from the same token, and the session is given up only if that also fails

### Requirement: A DAV block may send no credential
`caldav.auth` and `carddav.auth` SHALL be optional, a block naming none sending no `Authorization` header, as `auth = "none"` does: a server asking for no credential (Radicale with `auth = none`, a server behind a VPN or an authenticating reverse proxy) needs no key for it, as cardamum and calendula read their DAV blocks. A server answering a request 401 to a block sending no credential SHALL fail the watch and `account check` with a message saying so, "DAV server answered 401: it asks for credentials, and the account configures no `auth`", above io-webdav's `WebDAV server returned HTTP 401`, its code unchanged (`null`). A block naming a credential the server refuses fails as before, without the sentence.

#### Scenario: A server asking for credentials a block does not give
- **GIVEN** a CardDAV account naming no `auth`, its server answering 401
- **WHEN** `carillon account check` runs
- **THEN** the `carddav` row fails uncoded, its error saying the server asks for credentials and the account configures no `auth`

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, and sometimes what it says. A backend SHALL report an arrival together with the summary it already read, and SHALL read one it does not have only when the active backend configures the arrival hook of that domain. JMAP SHALL take its summary from the `Email/get` its round already makes, asking for the envelope properties only when a hook wants them. IMAP SHALL read one on a second connection, never the one holding the watch, an IMAP delta naming a UID and nothing more. A subject and a display name SHALL reach a hook as text, RFC 2047 encoded words decoded (adjacent ones joined, RFC 2047 §6.2), on IMAP and Gmail alike, a header holding no encoded word reading as written; an address is never decoded. The sender and the recipient SHALL be the first mailbox of the `From` and `To` lists on every backend, Gmail reading its headers as RFC 5322 §3.4 reads an address list, so a quoted display name holding a comma stays one name. A backend that can read no envelope SHALL leave the summary empty, and a resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no arrival hook
- **GIVEN** an account whose only hook is `imap.hooks.on-flag-add`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

#### Scenario: A JMAP arrival with an envelope
- **GIVEN** a JMAP account whose `jmap.hooks.on-message-add` notification names `$subject` and `$sender`
- **WHEN** a message arrives
- **THEN** both are filled from the round's own `Email/get`, with no second request

#### Scenario: An IMAP subject in encoded words
- **GIVEN** an IMAP account whose `imap.hooks.on-message-add` summary reads `$subject from $senderName`
- **WHEN** a message arrives whose ENVELOPE subject is `=?UTF-8?Q?R=C3=A9union?=` and whose sender is `=?ISO-8859-1?Q?Andr=E9?=` at `andre@example.org`
- **THEN** the notification reads `Réunion from André`, and `$sender` reads `André <andre@example.org>`

### Requirement: Ctrl+C is prompt on every path
`watch` SHALL install pimalaya-cli's `signal::Shutdown`: the first SIGINT (Ctrl+C), SIGTERM or SIGHUP SHALL request the shutdown, which the watch reads, and a second one SHALL end the process with 130. A requested shutdown SHALL be honoured within roughly a second on every path a watch can be waiting in: idling on a connection, sleeping between polls, backing off before a reconnect, or resolving an arrival's envelope. A watch sleeping between two polls or backing off, and the hook runner while no command runs, SHALL wake for the next round, the next command or the shutdown alone, never on a step of their own. No path SHALL wait on a server that has stopped answering: every connection the daemon opens SHALL carry a read deadline and SHALL hand back the not-ready failures rather than letting the transport retry them away, since the deadline exists to be the wakeup that re-reads the flag. A request a DAV round or an IMAP envelope resolution sends SHALL fail once the server has sent nothing for 60 seconds, so a connection gone dead under the watch (a resume, a network change) is dropped and reopened as any failed round's is, rather than held until TCP gives up.

#### Scenario: Ctrl+C while resolving against a silent server
- **GIVEN** a watch resolving an arrival's envelope against a server that has stopped answering
- **WHEN** the user presses Ctrl+C
- **THEN** the read deadline expires, the flag is seen, and the watch ends rather than waiting for the transport's own timeout

#### Scenario: A DAV connection gone dead during a suspend
- **GIVEN** a DAV watch whose connection died while the machine slept, the server never answering on it again
- **WHEN** the next round sends its report
- **THEN** the report fails after 60 seconds of silence, and the round runs again on a fresh connection

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification (`notify`), a command (`command`, a shell line or a program and its arguments), or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A variable SHALL reach a notification as text: pimalaya-config's rendering escapes `&`, `<`, `>`, `"` and `'` where the notification daemon reads markup (every Unix but macOS), turns a line break or a tab into a space and drops the other control and bidirectional override characters, the template's own text kept as written. A hook command's stdout SHALL be carillon's stderr, so a watch's stdout holds its events and result alone. A hook that fails SHALL be printed as a `hook-failed` event naming the reaction that failed (`notify` or `command`), a command exiting non-zero being a failure, and left behind: neither a missing notification daemon nor a broken script SHALL end the watch. A hook SHALL NOT half-fire for a reason the configuration could have been refused for: what a template may name is settled when the file is read, so the only failures left at watch time are the ones the machine around it produced.

A hook command SHALL run off the watch, so a slow one holds neither the next change, an alarm nor a shutdown: it SHALL be spawned with no stdin and reaped by a thread of its own, its failure printed as `hook-failed` once it ends, and the notification of the same hook shown when it fires. One command of an account SHALL run one instance at a time: each firing while it runs SHALL wait its turn, in order and with its own variables, so every arrival and every due alarm runs its command; a firing equal to one already waiting (the same command, the same variables) SHALL be dropped, and past 1000 waiting a firing SHALL be dropped and counted, the count logged as a warning once the command ends or the watch stops. A hook MAY carry a `timeout` beside its command, read as pimalaya-config's `Timeout` (a duration, zero refused); unset, a command runs until it ends. A command still running once its timeout passed SHALL be sent SIGTERM (its process group, on Unix), then SIGKILL once pimalaya-config's `Timeout::GRACE` (five seconds) passed too, and its failure printed as `hook-failed` naming the timeout, whatever it exited with; the run waiting next on it then starts. On shutdown, every running command SHALL be sent SIGTERM (its process group, on Unix, a terminal's Ctrl+C reaching carillon alone) and given one second to end, then SIGKILL and reaped, the runs waiting being dropped, so a shutdown keeps its promise of a second or so.

#### Scenario: The hook script exits non-zero
- **GIVEN** an account whose hook `command` exits with an error
- **WHEN** it fires
- **THEN** a `hook-failed` event naming `command` and the exit status is printed, and the watch keeps running

#### Scenario: A hook still running when the next change comes
- **GIVEN** a Maildir account whose `on-message-add` command runs for a minute
- **WHEN** two messages arrive a second apart, then SIGTERM is sent
- **THEN** both `changed` events are printed as they come, and the watch exits within a couple of seconds, its running command sent SIGTERM, then SIGKILL past the second of grace, and the run waiting for the second message dropped

#### Scenario: A hook command past its timeout
- **GIVEN** an account whose `on-message-add` hook reads `timeout = "30s"` beside a command that hangs
- **WHEN** a message arrives and the command is still running 30 seconds later
- **THEN** it is sent SIGTERM, then SIGKILL five seconds later if it is still running, a `hook-failed` event naming `command` and the timeout is printed, and the watch keeps running

#### Scenario: A subject holding markup
- **GIVEN** a Linux desktop and an `on-message-add` notification summary reading `$sender`
- **WHEN** a message from `Alice <alice@example.org>` arrives
- **THEN** the notification reads `Alice <alice@example.org>` as text, the daemon given `Alice &lt;alice@example.org&gt;`

### Requirement: The account can be checked before it is watched
`carillon account check` SHALL open each backend the account declares and report per backend whether it worked, so a credential or connectivity error surfaces before a watch is started rather than in the middle of one. `carillon check`, its name before the account group, SHALL run it too, hidden from the help. The report SHALL be pimalaya-cli's `account::AccountCheckOutput`, a list of one account, `{"accounts": [{"account", "backends"}]}`, each row its `account::BackendCheck`, `{backend, ok, code, error, tls}`, `tls` the TLS the backend's table resolved to (`{provider, crypto, cert, clientCert}`, each `null` where the build's default answers), `null` for `maildir` and `pimdir`, for an `imap://` server without `starttls` and for an `http://` JMAP or DAV server, `code` typed by carillon's vocabulary and `null` when the backend passed or its failure carries none, read as every carillon failure is (a coded error in the chain, else a typed cause a backend's reader knows); a backend the build leaves out SHALL be a failed row coded `backend-not-built`. When any backend failed, the command SHALL still print the report and exit 3, pimalaya-cli's partial outcome. Its text SHALL be the layout of every family `account check`: an `Account` field, then one field per backend reading `OK` or `FAIL (<error>)`, followed by `, TLS <parts>` when `tls` is set, the values aligned past the longest label (pimalaya-cli's `Fields`), the account and a failure's text passed through pimalaya-cli's `sanitize::text`. It SHALL resolve the whole account through one secret resolver, so a credential command named by two of its backends is spawned once rather than once per backend, and a `pass` or `gpg` entry unlocks its store once. The resolver SHALL live no longer than the account it was built for.

#### Scenario: A wrong password
- **GIVEN** an account whose IMAP password is wrong
- **WHEN** `carillon account check` runs
- **THEN** the imap backend is reported as failed with the server's reason, and the process exits 3

#### Scenario: One credential named by two backends
- **GIVEN** an account whose `caldav` and `carddav` tables read the same `pass` entry
- **WHEN** `carillon account check` runs
- **THEN** the command is spawned once, both backends are opened with its value, and the key is unlocked once

#### Scenario: A script calling `check`
- **GIVEN** a script running `carillon --json check`
- **WHEN** it runs
- **THEN** it prints what `carillon --json account check` prints and exits as it does, and `carillon --help` lists `account` but not `check`

#### Scenario: A backend inheriting the account's TLS
- **GIVEN** an account holding `tls.rustls.crypto = "ring"`, `tls.cert` and `tls.client`, an `imaps://` IMAP block holding `imap.tls.cert` and a Maildir block
- **WHEN** `carillon --json account check` runs
- **THEN** the imap row's `tls` names `ring`, the IMAP certificate and the account's client certificate, and the maildir row's `tls` is `null`

### Requirement: The accounts can be listed
`carillon account list` (`ls` its visible alias) SHALL print every account the configuration declares, sorted by name, opening nothing and taking no account, so it runs where no account is marked default. Its output SHALL be pimalaya-cli's `account::AccountListOutput`, `{accounts: [{name, default, backends, notBuilt}]}`, `backends` the backends the account carries a block for that the build serves and `notBuilt` the ones it leaves out, each in the order `--backend auto` tries them. Its text SHALL be one line per account with no header row: the name sanitized, the backends then each left-out one followed by ` (not built)`, comma-separated, and `default` on the default account. `account` SHALL hold `list` and `check`, as in every family binary, and no `account capability`: carillon has no unified API whose capabilities a backend would serve.

#### Scenario: A build leaving a backend out
- **GIVEN** a build without the `imap` feature and an account `a`, marked default, holding an `imap` block
- **WHEN** `carillon --json account list` runs
- **THEN** it prints `{"accounts":[{"name":"a","default":true,"backends":[],"notBuilt":["imap"]}]}` and exits 0, and its text reads `a  imap (not built)  default`

### Requirement: The hooks belong to the backend
The hooks SHALL be configured under their backend, in its `hooks` table (`imap.hooks`, `jmap.hooks`, `maildir.hooks`, `caldav.hooks`, `carddav.hooks`, `msgraph.hooks`, `gmail.hooks`, `gcal.hooks`, `gpeople.hooks`, `pimdir.hooks`), as Comodoro configures its own. Each backend SHALL declare only the events it reports, so a hook it cannot fire is refused when the configuration is read rather than never firing. The variables a hook templates against SHALL be the ones its backend can fill, which is why the envelope names belong to the arrival hook of a backend resolving one. An account declaring more than one backend SHALL configure the hooks of each, since a hook written for one backend says nothing about what another would report.

#### Scenario: A hook the backend cannot fire
- **GIVEN** an account configuring `carddav.hooks.on-flag-add`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the events CardDAV reports, and no watch is started

#### Scenario: Two backends on one account
- **GIVEN** an account declaring both `imap` and `maildir`, with an `imap.hooks.on-message-add` summary reading `New mail from $sender`
- **WHEN** the account is watched with `-b maildir`
- **THEN** no hook fires from the IMAP table, and the Maildir table is what the watch reads

### Requirement: An event is named after its domain
A hook SHALL be named `on-<subject>-<verb>`, after what it carries and what happened to it, the verb bare (`add`, `remove`, `change`, `trigger`), as Comodoro names its hooks. Mail SHALL be `on-message-add` and `on-message-remove`, whichever of IMAP, JMAP and Maildir reports it, and `on-message-change` for a Microsoft Graph draft edited where it stands, which no other mail backend accepts. A CardDAV addressbook SHALL be `on-card-add`, `on-card-remove` and `on-card-change`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. An alarm of an event or a task SHALL be `on-alarm-trigger`, named after itself as a flag is, whichever item holds it. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hooks.on-event-add`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-add`

#### Scenario: A message edit hook on JMAP
- **GIVEN** an account configuring `jmap.hooks.on-message-change`
- **WHEN** the configuration is read
- **THEN** it is refused, a JMAP email being immutable

### Requirement: A flag hook fires once per flag
`on-flag-add` and `on-flag-remove` SHALL fire once for each flag that moved rather than once for the delta, so `$flag` SHALL always name the flag the firing is about and no plural variable SHALL be exposed. The optional `flags = [...]` filter SHALL narrow which of those firings happen, matching one flag at a time, with or without a leading `\` or `$` and without regard to case. A backend with no flags SHALL take no flag hook at all.

#### Scenario: Two flags set at once
- **GIVEN** an IMAP account with an unfiltered `imap.hooks.on-flag-add` command
- **WHEN** one STORE sets both `\Seen` and `\Flagged`
- **THEN** the command runs twice, once with `$flag` as `Seen` and once as `Flagged`

#### Scenario: A filtered flag hook
- **GIVEN** the same account filtering `flags = ["Seen"]`
- **WHEN** the same STORE sets both flags
- **THEN** the command runs once, with `$flag` as `Seen`

### Requirement: A CalDAV calendar knows its components
A CalDAV watch SHALL resolve what its collection holds from `supported-calendar-component-set` when the watch starts. A calendar advertising a single component SHALL report every member as that component, at no cost per member. A calendar advertising several SHALL read `getcontenttype` on a member it has not seen and route it by the `component` parameter RFC 4791 §10.1 allows; a member whose content type names no component SHALL be taken for the one domain the account's hooks name, or for an event when they name both. A member SHALL never be fetched to find out what it is, since a poll carrying a VEVENT is what asking for etags alone exists to avoid; reading a property is not fetching a member. The domain of each member SHALL be remembered beside its etag, since a removal leaves only an href behind.

#### Scenario: A task is deleted from a calendar holding both
- **GIVEN** a CalDAV account watching a calendar of events and tasks, with `on-task-remove` configured
- **WHEN** a VTODO is deleted
- **THEN** `on-task-remove` fires, the domain coming from what the watch remembered of that href, and `on-event-remove` does not

#### Scenario: A calendar that holds one component
- **GIVEN** a CalDAV account watching a calendar advertising `VEVENT` alone
- **WHEN** a member is added
- **THEN** `on-event-add` fires without any further request, and the account's task hooks are refused when the configuration is read

#### Scenario: A server naming no component
- **GIVEN** a calendar advertising `VEVENT` and `VTODO` whose members' content type is a bare `text/calendar`, and an account hooking `on-event-*` alone
- **WHEN** an event is added
- **THEN** `on-event-add` fires

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read, a default or not. Notifications SHALL be templated in the shell's syntax (`$name`, `${name}`, `${name:-default}`, `$$`), any other `${...}` form being refused when read, and variables SHALL be camelCase. `$id` SHALL be available to every hook, the collection SHALL be available under the name its backend configures it as and under `$collection` whatever the backend, `$flag` to a flag hook, and the envelope names (`$subject`, `$date`, `$sender`, `$senderName`, `$senderAddress`, `$recipient`, `$recipientName`, `$recipientAddress`) only to the arrival hook of a backend that resolves one. A `${name:-default}` SHALL take its default when the value is empty. A command SHALL NOT be validated, its placeholders reaching it as environment variables of the same names, where an unset name is ordinary. A shell line expands them itself; a program and its arguments SHALL run as written, a `$name` in an argument reaching the program as those characters, which is why the sample hands such a command to `sh -c`.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hooks.on-message-remove` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hooks.on-message-add` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

#### Scenario: A template in a former spelling
- **GIVEN** an account whose `jmap.hooks.on-message-add` notification summary reads `$sender_name` or `${subject:none}`
- **WHEN** the configuration is read
- **THEN** it is refused, naming `sender_name` or `${subject:none}`

#### Scenario: One command for every domain
- **GIVEN** a `caldav` account on `work` and a `carddav` one on `default`, both running the same command
- **WHEN** an event and a contact are added
- **THEN** the command reads `collection=work` for the first and `collection=default` for the second, from the same variable

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the collection it watches, required, under the name its domain uses: `imap.mailbox`, `maildir.mailbox`, `gmail.mailbox`, `caldav.calendar`, `gcal.calendar`, `carddav.addressbook` and `gpeople.addressbook`, and for the backends serving several domains `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`, and the same three under `msgraph`, of which at least one. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the name the collection its event is about was configured under, `$id` and `$collection` being the two variables every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hooks.on-message-add` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hooks.on-event-add` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

#### Scenario: A hook naming another domain's word
- **GIVEN** an account whose `jmap.hooks.on-card-add` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a card is configured and templated as `$addressbook`

### Requirement: A watch survives the change it reports
A watch SHALL keep working across the changes it reports. A connection a backend's own protocol closes as part of reporting SHALL be reopened before the next request rather than written into, and a round that fails SHALL be retried once on a fresh connection before the session is given up. A round SHALL advance no state it did not complete, so running it twice reports nothing twice. A JMAP event stream SHALL be reopened with the id of the last event read (RFC 8620 §7.3), so a server keeping its events pushes what moved between two streams. A stream that sent nothing, its pings included, for three of the `ping` intervals it asked for SHALL be taken for dead and opened again.

#### Scenario: A JMAP event stream that closes after its state change
- **GIVEN** a JMAP account watching over the event stream, which the server closes after reporting one state change
- **WHEN** a message arrives
- **THEN** the round that follows runs on a fresh connection and fires the hook, rather than losing the session and re-baselining the change away

#### Scenario: A polling watch whose idle connection was closed
- **GIVEN** a JMAP account polling on an interval, whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again, and the session is given up only if that also fails


### Requirement: The TLS handshake is configured under its backend
An account MAY carry a `tls` table, and every backend speaking TLS SHALL carry a `tls` table and an `alpn` key of its own, and the runtime TLS handle SHALL be built through one conversion taking that list, so a connection cannot be opened without saying what it negotiates. `alpn` SHALL be a list of ALPN identifiers: unset SHALL take the default the backend's client crate owns (`["imap"]` over IMAP, `["http/1.1"]` over JMAP and WebDAV), an empty list SHALL skip ALPN negotiation, and a non-empty list SHALL replace the default. Only rustls SHALL read it, native-tls having no ALPN. A `tls` table SHALL hold `provider` (`rustls` or `native-tls`), `rustls.crypto` (`aws` or `ring`), `cert`, a PEM certificate trusted beside the platform's, and `client.cert` and `client.key`, a PEM client certificate presented when the server asks for one and its private key (PKCS#8 under native-tls), both required once `client` is given: the keys every Pimalaya CLI spells alike (pimalaya-config cairn/spec/toml.md). A backend's table SHALL take from the account's, field by field, each of `provider`, `rustls.crypto`, `cert` and `client` it leaves unset, `client` as one, so a certificate is never paired with another table's key. Every path in a `tls` table SHALL be expanded when the file is read, so a leading tilde or a shell variable names the same thing there as everywhere else.

#### Scenario: A server that refuses the handshake carrying an ALPN
- **GIVEN** a CalDAV server whose TLS terminator rejects a `ClientHello` offering `http/1.1`
- **WHEN** the account sets `caldav.alpn = []`
- **THEN** the connection is opened with no ALPN extension at all

#### Scenario: A configuration naming no ALPN
- **GIVEN** an account carrying no `alpn` key
- **WHEN** it is watched
- **THEN** the backend offers the default its client crate owns, unchanged from before the key existed

#### Scenario: A certificate under the home directory
- **GIVEN** `imap.tls.cert = "~/certs/example.pem"`
- **WHEN** the configuration is read
- **THEN** the path is expanded against the home directory rather than read as a relative `./~/certs/example.pem`

#### Scenario: A backend table over the account's
- **GIVEN** an account holding `tls.provider = "native-tls"` and `tls.cert = "~/ca.pem"`, and its `imap.tls.cert = "~/imap.pem"`
- **WHEN** the configuration is read
- **THEN** IMAP runs native-tls trusting `imap.pem`, and every other network backend of the account native-tls trusting `ca.pem`

### Requirement: A network backend reaches its server through a configurable proxy
An account SHALL accept a `proxy` table, and every backend opening a socket (`imap`, `jmap`, `caldav`, `carddav`) SHALL accept one of its own, overriding the account's. The table SHALL carry a `url` (`socks5://`, `socks5h://` or `http://`), an optional `username` and an optional `password` resolved as a secret; a password without a username SHALL be refused. A backend naming none SHALL inherit the account's when the file is loaded, and with neither the `all_proxy` and `https_proxy` environment variables SHALL be read, `no_proxy` and loopback bypassing them.

#### Scenario: One account behind a SOCKS proxy
- **GIVEN** an account with `proxy.url = "socks5h://127.0.0.1:9050"` and an `imap` block naming no proxy
- **WHEN** it is watched
- **THEN** the IMAP connection, the envelope resolver's and every reconnect go through the proxy

#### Scenario: A configuration naming no proxy
- **GIVEN** an account carrying no `proxy` key anywhere
- **WHEN** it is watched with `all_proxy` unset and `https_proxy` unset
- **THEN** it connects directly


### Requirement: Every printed output has a published schema
Every command handing data to the printer SHALL return a named `*Output` type deriving `Display`, `Serialize` and `JsonSchema`, and `carillon json-schema` SHALL publish the schema of each, keyed by the command path joined with hyphens and prefixed `carillon-`. Every type reaching the printer SHALL spell its keys in camelCase, declared as `rename_all` on the type, which is the convention of the `--json` output alone and not of the TOML configuration, whose keys stay kebab-case. A command that writes files rather than data SHALL stay out of the registry. `watch` SHALL register its result under `carillon-watch` and its events under `carillon-watch.events`. `account list` and `account check` SHALL be registered under `carillon-account-list` and `carillon-account-check`, and the hidden `check` SHALL keep `carillon-check`, describing the same payload.

#### Scenario: A consumer reading the check payload
- **GIVEN** `carillon json-schema carillon-account-check`
- **WHEN** it runs
- **THEN** the JSON Schema of the `--json` payload of `carillon account check` is printed on stdout, and `carillon-check` describes the same payload

#### Scenario: Every schema at once
- **GIVEN** `carillon json-schema --dir <DIR>`
- **WHEN** it runs
- **THEN** one file per command is written there, the directory being created if it is not already

### Requirement: The JSON contract is described and versioned
`carillon json-schema` SHALL describe, under the key `carillon-error`, the failure object every command prints under `--json`, its `code` typed by the code vocabulary and pimalaya-cli's common codes, `usage-invalid` and `request-origin` (a raw request off its API, which carillon, having no `request` command, never raises). Every generated schema SHALL carry an `$id` naming the contract version, which is the minor version while Carillon is below 1.0 (`v0.2`) and the major version after. Every registered payload SHALL be a JSON object, checked by a test. Piped `--json` output SHALL print one value per line, and nothing else SHALL reach stdout, logs included.

#### Scenario: A caller reads a failure against its schema
- **GIVEN** `carillon json-schema carillon-error`
- **WHEN** it runs
- **THEN** the schema of the JSON failure object is printed, its `$id` being `https://pimalaya.org/schemas/carillon/v0.2/carillon-error.json`

### Requirement: A watch streams what it observes
`watch` SHALL print one event line per thing it observes while it runs, each an object tagged `event` and naming its `account`: `session-opened` once a session is established, connected, authenticated and its collections read, right before it first waits for a change (`backend`, `method` `idle`, `poll` or `push`, and its `collections`, each `{name, value}`), `session-lost` when one ends or cannot open (`code`, `error`, `sources`), a session failing before it is established printing `session-lost` alone and a reopened one printing `session-opened` again once established, `changed` for every change a backend reports, whether a hook is configured for it or not (`collection` and `change`, the change as its hook receives it, tagged `kind`: `item-added`, `item-removed`, `item-changed`, `flag-added`, `flag-removed`, with its `domain`, `id` and, for a flag, `flag`), `hook-failed` when a reaction of a hook fails (`hook`, the hook's name such as `on-message-add` or `on-alarm-trigger`, `id`, the item it fired for, `reaction`, `code`, `error`, `sources`), and `alarm-fired` when an alarm reaches its trigger (`collection`, `id`, `uid`, `kind`, `summary`, `action`, `trigger`, an RFC 3339 instant in UTC with `Z`, as every instant the family prints; the hook variables `$trigger` and `$at` keep the system zone's offset). The account threads SHALL hand their events to the command, which alone prints them, so no two lines interleave. Once a shutdown is requested, it SHALL stop every account, print exactly one result line, `{"stoppedBy": "signal"}` (text: `Watch stopped by a signal`), and exit 0. A text event SHALL be one line whatever its strings hold: what a server, a store or the configuration hands it (an account, a collection, an id, a flag, a summary, an error and its causes) SHALL pass through pimalaya-cli's `sanitize::text`, its control and bidi characters printing as U+FFFD, as the family's text listings print them. Its printer SHALL stream: text events reach stdout even when it is a pipe, and nothing else SHALL reach it, a hook command's output going to stderr. A stdout no one reads any more SHALL stop the watch as a signal does, and its failure SHALL be the command's.

#### Scenario: A watch piped to a status bar
- **GIVEN** a Maildir account whose `on-message-add` command exits 3
- **WHEN** `carillon --json watch` runs piped, a message arrives, then SIGINT is sent
- **THEN** the pipe receives `session-opened`, `changed` (`item-added`), `hook-failed` (`command`), then `{"stoppedBy":"signal"}`, and the exit status is 0

#### Scenario: A session that cannot open
- **GIVEN** a Maildir account whose mailbox the Maildir does not hold
- **WHEN** `carillon --json watch` runs, then SIGINT is sent past the first backoff
- **THEN** stdout receives `session-lost` events coded `mailbox-not-found` and the result, never `session-opened`, and the exit status is 0

### Requirement: A JMAP account watches every domain it configures
The JMAP backend SHALL watch mail, contacts and calendar events, each under the collection key its domain uses (`jmap.mailbox`, `jmap.addressbook`, `jmap.calendar`), of which at least one SHALL be given. It SHALL fire `on-message-*` and `on-flag-*` for mail, `on-card-*` for contacts and `on-event-*` for calendar events, and SHALL NOT offer `on-task-*`, the JMAP calendars draft having no task type. A hook naming a domain the account configured no collection for SHALL be refused when the configuration is read, naming the hook and the key it would need. All the domains SHALL share one session, one connection and one event stream, that being what JMAP offers over a protocol needing an account per domain. Each collection SHALL be read when its session opens with its state first, so an item moving while it is listed and missing from the listing is reported by the first round rather than lost between the two, one the listing already holds reading as there before the watch (a card or an event so held may read as edited), then listed page by page, a page asking for no more than the server's `maxObjectsInGet` and 500; a listing whose count does not end at the `total` the server answered SHALL be read again from the start, and the session given up after three such listings. A round SHALL ask `/changes` again for as long as the server answers `hasMoreChanges`, so a batch the server splits fires every hook in one round, each answer applied whole before the next is asked for. A server answering `cannotCalculateChanges` (RFC 8620 §5.2) SHALL NOT end the session nor take a new baseline that hides the gap: the collection SHALL be listed again, its state read first as when the session opens, and only what differs from the picture the watch held reported, an item never seen an arrival (with its envelope when the arrival hook wants one), one no longer listed a removal, a keyword moved a flag; a listing names ids and keywords only, so an edit of a card or an event the gap hides is not told.

#### Scenario: Changes the server can no longer calculate
- **GIVEN** a JMAP account watching `jmap.mailbox = "INBOX"`, whose server answers `Email/changes` from the watch's state with `cannotCalculateChanges`
- **WHEN** a message arrived meanwhile
- **THEN** `on-message-add` fires once for it, and the watch goes on from the state read before the listing

#### Scenario: Changes answered in pages
- **GIVEN** a JMAP account whose server answers `Email/changes` two ids at a time with `hasMoreChanges`
- **WHEN** three messages arrive before a round
- **THEN** that round fires `on-message-add` for all three

#### Scenario: One account, three domains
- **GIVEN** a JMAP account configuring `jmap.mailbox`, `jmap.addressbook` and `jmap.calendar`
- **WHEN** a contact is edited
- **THEN** `jmap.hooks.on-card-change` fires, over the same connection the mail watch holds

#### Scenario: A hook for a domain the account does not watch
- **GIVEN** a JMAP account configuring `jmap.mailbox` and `jmap.hooks.on-card-add`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the `jmap.addressbook` it would need

#### Scenario: A mailbox larger than a page
- **GIVEN** a JMAP account watching a mailbox of 20,000 messages, on a server answering at most 500 ids per `Email/query`
- **WHEN** the session opens
- **THEN** the watch holds all 20,000, and a message arriving after its page was listed fires `on-message-add` on the first round


### Requirement: Microsoft Graph and Google accounts are watched through their own APIs
carillon SHALL offer four bearer-authenticated backends, each polled, none offering another method. `msgraph` SHALL watch mail, contacts and calendar events under `msgraph.mailbox`, `msgraph.addressbook` and `msgraph.calendar`, at least one given, sharing one token and one connection; its mail and contacts SHALL be read through a delta asking for 1000 items a page on every page of a round (`Prefer: odata.maxpagesize`), Graph answering fewer where it caps them; `contacts` SHALL name the default contact folder, and a folder or calendar SHALL be found by id wherever it sits, or by name. `gmail` SHALL watch a label under `gmail.mailbox`, `gcal` a calendar under `gcal.calendar`, by id or name, and `gpeople` a contact group under `gpeople.addressbook`, `contacts` naming every contact the account owns rather than a group. Each SHALL fire the hooks of the domains it holds and declare no other, flags being reported under the shared names (`Seen`, `Flagged`). An expired change feed SHALL NOT report what the gap hides as removals: where the collection can be listed again it SHALL be, and only what differs from the known picture reported; Gmail, whose history cannot be listed again, SHALL resume from the current cursor, as it SHALL from a history whose listing does not end.

#### Scenario: A follow-up flag in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Inbox"` with `msgraph.hooks.on-flag-add.flags = ["Flagged"]`
- **WHEN** a message in the inbox is flagged for follow-up
- **THEN** the hook fires with `$flag` set to `Flagged`

#### Scenario: A draft edited in Outlook
- **GIVEN** an account watching `msgraph.mailbox = "Drafts"` with `msgraph.hooks.on-message-change`
- **WHEN** a draft's subject, body or recipients are edited
- **THEN** the hook fires with the draft's `$id`, which the handler reads back itself

#### Scenario: A far-future event
- **GIVEN** an account watching `msgraph.calendar`
- **WHEN** an event two years ahead is edited
- **THEN** `msgraph.hooks.on-event-change` fires, no window hiding it

#### Scenario: An expired Google sync token
- **GIVEN** a `gcal` account whose sync token the server answers `410 Gone`
- **WHEN** the next poll runs
- **THEN** the calendar is re-read and only what differs from the known picture is reported

#### Scenario: Every Google contact
- **GIVEN** a `gpeople` account watching `contacts`
- **WHEN** a contact belonging to no group is added
- **THEN** `gpeople.hooks.on-card-add` fires

### Requirement: Notifications are shown when the build can show them
A `notify` cargo feature, on by default, SHALL decide whether hooks can show notifications. A build without it SHALL refuse a configuration carrying a `notify` table, naming the feature.

#### Scenario: A headless build
- **GIVEN** a build without the `notify` feature
- **WHEN** a configuration carrying `imap.hooks.on-message-add.notify.summary` is read
- **THEN** it is refused, naming the `notify` cargo feature

Times in the alarm scenarios below are UTC unless a zone is named.


### Requirement: A calendar fires its alarms
The `caldav` and `pimdir` backends SHALL offer `on-alarm-trigger`, firing once per firing of every `VALARM` of the events and tasks of the watched calendar, at its trigger, as ical-rs resolves it (`IcalAlarms`), repetitions included. A firing SHALL be dropped when the component holding its alarm is `STATUS:CANCELLED`, when that component is a `VTODO` completed (`STATUS:COMPLETED` or a `COMPLETED` property), when its alarm's `ACKNOWLEDGED` (RFC 9074 6) is at or after its trigger, and when its `ACTION` is neither `DISPLAY` nor `AUDIO`. No other backend SHALL accept the hook.

#### Scenario: A firing at its trigger
- **GIVEN** a CalDAV account with `caldav.hooks.on-alarm-trigger.notify.summary = "$summary"`, watching a calendar holding an event "Standup" at 09:00 with a `-PT15M` display alarm
- **WHEN** the clock reaches 08:45
- **THEN** one notification titled "Standup" is shown

#### Scenario: A cancelled event
- **GIVEN** an event with `STATUS:CANCELLED` holding a `-PT15M` alarm
- **WHEN** its trigger is reached
- **THEN** the hook does not fire

#### Scenario: An email alarm
- **GIVEN** an event holding an `ACTION:EMAIL` alarm
- **WHEN** its trigger is reached
- **THEN** the hook does not fire

#### Scenario: An alarm hook on an addressbook
- **GIVEN** an account configuring `carddav.hooks.on-alarm-trigger`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hooks a CardDAV account has

### Requirement: Alarms are held in memory
A watch SHALL resolve the firings of the calendar it watches over a window ending a horizon after now (48 hours), and SHALL build it again when a member changes and when half the horizon has passed. Nothing SHALL be written between runs: a watch starting SHALL fire nothing whose trigger is before its start, and SHALL never fire one firing twice. A firing SHALL be told apart by the `UID` of the component holding its alarm, the override, the alarm's index, its repetition, its occurrence and its trigger, never by the backend's id of the item holding it, so an item read again under another id (a pimdir store synced anew, a CalDAV member moved to another href) fires nothing it already fired. A floating time SHALL resolve in the system's time zone, else as if on UTC, and zones SHALL come from the database bundled in the binary. Each account with an alarm hook SHALL wait for its next firing on a thread of its own, living as long as the account rather than its session, so the alarms a lost session read keep firing on time until the next session replaces them with what it reads. The thread SHALL wait on a ceichas timer, the wall clock counting a suspend and following a clock change, and SHALL not wake before its next firing, a move of its firings or a shutdown, which wakes every alarm thread once the shutdown is requested. The watch SHALL keep its own interval.

#### Scenario: A restart does not repeat
- **GIVEN** a watch that fired an alarm at 08:45 and was stopped at 08:46
- **WHEN** it starts again at 08:50
- **THEN** that alarm does not fire again

#### Scenario: Beyond the horizon
- **GIVEN** an event starting in three days with a `-PT15M` alarm
- **WHEN** the watch starts
- **THEN** the firing is not queued, and it is once the horizon reaches it

#### Scenario: A store synced anew after an alarm fired
- **GIVEN** a pimdir watch that fired the 08:45 alarm of the event `standup`, item 1 of its store
- **WHEN** at 08:50 the store is synced anew, `standup` becoming item 2, and the watch reads it
- **THEN** that alarm does not fire again

### Requirement: A late firing fires within the grace window only
A firing reached late, the machine having slept or a change having come late, SHALL fire when it is at most the grace window late (15 minutes), and SHALL otherwise be dropped with an info log.

#### Scenario: A short suspend
- **GIVEN** a running watch holding an alarm firing at 08:45, the machine asleep from 08:40 to 08:55
- **WHEN** it wakes at 08:55
- **THEN** the hook fires for that alarm

#### Scenario: A night's suspend
- **GIVEN** a running watch holding an alarm firing at 02:00, the machine asleep from 23:00 to 07:00
- **WHEN** it wakes at 07:00
- **THEN** the hook does not fire for that alarm, and an info line names it

### Requirement: An edited calendar moves its alarms
A member the watch reports added or changed SHALL have its body read again and its firings replaced, and a member reported removed SHALL have its firings forgotten, before the next firing is waited for. Every member SHALL stay in the watch's picture whether it holds an alarm or not, so a member gaining one is seen.

#### Scenario: An edited event
- **GIVEN** a watch holding an event whose alarm fires at 08:45
- **WHEN** the event is moved an hour later on the server, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

#### Scenario: A deleted event
- **GIVEN** a watch holding an event with an alarm
- **WHEN** the event is deleted on the server, and the watch polls
- **THEN** its alarm does not fire

#### Scenario: An event gaining an alarm
- **GIVEN** a watch holding an event with no alarm
- **WHEN** a `-PT10M` alarm is added to it on the server, and the watch polls
- **THEN** the alarm fires ten minutes before the event

### Requirement: Bodies are read only when an alarm hook wants them
A CalDAV watch SHALL read member bodies only when its account configures `on-alarm-trigger`. It SHALL then read, once the baseline is taken, the members holding a `VALARM` through one `calendar-query` (RFC 4791 7.8) per component, a `VEVENT` and a `VTODO`, the comp-filters of one query all having to match (RFC 4791 9.7.1); a server refusing the filter SHALL be asked for every member instead. A server truncating a query (a 507 row, RFC 6578 §3.6) SHALL have the members of the watch's picture the queries did not name read through `calendar-multiget` (RFC 4791 7.9), 100 at a time, so no alarm is left out. After that it SHALL read the members a poll reports added or changed through `calendar-multiget` (RFC 4791 7.9). A member SHALL be keyed by the id io-webdav's `id_from_location` reads from its href, its last segment percent-decoded when the decoding encodes back to it, which is the id the query and the multiget give it, so a member whose href holds an escape is one member whichever report named it. A body that is not iCalendar SHALL be logged and skipped; a report that fails SHALL end the session, the next one reading every alarm again.

#### Scenario: An account with no alarm hook
- **GIVEN** a CalDAV account configuring `on-event-change` alone
- **WHEN** an event is edited
- **THEN** the hook fires, and no event body is read

#### Scenario: A server truncating the alarm query
- **GIVEN** a watch with an alarm hook, on a server answering a `calendar-query` with two members and a 507 row, and a calendar of three events holding an alarm
- **WHEN** the session opens
- **THEN** the third event is read by `calendar-multiget`, and its alarm fires as the other two do

#### Scenario: A member whose href holds an escape
- **GIVEN** a watch with an alarm hook, holding an event at `/cal/alice/work/Daily%20standup.ics` whose alarm fires at 08:45
- **WHEN** the event is moved an hour later on the server and the watch polls, then it is removed and the watch polls
- **THEN** the alarm fires at 09:45 and not at 08:45, then not at all

### Requirement: An alarm hook templates against its firing
`on-alarm-trigger` SHALL fill `$id`, `$collection` and `$calendar`, `$etag`, `$uid`, `$kind` (`event` or `task`), `$summary`, `$recurrenceId`, `$alarm` (the index of the `VALARM` among those of its component), `$alarmUid`, `$action` (`display` or `audio`), `$trigger` and `$at` (the start of the occurrence for an event, the due time for a task). Times SHALL be RFC 3339 with the system zone's offset, the `trigger` of the `alarm-fired` event aside (see the watch output). A variable the firing has no value for SHALL expand to nothing. The notification of an `AUDIO` alarm SHALL ask for the freedesktop sound `alarm-clock-elapsed`.

#### Scenario: A command given the firing
- **GIVEN** `caldav.hooks.on-alarm-trigger.command = 'echo "$uid $trigger" >> fired.log'`
- **WHEN** an alarm of an event whose `UID` is `standup` fires at 08:45 in Europe/Paris
- **THEN** fired.log ends with `standup 2026-11-05T08:45:00+01:00`

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.20 reads (pimdir storage-draft-17, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), and optionally `pimdir.account`, the account of the store the calendar is grouped under (pimdir STORAGE §9.2), unset reading the calendar whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL open the store again on every poll, every `pimdir.watch.poll.interval` (30 seconds by default), and read its change cursor (pimdir STORAGE §4.5): the items stamped above it are read again and the deleted or retained ones forgotten; a store of another id or incarnation, a purge count moved or the collection itself moved SHALL have the collection listed again, only the items new to the watch or holding another body read, the others kept and the missing ones forgotten. A store moved aside and synced anew under `pimdir.root` is so read on the next poll, and a store that poll cannot open SHALL end the session with its failure, the alarms already read firing on until a session reads the store again. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `account check` coded `calendar-not-found`, naming it; one holding no calendars SHALL fail them naming it. A store of another draft SHALL fail them coded `store-version`, in the wording every pimdir program shows (io-pimdir's `PimdirFormatRefusal`) naming carillon: for a store of an earlier draft, that its sync engine drains it and a new store is synced once it is moved aside; for a later one, that carillon is to be upgraded.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

#### Scenario: A store synced anew under a running watch
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store is moved aside, neverest syncs a new one under `pimdir.root` holding the event moved an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, read from the new store, and not at 08:45

#### Scenario: A store of an earlier draft
- **GIVEN** a `pimdir.root` holding a store of storage-draft-16, beside a second account watching a store of storage-draft-17
- **WHEN** `carillon --json watch` runs
- **THEN** the first account prints `session-lost` coded `store-version`, its error naming carillon and storage-draft-17 and saying to move the store aside and sync a new one, retried on its backoff, and the second account's alarms fire on time

#### Scenario: A purge elsewhere in the store
- **GIVEN** a pimdir watch over a calendar of 10,000 events
- **WHEN** the store's owner purges a retained message of another collection, and the watch polls
- **THEN** the calendar is listed again and no event body is read

### Requirement: A first run points at the sample
Running `carillon` with no command SHALL answer through pimalaya-cli's `clap::meet_bare_invocation`: under `--json` it SHALL fail coded `usage-invalid`, "A command is required, see --help", exit 2, its configuration unread; in text it SHALL print the help and exit 0 when a configuration exists, one that fails to parse included, its error left to the first command reading it, and SHALL otherwise meet the missing configuration as any command does, as himalaya's does: carillon writes no configuration, and an account is written by hand against the documented sample, config.sample.toml. A command that finds no configuration SHALL fail, exit 1, naming the path it looked at (the first `-c` path when given, else the default one) and the sample; a person at a terminal SHALL meet pimalaya-cli's welcome in its place, naming the same path and sample and exiting 1, as Himalaya and Comodoro do, a script or a `--json` caller the failure alone. A configuration that fails to parse SHALL be an error rather than read as absent.

#### Scenario: A newcomer
- **GIVEN** no configuration file
- **WHEN** `carillon watch` runs
- **THEN** it exits 1 naming the path it looked at and config.sample.toml, and writes nothing

#### Scenario: A bare run with no configuration
- **GIVEN** no configuration file, and stdout and stderr no terminal
- **WHEN** `carillon` runs with no command
- **THEN** it exits 1 naming the path it looked at and config.sample.toml, as `carillon watch` does, and prints the help once a configuration exists

### Requirement: A refused command line exits 2
The command line SHALL be parsed through pimalaya-cli's `clap::parse`: help and version print as clap prints them, and a command line the grammar refuses (an unknown flag or command, a missing argument, a bad value) SHALL exit 2 before any configuration is read. With a `--json` word before any `--`, it SHALL print on stdout the failure object coded `usage-invalid`, its `error` clap's message without its `error: ` prefix nor styles, `sources` empty and `backtrace` null, and nothing on stderr; otherwise clap's rendering on stderr and nothing on stdout. Exit 1 stays the failure object and 3 a printed result that is not a full success, an `account check` with a failed backend.

#### Scenario: An unknown flag under JSON
- **WHEN** `carillon --json watch --nope` runs
- **THEN** it prints one failure object coded `usage-invalid`, its `error` naming `--nope`, on stdout and exits 2

### Requirement: Help is plain text
`--help` and `-h` SHALL print without colour, each paragraph on one line, never wrapped to the terminal width, the top-level help ending with the Pimalaya footer (where to report bugs, how to sponsor). clap SHALL be built with himalaya's features, its defaults off: `derive`, `error-context`, `help`, `std`, `suggestions` and `usage`, neither `color` nor `wrap_help` (pimalaya-cli terminal.md, "Help is printed as written").

#### Scenario: A narrow terminal asking for colour
- **GIVEN** `COLUMNS=60` and `CLICOLOR_FORCE=1`
- **WHEN** `carillon --help` prints
- **THEN** the help of `-b` prints its second paragraph on one line, longer than 60 columns, and no escape sequence

### Requirement: A failure carries a stable code
A failure a caller is expected to act on SHALL carry a stable code, printed under `--json` as a `code` string beside `error`, `sources` and `backtrace`, and in an `account check` row beside `backend`, `ok` and `error`. The code SHALL be found anywhere in the error chain or read from the typed error of the protocol that refused or of the store that failed. A failure with no code SHALL print `"code": null`; the text output SHALL stay unchanged. Codes are kebab-case and never renamed; `error` stays free wording. Every code SHALL be declared in every build.

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli; nothing ran |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 whose body names no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV body naming `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon sends no request twice |
| `mailbox-not-found` | the mailbox a backend watches does not exist: an IMAP `NONEXISTENT` (RFC 5530 §3), a Maildir folder that is no directory, a JMAP, Gmail or Graph mailbox no listing names, Graph answering its direct read 404 or 400 `ErrorInvalidIdMalformed` |
| `calendar-not-found` | the calendar a backend watches does not exist: a DAV 404 on it, a DAV calendar id naming no collection, a JMAP, Google Calendar or Graph calendar not found as a mailbox is, a pimdir collection the store does not hold or holds under another account than `pimdir.account` |
| `addressbook-not-found` | the addressbook a backend watches does not exist: a DAV 404 on it, a DAV addressbook id naming no collection, a JMAP addressbook, Google People contact group or Graph contact folder not found as a mailbox is |
| `store-version` | the pimdir store is of another storage draft than the one carillon reads (storage-draft-17), its message naming carillon and which of the two is the earlier; neverest syncs a new store once the old one is moved aside |
| `store-uncreated` | the pimdir root holds no store yet; its sync engine creates it |
| `store-foreign` | the pimdir root holds a database another program wrote |
| `store-corrupt` | the pimdir store's version records disagree |
| `store-not-current` | the pimdir store holds no schema yet |
| `store-hash-algo` | the pimdir store names its bodies by a hash carillon does not know |
| `store-exposed` | another user can reach the pimdir store, or a link stands where its files belong |
| `sync-version` | another sync engine version wrote the pimdir store's sync state |
| `store-busy`, `store-owned`, `store-staging`, `store-in-flight`, `store-reset`, `lock-replaced`, `item-duplicate`, `flag-invalid`, `action-malformed`, `conflict-unread`, `unsupported`, `no-performer`, `ambiguous-performer`, `index-missing`, `index-outdated`, `index-version`, `index-zone-unknown`, `index-fts5-missing`, `query-refused`, `query-unanswerable`, `tag-refused`, `property-invalid` | the pimdir failure io-pimdir names so, relayed as himalaya relays it, so every pimdir app reads one vocabulary; a reader taking no lock and writing nothing meets none of them today |
| `account-missing` | no account carries the name `-a` gave, `default` included |
| `account-default-missing` | `-a` named no account and none is marked default |
| `account-default-several` | `-a` named no account and several are marked default |
| `backend-not-built` | a command reached a backend block the build leaves out |
| `capability-missing` | `-b` named a backend the account has no block for, as himalaya codes its `--backend` |

A DAV 403 naming another precondition is no matter of rights and SHALL carry no code. A watch's `session-lost` event SHALL carry the code of the failure that ended the session, `null` when it carries none.

#### Scenario: A store another draft wrote
- **GIVEN** an account whose `pimdir.root` holds a store of another pimdir draft
- **WHEN** `carillon --json account check` runs
- **THEN** the pimdir row reads `ok: false`, `code: "store-version"`, its error naming carillon, and the command exits 3

### Requirement: The standard commands are the family's
carillon SHALL offer `completion`, `manual` and `json-schema` in himalaya's order and under the abouts pimalaya-cli gives them, `completions`, `manuals` and `json-schemas` staying hidden aliases. Given no argument, each SHALL be refused by clap as pimalaya-cli requires its names or `--dir`: exit 2, clap's text on stderr naming `--dir`, under `--json` the failure object coded `usage-invalid`, a directory being what one page, script or schema per command or shell goes to. carillon SHALL offer no `repl` and no `mcp`: both hold a session of commands over one account, and carillon has nothing to call twice in one: `watch` already holds every account for as long as it runs and streams what it sees, and `account check` is run once before it.

#### Scenario: A man page directory left out
- **WHEN** `carillon manual` runs
- **THEN** it exits 2, clap's refusal on stderr naming `--dir`, and nothing reaches stdout

### Requirement: A default build runs anywhere
The default cargo features SHALL include `vendored`, as every family binary's do, which builds SQLite (rusqlite's `bundled`, through io-pimdir) and D-Bus from source: a default build SHALL link neither from the system, so the binary starts outside the shell it was built in.

#### Scenario: A default build out of its devshell
- **GIVEN** `cargo build` with the default features
- **WHEN** the binary runs where no libsqlite3 and no libdbus are installed
- **THEN** it starts

### Requirement: The backend is picked by `-b`
`-b/--backend` SHALL take `auto`, its default, or one backend by name: `imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople` or `pimdir`, every one of them in every build, a backend the build leaves out failing coded `backend-not-built` once a command reaches its block. `auto` SHALL let `watch` take the first block the account declares in that order and `account check` check every one; a name SHALL narrow both to that block. A name the account has no block for SHALL fail both coded `capability-missing`, "Account `<name>` has no `<backend>` block", as himalaya refuses its `--backend`, `watch` printing it as the account's `session-lost` and stopping that account for the run, as a backend the build leaves out. The help SHALL state the order and list every value as clap's possible values, and a value outside them SHALL be refused as a command line the grammar refuses, naming the possible values.

#### Scenario: A backend the account has no block for
- **GIVEN** an account `a` holding a `maildir` block alone
- **WHEN** `carillon --json -b imap account check` runs, then `carillon --json -b imap watch`
- **THEN** both fail coded `capability-missing` with "Account `a` has no `imap` block", exiting 1, the watch printing one `session-lost` event first and retrying nothing

#### Scenario: A backend no build knows
- **WHEN** `carillon -b nope watch` runs
- **THEN** it exits 2 before any configuration is read, its message naming `[possible values: auto, imap, jmap, maildir, caldav, carddav, msgraph, gmail, gcal, gpeople, pimdir]`
