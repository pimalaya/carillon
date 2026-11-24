---
cairn: log
change: the-live-night-findings
landed: 2026-11-24
---

# JMAP changes drained and listed again

From the live night of 2026-11-23 (the-live-night-findings, K1 to K3).

## What landed

- src/jmap.rs: `Watched::round` asks `/changes` again while the server answers `hasMoreChanges`, each answer applied whole (K2, read in code on 2026-11-23, never seen live). Pinned by `jmap::tests::a_round_reads_every_page_of_changes`, red before the change (`e4` missing from the round).
- src/jmap.rs: a `cannotCalculateChanges` answer no longer fails the round, whose retry met it again and ended the session, which reopened on a new baseline (Fastmail, twice on 2026-11-23, "JMAP Email/changes failed: cannotCalculateChanges: outdated sinceState"). `Watched::rebase` reads the state, lists the collection as the session opening does, and reports what differs from the picture: arrivals (with their envelope when the arrival hook wants one), removals, keyword moves (K1). Pinned by `jmap::tests::changes_it_cannot_calculate_are_read_from_a_listing`, red before the change (the round failed with that error through `rounds`).
- Capability moved: daemon (the JMAP requirement reads `hasMoreChanges` and `cannotCalculateChanges`, two scenarios).

## Checked, no code change

- K3: notify hooks deliver. Under `dbus-run-session` with a stub `org.freedesktop.Notifications` (Python dbus-next from nix-shell), a watch of the live Stalwart over JMAP under pasta showed one notification per arrival, 31 of 31, summary and body templated. The night's "Did not receive a reply" was a session bus with no notification daemon behind it.
- Live Stalwart JMAP push on 2026-11-24: 31 arrivals, one flag and one removal each fired its hook once. Stalwart cannot be made to answer `cannotCalculateChanges`, so K1 rests on its scripted test.
