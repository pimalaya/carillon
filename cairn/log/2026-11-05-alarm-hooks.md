---
cairn: log
change: alarm-hooks
landed: 2026-11-05
---

# Alarms fire hooks

carillon fires `on-alarm-trigger` for the alarms of a CalDAV calendar and of a pimdir calendar, alarmd's core imported (calendula discussion #29). Spec moved: `daemon` (ADDED: A calendar fires its alarms, Alarms are held in memory, A late firing fires within the grace window only, An edited calendar moves its alarms, Bodies are read only when an alarm hook wants them, An alarm hook templates against its firing, A pimdir calendar is watchable; MODIFIED: A watch runs from a TOML file, An event is named after its domain; the intro names alarms).

- src/alarm.rs holds alarmd's resolution (src/firing.rs and the iCalendar half of src/store.rs) over ical-rs 0.7 from crates.io, alarmd's path patch gone, and its queue (src/daemon.rs) without the state file: the window starts at the later of the watch's start and the grace window before now, fired keys kept in memory. The wait reads the wall clock every 200 ms behind `wait_until`, shaped as ceichas's.
- The alarm thread lives per account in src/watch.rs (`AlarmRunner`), across sessions; each session swaps a freshly read `AlarmBook` in whole.
- src/dav.rs reads bodies only for an alarm hook: one `calendar-query` per component after the baseline (several comp-filters in one query must all match, RFC 4791 9.7.1, so one query for both would match nothing), a refused filter falling back to every member, then `calendar-multiget` on what each round reports, keyed by resource name. A failed body report ends the session.
- src/pimdir.rs is alarmd's reader narrowed to one collection, on io-pimdir at 0c70144 (`Add` gained `supersedes`, `Update` `if_match` and `resolves` since alarmd's 3b4bed8); a store reset (`store_id`, `incarnation`) reads the calendar again, as a purge does. An account without the hook is refused.
- `backend` (build.rs) stays the cfg of backends reporting changes, so a pimdir-only build compiles the change runner unused, as a build with no backend does. shell.nix and package.nix gain SQLite, package.nix links dbus only under `notify`; neither was built through Nix (the new files are untracked, so the flake does not see them).

Verified on Stalwart (tests/stalwart.sh), a watch polling every 5 seconds: an event read at start fired at T+30, one added during the watch at T+60, a task at T+70, an event edited from T+90 to T+120 at T+120 only, a deleted one and one without an alarm never, each to the second, `$trigger` and `$at` in RFC 3339 with the system offset.

Left: the swap to ceichas (change alarm-wait-on-ceichas); a test moving the wall clock past a trigger mid-wait, which needs a clock the wait can be handed; acknowledge and snooze through hooks; the reply on calendula #29; the ical-rs alarm-instants fold, in that repo.
