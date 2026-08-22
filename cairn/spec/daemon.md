---
cairn: spec
capability: daemon
status: current
---

# The watch daemon

carillon watches PIM accounts and fires local hooks on every change. It reads a TOML config of named accounts, watches each one on its own thread, and runs on one machine with no server apparatus: no HTTP listener, datastore, auth, custody, metering or billing.

It watches; it never syncs. A change is reported as it is seen (an item arrived, one left, one was edited, a flag moved) and nothing is stored between runs. What a hook wants beyond that, carillon goes and reads on demand.

Each backend brings its own way of learning about a change, and the daemon translates all of them into one vocabulary, so one hook runner serves them all: IMAP holds IDLE and reports UID-keyed deltas, JMAP polls `Email/changes`, Maildir re-lists the mailbox, and the DAV backends report what a collection did since a sync token. Mail is not the boundary: CalDAV holds events and tasks and CardDAV holds cards, and each is configured and named for what it holds rather than reduced to a word true of everything. The protocol crates own the protocols (io-imap, io-jmap, io-maildir, io-webdav); this repository owns the config, the hooks and the supervision.

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from an explicit path then the standard user paths. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. The account block SHALL keep the shape himalaya CLI and himalaya TUI read, and unknown keys SHALL be ignored there rather than refused, so an account can be recognised across the binaries. A whole file SHALL NOT be claimed to load in all three, since every backend block is strict on both sides and each has keys the other does not know.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hook.on-message-added` notify hook
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
An account SHALL watch the one collection its backend names, and MAY name the one method it watches with. Neither SHALL be overridable from the command line: what an account watches is its configuration, and watching a second collection is a second account, which is also how it gets its own hooks. Every backend SHALL read its collection the same way, the DAV ones included, whose `server` names the DAV root and whose collection is the path under it.

#### Scenario: A second collection
- **GIVEN** an account watching one mailbox
- **WHEN** a second mailbox is to be watched
- **THEN** it is a second account, with its own hooks, and no flag exists to ask for it

### Requirement: The watch method belongs to the backend
The method SHALL be configured under its backend (`imap.watch`, `jmap.watch`, `maildir.watch`, and `watch` under each of `caldav` and `carddav`) and named by its mechanism, the way a SASL mechanism and an HTTP auth scheme already are. Each backend SHALL declare only the methods it has, so a method it does not have is refused when the configuration is read rather than when the watch runs. Unset, an account SHALL watch the best way its backend has: IDLE for IMAP, a held event stream for JMAP, a poll for the backends with nothing else. Every backend SHALL offer the poll, whose interval MAY be given and otherwise takes what suits that backend.

#### Scenario: A server whose IDLE cannot be trusted
- **GIVEN** an IMAP account whose server accepts IDLE and then never speaks
- **WHEN** the account configures `imap.watch.poll.interval`
- **THEN** the watch re-reads the mailbox on that interval instead, reporting the same events

#### Scenario: A method the backend does not have
- **GIVEN** a Maildir account configuring `maildir.watch.idle`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the methods that backend has, and no watch is started

### Requirement: The daemon owns the connection lifecycle
The daemon SHALL own reconnection: a session that ends, for any reason other than a requested shutdown, SHALL be reopened after a capped exponential backoff, and a session that stayed up long enough to look healthy SHALL reset that backoff. Credentials SHALL be resolved per attempt rather than held, so a rotated secret is picked up by the next reconnect and residency stays minimal.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the session ends because the connection dropped
- **THEN** the daemon waits its backoff, resolves the credential again, and reopens the watch

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, a flag added, a flag removed, each carrying the domain of what it is about. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a filter written once (`flags = ["Seen"]`) fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit, and a WebDAV poll reads etags, so the flags of an item are unknown to it rather than empty, and it reports none. Unknown and empty are distinct, as they are in a pimdir store. The vocabulary SHALL stay one across the backends even though the hooks configuring it are per backend and per domain, so that one hook runner serves all of them.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flag-added` with the flag named `Seen`

