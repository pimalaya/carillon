---
cairn: tasks
change: collection-under-backend
---

- [x] Give each backend the collection it watches, required, under its own domain's name
- [x] Drop the account-level `collection` and its `mailbox` alias
- [x] Template against the same name the backend configures, and refuse another backend's word
- [x] Correct the himalaya compatibility claim wherever it is made
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify against a live server that a mail hook expands `$mailbox`
- [x] Fold the delta into the spec and log the change
