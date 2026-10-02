---
cairn: tasks
change: jmap-domains
---

- [x] Take a collection per domain under `jmap`, at least one required
- [x] Add the card and event hooks to the JMAP table, with no `on-task-*`
- [x] Refuse a hook whose domain the account configured no collection for, at load
- [x] Subscribe the event stream to the types the account configured
- [x] Ask each configured domain what moved, and reconcile each against its own picture
- [x] Template the collection from the event's domain rather than the backend
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify against a live JMAP server that a contact and an event fire their hooks
- [x] Fold the delta into the spec and log the change