#### Scenario: An item that is edited where it stands
- **GIVEN** a CardDAV account watching an addressbook
- **WHEN** a contact is edited and its etag moves
- **THEN** `carddav.hook.on-card-changed` fires, an event no mail backend accepts a hook for

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection by polling an RFC 6578 `sync-collection` report, under whichever of `caldav` and `carddav` names the domain it holds, both sharing one server, authentication and poll shape. It SHALL request `getetag`, and the content type only where a mixed calendar needs it, so a poll never carries a contact or an event; it SHALL keep an href to etag and domain picture of the collection, so that a member it has never seen reads as an arrival, a known member whose etag moved reads as an edit, and a member that vanished is still reported under the domain it had. A truncated report SHALL be drained immediately rather than at the next interval, and a sync token the server rejects SHALL cause a re-enumeration, which reports nothing because a re-baseline is not news. No backend SHALL watch a collection holding neither calendars nor contacts: the domains that exist have their own backend, and a collection naming none of them has no hook worth firing.

#### Scenario: A contact is edited
- **GIVEN** a CardDAV account watching an addressbook it has already enumerated
- **WHEN** a contact is edited and its etag moves
- **THEN** `on-card-changed` fires for that href, and no vCard is read

#### Scenario: The server forgets its history
- **GIVEN** a watch holding a sync token the server no longer honours
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, no event is fired for what was already there, and the watch continues from the fresh token

### Requirement: Arrivals are resolved only when a hook wants them
A watch learns that an item arrived, and sometimes what it says. A backend SHALL report an arrival together with the summary it already read, and SHALL read one it does not have only when the active backend configures the arrival hook of that domain. JMAP SHALL take its summary from the `Email/get` its round already makes, asking for the envelope properties only when a hook wants them. IMAP SHALL read one on a second connection, never the one holding the watch, an IMAP delta naming a UID and nothing more. A backend that can read no envelope SHALL leave the summary empty, and a resolution failure SHALL degrade to an unresolved event rather than ending the watch.

#### Scenario: An account with no arrival hook
- **GIVEN** an account whose only hook is `imap.hook.on-flag-added`
- **WHEN** a message arrives
- **THEN** no envelope is fetched and no second connection is opened

#### Scenario: A JMAP arrival with an envelope
- **GIVEN** a JMAP account whose `jmap.hook.on-message-added` notification names `$subject` and `$sender`
- **WHEN** a message arrives
- **THEN** both are filled from the round's own `Email/get`, with no second request

### Requirement: Ctrl+C is prompt on every path
A requested shutdown SHALL be honoured within roughly a second on every path a watch can be waiting in: idling on a connection, sleeping between polls, backing off before a reconnect, or resolving an arrival's envelope. No path SHALL wait on a server that has stopped answering: every connection the daemon opens SHALL carry a read deadline and SHALL hand back the not-ready failures rather than letting the transport retry them away, since the deadline exists to be the wakeup that re-reads the flag.

#### Scenario: Ctrl+C while resolving against a silent server
- **GIVEN** a watch resolving an arrival's envelope against a server that has stopped answering
- **WHEN** the user presses Ctrl+C
- **THEN** the read deadline expires, the flag is seen, and the watch ends rather than waiting for the transport's own timeout

### Requirement: A hook failure never stops the watch
A hook SHALL be a desktop notification, a shell command, or both. Its templates SHALL expand the event's variables, and the command SHALL receive the same variables in its environment. A hook that fails SHALL be logged and left behind: neither a missing notification daemon nor a broken script SHALL end the watch. A hook SHALL NOT half-fire for a reason the configuration could have been refused for: what a template may name is settled when the file is read, so the only failures left at watch time are the ones the machine around it produced.

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

### Requirement: The hooks belong to the backend
The hooks SHALL be configured under their backend (`imap.hook`, `jmap.hook`, `maildir.hook`, `caldav.hook`, `carddav.hook`, `dav.hook`), singular, with `hooks` accepted as an alias. Each backend SHALL declare only the events it reports, so a hook it cannot fire is refused when the configuration is read rather than never firing. The variables a hook templates against SHALL be the ones its backend can fill, which is why the envelope names belong to the IMAP table alone. An account declaring more than one backend SHALL configure the hooks of each, since a hook written for one backend says nothing about what another would report.

#### Scenario: A hook the backend cannot fire
- **GIVEN** an account configuring `carddav.hook.on-flag-added`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the line and the events CardDAV reports, and no watch is started

#### Scenario: Two backends on one account
- **GIVEN** an account declaring both `imap` and `maildir`, with an `imap.hook.on-message-added` summary reading `New mail from $sender`
- **WHEN** the account is watched with `-b maildir`
- **THEN** no hook fires from the IMAP table, and the Maildir table is what the watch reads

