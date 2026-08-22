---
cairn: delta
change: wizard
---

## ADDED Requirements

### Requirement: A first account is generated, never hand-written from scratch
The daemon SHALL offer to generate an account when it finds no configuration, and SHALL expose the same generator as `carillon configure`. The offer SHALL introduce carillon, name the configuration file that is missing, and point at the documented sample for everything the generator does not cover. It SHALL be raised only where nothing can happen without a configuration: a bare invocation, and a command that needs an account. A non-interactive caller (no terminal, or JSON output) SHALL never be prompted, and SHALL fail naming the file and the command that would create it.

The generator SHALL take one input, an email address, a bare domain, a `scheme://` server URL or a local folder path, and derive everything else. It SHALL discover the services reachable from that input and offer one entry per service, prompt the authentication method among those the chosen service advertises, and collect the credential through the shared picker so a secret is read from a keyring or a token broker rather than stored in the file. It SHALL NOT prompt for the watch method: the account SHALL take the best method its backend has, and SHALL write one only when the server cannot serve it. It SHALL test the connection before anything is written, and a failed test SHALL stop the wizard rather than yield an account that cannot connect.

The generated account SHALL be saved to the configuration file, appended to the one already there, or printed on stdout, at the user's choice; a redirected stdout or JSON output SHALL print it and touch no file. An appended account SHALL leave every comment and hand-written line of the existing file untouched, SHALL take a name no other account holds, and SHALL claim the default only when no other account does.

#### Scenario: A newcomer with an email address
- **GIVEN** no configuration file, and a provider publishing its settings
- **WHEN** `carillon` runs with no command and the offer is accepted
- **THEN** the address is discovered, one service is chosen, its credential is prompted, the connection is tested, and an account watching that service is written where the loader reads it

#### Scenario: A calendar server
- **GIVEN** a CalDAV service chosen from discovery
- **WHEN** the credential is accepted
- **THEN** the calendars of that account are listed from the home-set, the chosen one becomes `caldav.calendar`, and the account fires hooks only for the components that calendar advertises

#### Scenario: A server whose IDLE is not advertised
- **GIVEN** an IMAP server that does not advertise IDLE
- **WHEN** the connection is tested
- **THEN** the generated account carries an explicit `imap.watch.poll.interval`, and nothing was asked about it

#### Scenario: Nothing is discovered
- **GIVEN** an input no mechanism resolves
- **WHEN** the search comes back empty
- **THEN** the wizard stops and points at the documented sample, rather than prompting for a hand-entered configuration
