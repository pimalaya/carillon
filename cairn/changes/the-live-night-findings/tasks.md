---
cairn: tasks
change: the-live-night-findings
---

# Tasks

- [x] K1: `cannotCalculateChanges` re-lists and diffs instead of a new baseline. Test: a scripted JMAP server answering it once while a message arrives; the hook fires once. Landed 2026-11-24, `jmap::tests::changes_it_cannot_calculate_are_read_from_a_listing`.
- [x] K2: `round()` follows `hasMoreChanges`. Test: a scripted server paging changes in two answers; every hook fires in one round. Landed 2026-11-24, `jmap::tests::a_round_reads_every_page_of_changes`.
- [x] K3: notify hooks checked under `dbus-run-session` with a stub server, or by the owner in a desktop session. Checked 2026-11-24 headless: 31 of 31 notifications reached a stub server on live Stalwart JMAP.
- [x] Next live run: trigger Graph and JMAP contacts and calendars. Run on the live night of 2026-11-24 (.lanes/live/journal-2.md 23:12, 23:13): one Stalwart JMAP watch over mail, an address book and a calendar fired `card-*` and `event-*` once each; Graph contact folder and calendar hooks fired through tests/msgraph.rs (`--ignored`, 3/3).
- [x] J1: a round reads changed items through `/get` calls bounded by `maxObjectsInGet`. Found on live night 2 (Stalwart `requestTooLarge` on a 5200-message flag change). Landed 2026-11-24 (df49c9b, log 2026-11-24-jmap-get-split-by-max-objects), `jmap::tests::a_round_reads_more_changes_than_one_get_takes`.
- [ ] J2 (ruling, still open at 1.0.0 on 2026-11-26: src/jmap.rs still takes a new baseline on a new session): a JMAP session lost (a 429 under a burst, a 5xx) reopens on a new baseline, so what arrived meanwhile never fires; the watch also fetches the session and opens a stream for every round, which drew Stalwart's 429 on live night 2. Recommendation: keep the picture and state across sessions and resume through `/changes`, falling back to the listing diff on `cannotCalculateChanges`; reuse the session between rounds.
- [ ] J3 (ruling, from .lanes/live/report-2.md): one command hook runs about 10 times a second at most, src/hook/command.rs reaping every `REAP_STEP` (100 ms); in a 5200-message burst 2731 ran and the rest were dropped past 1000 waiting, with a WARN. Keep, or reap by a blocking wait per child.
