---
cairn: change
id: proxy-under-each-backend
status: landed
created: 2026-10-02
---

# Let an account reach its servers through a proxy

## Why

io-imap 0.7, io-jmap 0.4 and io-webdav 0.5 take a proxy at connect time, defaulting to the one the environment names (`all_proxy`, then `https_proxy`, `no_proxy` bypassing). Bumping to them made carillon read those variables with nothing in the configuration to override them, while himalaya and cardamum, reading the same account shape, gained a `proxy` table in the same round.

## What

- `AccountConfig` gains `proxy`, a SOCKS5 or HTTP proxy every network backend of the account goes through, and `imap`, `jmap`, `caldav` and `carddav` each gain `proxy` overriding it. Maildir gains none: it opens no socket.
- The table is cardamum's: `url` (`socks5://`, `socks5h://` or `http://`), `username`, and `password` as a secret resolved like any credential. A password without a username is refused.
- Neither set, the environment is read, as the client crates now do by default.
- The account proxy is handed down to the backends naming none when the file is loaded, so every connection site reads its own backend's key.
