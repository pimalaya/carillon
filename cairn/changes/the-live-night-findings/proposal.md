---
cairn: change
id: the-live-night-findings
status: active
created: 2026-11-24
---

# The live night findings

The live test night of 2026-11-23 (workspace .lanes/live/report.md) watched every carillon backend and fired every hook once. 9cebdda fixed IMAP flags reported as sent rather than under the shared names.

## What

1. **A JMAP watch can miss a hook.** Fastmail answered `Email/changes` with `cannotCalculateChanges` twice; the session ended and reopened on a new baseline, so what arrived between fires nothing. Recommendation: on that error, list the mailbox again and diff it against what the watch knows, reporting the difference, as the DAV backend does for a refused sync token.
2. **`round()` in src/jmap.rs ignores `hasMoreChanges`** (read in code, not seen live): the rest of a batch waits for the next push. Loop until it is false.
3. **Notify hooks unconfirmed**: under pasta the notification answers "Did not receive a reply". Check in a desktop session, or under `dbus-run-session` with a stub notification server in a test.

## Not tested, for the next live run

A triggered change on Graph and JMAP contacts and calendars, a card hook on iCloud (no address book of the run's own can be made there).