### Requirement: An event is named after its domain
A hook SHALL be named after what it carries. Mail SHALL be `on-message-added` and `on-message-removed`, whichever of IMAP, JMAP and Maildir reports it. A CardDAV addressbook SHALL be `on-card-added`, `on-card-removed` and `on-card-changed`, and a CalDAV calendar the same three under `on-event-` and `on-task-`. A backend SHALL take only the domains it holds, so the domain a hook names is checked when the configuration is read rather than assumed while the watch runs. The domain SHALL be carried by the event itself, so that one hook runner still serves every backend.

#### Scenario: A calendar hook on an addressbook
- **GIVEN** an account configuring `carddav.hook.on-event-added`
- **WHEN** the configuration is read
- **THEN** it is refused, and the account is pointed at `on-card-added`

### Requirement: A flag hook fires once per flag
`on-flag-added` and `on-flag-removed` SHALL fire once for each flag that moved rather than once for the delta, so `$flag` SHALL always name the flag the firing is about and no plural variable SHALL be exposed. The optional `flags = [...]` filter SHALL narrow which of those firings happen, matching one flag at a time, with or without a leading `\` or `$` and without regard to case. A backend with no flags SHALL take no flag hook at all.

#### Scenario: Two flags set at once
- **GIVEN** an IMAP account with an unfiltered `imap.hook.on-flag-added` command
- **WHEN** one STORE sets both `\Seen` and `\Flagged`
- **THEN** the command runs twice, once with `$flag` as `Seen` and once as `Flagged`

#### Scenario: A filtered flag hook
- **GIVEN** the same account filtering `flags = ["Seen"]`
- **WHEN** the same STORE sets both flags
- **THEN** the command runs once, with `$flag` as `Seen`

### Requirement: A CalDAV calendar knows its components
A CalDAV watch SHALL resolve what its collection holds from `supported-calendar-component-set` when the watch starts. A calendar advertising a single component SHALL report every member as that component, at no cost per member. A calendar advertising several SHALL read `getcontenttype` on a member it has not seen and route it by the `component` parameter RFC 4791 §10.1 allows, falling back to the components the calendar advertises when the server sends no parameter. A member SHALL never be fetched to find out what it is, since a poll carrying a VEVENT is what asking for etags alone exists to avoid; reading a property is not fetching a member. The domain of each member SHALL be remembered beside its etag, since a removal leaves only an href behind.

#### Scenario: A task is deleted from a calendar holding both
- **GIVEN** a CalDAV account watching a calendar of events and tasks, with `on-task-removed` configured
- **WHEN** a VTODO is deleted
- **THEN** `on-task-removed` fires, the domain coming from what the watch remembered of that href, and `on-event-removed` does not

#### Scenario: A calendar that holds one component
- **GIVEN** a CalDAV account watching a calendar advertising `VEVENT` alone
- **WHEN** a member is added
- **THEN** `on-event-added` fires without any further request, and the account's task hooks are refused when the configuration is read

### Requirement: A hook templates against what its event carries
Each hook SHALL declare the variables it can fill, and a notification naming anything else SHALL be refused when the configuration is read. `$id` SHALL be available to every hook, the collection SHALL be available under the name its backend configures it as, `$flag` to a flag hook, and the envelope names only to the arrival hook of a backend that resolves one, which is IMAP alone. A `${name:default}` SHALL keep working whatever the name, a default being how a template says it can do without the value. A command SHALL NOT be validated, its placeholders reaching it as environment variables where an unset name is ordinary.

#### Scenario: A removal that asks for an envelope
- **GIVEN** an account whose `imap.hook.on-message-removed` notification body reads `$subject`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hook and the variables it may use, since an expunged message has no envelope to read

#### Scenario: A variable that is legitimate but absent
- **GIVEN** an account whose `imap.hook.on-message-added` notification summary reads `New mail from $sender`
- **WHEN** a message arrives whose envelope carries no sender, or whose resolution failed
- **THEN** the notification fires with that part empty, rather than being dropped

### Requirement: The collection belongs to the backend, under its own name
Each backend SHALL take the one collection it watches, required, under the name its domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar` and `carddav.addressbook`. No account-level key SHALL name it, so an account block carries nothing that needs a backend to be understood. A hook SHALL template against the same name its backend configures, `$id` being the one variable every backend means the same way.

