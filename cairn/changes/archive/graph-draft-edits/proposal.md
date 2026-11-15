---
cairn: change
id: graph-draft-edits
status: landed
created: 2026-10-05
---

# Report a Microsoft Graph draft edited in place

## Why

A Graph draft keeps its id when its subject, body, recipients or attachments are edited (issue #17). The mail delta selected only the flags, so such an edit fired nothing, and an assistant reconciling drafts had no hint to read one back.

## What

The Graph mail delta selects `changeKey` (io-msgraph 0.4.7) and keeps it as the message's version. A moved version with no flag moved fires `msgraph.hook.on-message-changed`; a flag moves the version too, so an edit coinciding with a flag change reads as the flag change alone. JMAP refuses the hook, its emails being immutable.
