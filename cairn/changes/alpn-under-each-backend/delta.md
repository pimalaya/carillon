---
cairn: delta
change: alpn-under-each-backend
---

## ADDED Requirements

### Requirement: The TLS handshake is configured under its backend
Every backend speaking TLS SHALL carry a `tls` table and an `alpn` key of its own, and the runtime TLS handle SHALL be built through one conversion taking that list, so a connection cannot be opened without saying what it negotiates. `alpn` SHALL be a list of ALPN identifiers: unset SHALL take the default the backend's client crate owns (`["imap"]` over IMAP, `["http/1.1"]` over JMAP and WebDAV), an empty list SHALL skip ALPN negotiation, and a non-empty list SHALL replace the default. Only rustls SHALL read it, native-tls having no ALPN. Every path in a `tls` table SHALL be expanded when the file is read, so a leading tilde or a shell variable names the same thing there as everywhere else.

#### Scenario: A server that refuses the handshake carrying an ALPN
- **GIVEN** a CalDAV server whose TLS terminator rejects a `ClientHello` offering `http/1.1`
- **WHEN** the account sets `caldav.alpn = []`
- **THEN** the connection is opened with no ALPN extension at all

#### Scenario: A configuration naming no ALPN
- **GIVEN** an account carrying no `alpn` key
- **WHEN** it is watched
- **THEN** the backend offers the default its client crate owns, unchanged from before the key existed, and nothing is written back into a generated document

#### Scenario: A certificate under the home directory
- **GIVEN** `imap.tls.cert = "~/certs/example.pem"`
- **WHEN** the configuration is read
- **THEN** the path is expanded against the home directory rather than read as a relative `./~/certs/example.pem`

## MODIFIED Requirements

## REMOVED Requirements