#### Scenario: A mail hook naming its mailbox
- **GIVEN** an account whose `imap.hook.on-message-added` summary reads `New mail in $mailbox`
- **WHEN** a message arrives
- **THEN** the notification names the mailbox from `imap.mailbox`

#### Scenario: A hook naming another backend's word
- **GIVEN** an account whose `caldav.hook.on-event-added` summary reads `$mailbox`
- **WHEN** the configuration is read
- **THEN** it is refused, since a calendar is configured and templated as `$calendar`

### Requirement: A watch survives the change it reports
A watch SHALL keep working across the changes it reports. A connection a backend's own protocol closes as part of reporting SHALL be reopened before the next request rather than written into, and a round that fails SHALL be retried once on a fresh connection before the session is given up. A round SHALL advance no state it did not complete, so running it twice reports nothing twice.

#### Scenario: A JMAP event stream that closes after its state change
- **GIVEN** a JMAP account watching over the event stream, which the server closes after reporting one state change
- **WHEN** a message arrives
- **THEN** the round that follows runs on a fresh connection and fires the hook, rather than losing the session and re-baselining the change away

#### Scenario: A polling watch whose idle connection was closed
- **GIVEN** a JMAP account polling on an interval, whose server closed the connection while it slept
- **WHEN** the next round runs
- **THEN** it reconnects and runs again, and the session is given up only if that also fails


### Requirement: A first account is generated, never hand-written from scratch
The daemon SHALL offer to generate an account when it finds no configuration, and SHALL expose the same generator as `carillon configure`. The offer SHALL introduce carillon, name the configuration file that is missing, and point at the documented sample for everything the generator does not cover. It SHALL be raised only where nothing can happen without a configuration: a bare invocation, and a command that needs an account. A non-interactive caller (no terminal, or JSON output) SHALL never be prompted, and SHALL fail naming the file and the command that would create it.

The generator SHALL take one input, an email address, a bare domain, a `scheme://` server URL or a local folder path, and derive everything else. It SHALL discover the services reachable from that input and offer one entry per service, then prompt the authentication method the chosen service accepts, and collect the credential through the shared picker so a secret is read from a keyring or a token broker rather than stored in the file. The method SHALL be offered on the server's own word wherever the protocol has one to read, discovery reporting whether a provider takes a password or a token and never which mechanism carries it; what the server does not answer SHALL fall back to what discovery reported, so an unreachable probe narrows nothing rather than emptying the menu. A method the configuration cannot express SHALL NOT be offered. It SHALL NOT prompt for the watch method: the account SHALL take the best method its backend has, and SHALL write one only when the server cannot serve it. It SHALL test the connection before anything is written, and a failed test SHALL stop the wizard rather than yield an account that cannot connect.

The generated account SHALL be saved to the configuration file, appended to the one already there, or printed on stdout, at the user's choice; a redirected stdout or JSON output SHALL print it and touch no file. An appended account SHALL leave every comment and hand-written line of the existing file untouched, SHALL take a name no other account holds, and SHALL claim the default only when no other account does.

#### Scenario: A newcomer with an email address
- **GIVEN** no configuration file, and a provider publishing its settings
- **WHEN** `carillon` runs with no command and the offer is accepted
- **THEN** the address is discovered, one service is chosen, its credential is prompted, the connection is tested, and an account watching that service is written where the loader reads it

#### Scenario: A calendar server
- **GIVEN** a CalDAV service chosen from discovery
- **WHEN** the credential is accepted
- **THEN** the calendars of that account are listed from the home-set, the chosen one becomes `caldav.calendar`, and the account fires hooks only for the components that calendar advertises

#### Scenario: A provider whose policy is wider than its server
- **GIVEN** a Gmail address, whose discovered policy names a password and an OAuth grant, on an IMAP server advertising PLAIN, XOAUTH2, OAUTHBEARER and LOGIN
- **WHEN** the mechanism is prompted
- **THEN** the menu holds what the server advertised, and SCRAM-SHA-256, which Gmail does not implement, is not in it

#### Scenario: A server whose IDLE is not advertised
- **GIVEN** an IMAP server that does not advertise IDLE
- **WHEN** the connection is tested
- **THEN** the generated account carries an explicit `imap.watch.poll.interval`, and nothing was asked about it

#### Scenario: Nothing is discovered
- **GIVEN** an input no mechanism resolves
- **WHEN** the search comes back empty
- **THEN** the wizard stops and points at the documented sample, rather than prompting for a hand-entered configuration
