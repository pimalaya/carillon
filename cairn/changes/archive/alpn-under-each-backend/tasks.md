---
cairn: tasks
change: alpn-under-each-backend
---

- [x] Replace `From<TlsConfig> for Tls` with `TlsConfig::into_tls`
- [x] Add `alpn` to the IMAP, JMAP, CalDAV and CardDAV blocks
- [x] Feed it through `into_tls` at every call site, the wizard included
- [x] Expand `tls.cert` at deserialize
- [x] Document the keys in the sample and cover them with round-trip tests
- [x] Fold the delta into the spec and log the change
