---
cairn: tasks
change: one-collection-one-method
---

- [x] Replace `mailbox` with a required `collection`, drop `-m/--mailbox`
- [x] Split the DAV server URL from the collection path
- [x] Add the `watch` method, refused when the backend cannot honour it
- [x] Push JMAP over EventSource, keeping the poll as a method
- [x] Add the polling watch to io-imap and select it from here
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify the new shape end to end against a Maildir
- [x] Fold the delta into the spec and log the change
- [ ] Verify the IMAP, JMAP and WebDAV methods against live servers
