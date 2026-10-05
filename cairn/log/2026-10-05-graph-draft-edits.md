---
cairn: log
change: graph-draft-edits
landed: 2026-10-05
---

# Reported a Microsoft Graph draft edited in place

## What landed

The Graph mail delta selects `changeKey`, kept as each message's version. A message whose version moved while no flag did fires `msgraph.hook.on-message-changed`; one whose flags moved fires the flag hooks alone. `jmap.hook.on-message-changed` is refused when the configuration is read.

## Capabilities moved

- daemon: mail reports an edit for a Graph draft; `on-message-changed` joins the mail hooks, Graph only.

## Note for verification

Unit tests cover the picture and the JMAP refusal. The live Graph mail test now edits the draft's subject and waits for `on-message-changed`; it has not been run yet. Whether an attachment change moves `changeKey` is unverified.
