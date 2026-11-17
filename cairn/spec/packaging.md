---
cairn: spec
capability: packaging
status: current
---

# Packaging

How carillon is built: what its dependency tree carries and what it leaves out.

### Requirement: Hosts are ASCII
carillon SHALL build url's IDNA without a Unicode back end, `idna_adapter` being pinned to 1.0.0 in Cargo.toml, so no ICU4X crate reaches the tree. A host SHALL be written in ASCII, an internationalised one in its Punycode form (`xn--…`), and an `http` or `https` URL with a non-ASCII host (a CalDAV or CardDAV `server` or `home`, a JMAP server, an HTTP proxy) SHALL be refused when parsed: a DAV `home` when the configuration loads, a DAV or JMAP `server` when the account connects. A DAV `home`, a DAV `server` and a JMAP `server` SHALL be refused naming the host and the way out, "Host `bücher.example` is not ASCII: write it in its Punycode form (xn--...)". IMAP servers are URLs of a scheme url keeps opaque, which never reaches IDNA: their host is written in ASCII as well. The Microsoft Graph, Gmail, Google Calendar and Google People hosts are fixed ASCII names.

#### Scenario: An internationalised CalDAV home
- **GIVEN** `caldav.home = "https://bücher.example/dav/calendars/alice/"`
- **WHEN** the configuration loads
- **THEN** it is refused naming `bücher.example` and its Punycode form, and the same account loads with `caldav.home = "https://xn--bcher-kva.example/dav/calendars/alice/"`
