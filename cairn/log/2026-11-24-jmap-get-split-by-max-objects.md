---
cairn: log
change: the-live-night-findings
landed: 2026-11-24
---

# JMAP reads changed items in calls the server takes

## What landed

- src/jmap.rs: a round read every id one `/changes` answer named through a single `/get`, whatever the session's `maxObjectsInGet`. Live night 2 (2026-11-24), on the local Stalwart: a flag added over 5200 messages in one IMAP `STORE` made the watch's `Email/get` answer `requestTooLarge`, the session was lost and the reopened one took a new baseline, so none of the 5200 flag changes fired. `fetch` now splits the ids into calls of at most `maxObjectsInGet` and 500 (`get_limit`, the rule the baseline listing already followed), for mail, contacts and calendar events alike.
- Pinned by `jmap::tests::a_round_reads_more_changes_than_one_get_takes` (a scripted server declaring `maxObjectsInGet` 2), red before the change (`cannot resolve changed emails`).
- Checked live the same night: 5201 flag removals on Stalwart, which pages `Email/changes` at 5000 (`hasMoreChanges`), fired 5201 `flag-removed` in one round, no session lost.
- Capability moved: daemon (the JMAP requirement bounds each `/get` by `maxObjectsInGet`).
