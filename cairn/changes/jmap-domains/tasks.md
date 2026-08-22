---
cairn: tasks
change: jmap-domains
---

- [ ] Take a collection per domain under `jmap`, at least one required
- [ ] Add the card and event hooks to the JMAP table, with no `on-task-*`
- [ ] Refuse a hook whose domain the account configured no collection for, at load
- [ ] Subscribe the event stream to the types the account configured
- [ ] Ask each configured domain what moved, and reconcile each against its own picture
- [ ] Template the collection from the event's domain rather than the backend
- [ ] Build, clippy and fmt green on every feature combination
- [ ] Verify against a live JMAP server that a contact and an event fire their hooks
- [ ] Fold the delta into the spec and log the change
