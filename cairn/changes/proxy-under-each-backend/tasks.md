---
cairn: tasks
change: proxy-under-each-backend
---

- [x] Add `ProxyConfig` and the account-level `proxy`
- [x] Add `proxy` to the IMAP, JMAP, CalDAV and CardDAV blocks, inherited from the account at load
- [x] Feed it to every connection a watch, a check and a reconnect open
- [x] Document the keys in the sample and the changelog
- [x] Build, clippy and test green on every feature combination
- [x] Fold the delta into the spec and log the change
