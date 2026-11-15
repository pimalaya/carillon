---
cairn: tasks
change: vendor-backends
---

- [x] Add the `msgraph`, `gmail`, `gcal` and `gpeople` features, blocks and hook tables, refusing at load what each cannot fire
- [x] Watch Graph mail and contacts through delta queries, and the calendar through a `changeKey` listing diff
- [x] Watch a Gmail label through the history poll, resolving the envelope of an arrival
- [x] Watch a Google calendar and a contact group through their sync tokens, re-baselining on expiry
- [x] Teach `-b/--backend` and `carillon check` the four backends
- [x] Route Google and Microsoft addresses to them in the wizard, the token collected through the picker
- [x] Document the blocks in the sample, the README and the changelog
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify against a live Microsoft 365 tenant and a live Google Workspace account that each domain fires its hooks
- [x] Fold the delta into the spec and log the change
