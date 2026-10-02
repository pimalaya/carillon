---
cairn: delta
change: proxy-under-each-backend
---

## ADDED Requirements

### Requirement: A network backend reaches its server through a configurable proxy
An account SHALL accept a `proxy` table, and every backend opening a socket (`imap`, `jmap`, `caldav`, `carddav`) SHALL accept one of its own, overriding the account's. The table SHALL carry a `url` (`socks5://`, `socks5h://` or `http://`), an optional `username` and an optional `password` resolved as a secret; a password without a username SHALL be refused. A backend naming none SHALL inherit the account's when the file is loaded, and with neither the `all_proxy` and `https_proxy` environment variables SHALL be read, `no_proxy` and loopback bypassing them.

#### Scenario: One account behind a SOCKS proxy
- **GIVEN** an account with `proxy.url = "socks5h://127.0.0.1:9050"` and an `imap` block naming no proxy
- **WHEN** it is watched
- **THEN** the IMAP connection, the envelope resolver's and every reconnect go through the proxy

#### Scenario: A configuration naming no proxy
- **GIVEN** an account carrying no `proxy` key anywhere
- **WHEN** it is watched with `all_proxy` unset and `https_proxy` unset
- **THEN** it connects directly, and nothing is written back into a generated document

## MODIFIED Requirements

## REMOVED Requirements
