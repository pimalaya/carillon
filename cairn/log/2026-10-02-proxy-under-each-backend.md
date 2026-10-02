---
cairn: log
change: proxy-under-each-backend
landed: 2026-10-02
---

# Let an account reach its servers through a proxy

## What landed

The account gained a `proxy` table and `imap`, `jmap`, `caldav` and `carddav` one each, the shape cardamum and himalaya carry: `url`, `username`, and `password` as a secret. The account one is handed down to the backends naming none when the file is loaded, so `imap::open`, `jmap::open` and `dav::open` each read their own backend's key, and with it the envelope resolver and every reconnect, which go through those. Unset everywhere, `Proxy::System` reads the environment, the default io-imap 0.7, io-jmap 0.4 and io-webdav 0.5 adopted.

`tls.cert` now deserializes through `pimalaya_config::toml::opt_shell_expanded_path`, added to pimalaya-config for the purpose, retiring carillon's private copy and its TODO.

## What is still true

A configuration naming no proxy loads and renders unchanged. The wizard writes none, appending text rather than re-serializing, so an inherited proxy is never written into a backend block. The DAV wizard's connection test reads only the environment, having no account yet.

## Note for verification

The `opt_shell_expanded_path` helper is unreleased: Cargo.toml patches pimalaya-config to `../config` until it ships. Build, clippy and the suite are green on every feature combination against that patch; nothing was verified against a live proxy.
