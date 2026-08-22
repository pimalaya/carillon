---
cairn: tasks
change: absorb-core
---

- [x] Move imap, backend, credential, event and error into the daemon crate
- [x] Trim the transport class, the CardDAV source and the server-only parts of the event
- [x] Drop the carillon-core dependency and add thiserror
- [x] Drop the fuzz target and the hostile corpus test, which re-covered imap-codec
- [x] Build, test, clippy and fmt green
- [x] Fold the delta into the daemon spec and log the change
