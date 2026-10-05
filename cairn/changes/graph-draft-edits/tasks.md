---
cairn: tasks
change: graph-draft-edits
---

- [x] Select `changeKey` in the Graph mail delta, keep it as the version
- [x] Report an edit for a message whose version moved and no flag did
- [x] Add `on-message-changed` to the JMAP/Graph hook table, refused under JMAP
- [x] Unit-test the picture and the refusal, extend the live Graph mail test
- [ ] Run the live Graph mail test, and check whether an attachment change moves `changeKey`
- [x] Document the sample, fold the delta, log the change
