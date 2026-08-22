---
cairn: tasks
change: dav-backend
---

- [x] Add the fifth event and rename the hooks, keeping the old names as aliases
- [x] Add the dav backend, its config block and its cargo feature
- [x] Handle truncation and a rejected sync token
- [x] Wire it into the backend selector, the driver and `check`
- [x] Build, clippy and fmt green on every feature combination
- [x] Fold the delta into the spec and log the change
- [ ] Verify against a live CalDAV or CardDAV server
