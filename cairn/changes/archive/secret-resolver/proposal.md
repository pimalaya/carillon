---
cairn: change
id: secret-resolver
status: landed
created: 2026-08-29
---

# Resolve an account's credentials through one resolver

## Why

pimalaya-config 0.2.0 turns `Secret::Command` into a `CommandConfig`, a comparable value rather than a built `std::process::Command`, and adds `SecretResolver`, which spawns each distinct command once and hands its value to every field naming it.

carillon needs the first half to keep compiling. The second half pays for itself in one place: `carillon check` opens every backend `--backend` allows on an account, and an account whose CalDAV and CardDAV tables read the same `pass` or `gpg` entry unlocks that store once per backend. Two backends, two key unlocks, for one credential.

## What

- The wizard's secret module builds a `CommandConfig::Argv` or a `CommandConfig::Shell` instead of a `std::process::Command`. Both TOML shapes parse and serialize exactly as before, so no configuration moves.
- Every credential resolution takes a `&mut SecretResolver`: `SaslConfig::try_into_sasl`, `jmap::http_auth`, and the new `imap::open_with`, `jmap::open_with`, `dav::open_with` and `dav::auth_with`. The plain `open` and `auth` stay as the convenience an isolated call site uses, each building a resolver of its own.
- `check` builds one resolver for the account and passes it to every backend it opens.

The watch loop keeps resolving per connection attempt. Holding a resolver for a session that may last days would make it a long-lived plaintext cache, and would serve a stale token to the reconnect that a broker exists to refresh. That is the daemon's existing rule, not an omission.
