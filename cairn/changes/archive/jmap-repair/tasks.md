---
cairn: tasks
change: jmap-repair
---

- [x] Give the JMAP EventSource its own connection, so a closing subscription leaves the API connection alone
- [x] Reconnect and retry a round once, for the idle connection a polling watch finds closed
- [x] Resolve a JMAP arrival from the `Email/get` the round already makes, when an arrival hook wants one
- [x] Report a change and what the backend already knows about it, in one callback shape for every backend
- [x] Retire the untyped `dav` backend, its hooks, its events and the `Item` domain
- [x] Build, clippy and fmt green on every feature combination
- [~] Verify against a live JMAP server: not done, see the log entry
- [x] Fold the delta into the spec and log the change
