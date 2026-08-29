---
cairn: log
change: secret-resolver
landed: 2026-08-29
---

# Resolved an account's credentials through one resolver

pimalaya-config 0.2.0 replaced the `std::process::Command` inside `Secret::Command` with a `CommandConfig`, and added the resolver that becoming a comparable value made possible.

## What landed

The wizard's secret module builds a `CommandConfig::Argv` for what a keyring provider or token broker yields and a `CommandConfig::Shell` for a hand-typed line. Both write back the TOML shape they were read as, an array and a string, so no configuration file and no sample moves. The empty-argv and blank-line rejections are unchanged.

Every credential resolution now takes a `&mut SecretResolver`: `SaslConfig::try_into_sasl`, `jmap::http_auth`, and the new `imap::open_with`, `jmap::open_with`, `dav::open_with` and `dav::auth_with`. The plain `imap::open`, `jmap::open`, `dav::open` and `dav::auth` remain for the call sites that open one backend on their own, the watch loop and the wizard's connection tests, each building a resolver of its own.

`carillon check` builds one resolver for the account and hands it to every backend it opens. That is the case the resolver exists for here: `check` opens every backend `--backend` allows, and an account whose CalDAV and CardDAV tables read the same `pass` entry used to unlock its store once per backend.

## What is still true

The watch loop resolves per connection attempt, as the connection-lifecycle requirement says it must. A resolver held for a session that may run for days would be a long-lived plaintext cache, and would hand a reconnect the stale token a broker exists to refresh. The IMAP envelope resolver, which opens a second connection of its own, resolves on its own for the same reason.

The hook payload is untouched. It reaches the `command` serde adapter, which still yields a runnable `std::process::Command`, and 0.2.0 did not change it.

## Note for verification

`cargo check --all-features` cannot build in this tree: the uncommitted `[patch.crates-io]` entry points io-webdav at git, whose `WebdavSyncCollection::new` has grown an options argument `dav.rs` does not pass yet. That breakage predates this change and belongs to the io-webdav migration. Against the released io-webdav 0.2.1, every feature builds, and the whole test suite passes.
