---
cairn: tasks
change: alarm-hooks
---

## M0: review

- [x] Settled (user, 2026-11-05): `on-alarm-trigger`, a firing beyond the grace window dropped with a log, the pimdir poll at 30 s
- [x] Hook renames left to hook-shape-follows-siblings (user, 2026-11-05: breaking changes later); templates and camelCase variables from hook-templates-from-config

## M1: the alarm core, imported from alarmd

- [x] Dependencies: ical-rs 0.7 from crates.io (`parser`, `tzdb`), jiff (`std`, `tz-system`, `tzdb-bundle-always`), both under `dav` and `pimdir`
- [x] src/alarm.rs from alarmd src/firing.rs and the iCalendar half of src/store.rs: a body to series, firings over a window in time order, the drop rules
- [x] The queue from alarmd src/daemon.rs without the state file, the window starting at the watch's start; a pass taking `now`, so tests need no clock
- [x] The alarm thread per account, across sessions, waiting on the wall clock in 200 ms steps, woken by a moved firing and by the shutdown flag
- [x] The wait behind `wait_until`, shaped as ceichas's (`Reached` or `Woken`, a wake sent before the wait kept)
- [ ] A test moving the wall clock past a trigger mid-wait: needs a clock the wait can be handed, left for the ceichas swap

## M2: the hook

- [x] `on-alarm-trigger` on `CaldavHookConfig` and `PimdirHookConfig`, in the `ItemHook` shape; refused elsewhere by the backend's strict table
- [x] The alarm vocabulary in src/hook.rs, RFC 3339 times in the system zone, `alarm-clock-elapsed` on an `AUDIO` notification

## M3: CalDAV

- [x] After the baseline, when the hook is configured: one `calendar-query` per component holding a `VALARM`, every member on a refused filter
- [x] Added and changed members fetched with `calendar-multiget` by resource name, removed ones forgotten, the alarm thread woken
- [x] No body read without the hook: `alarms` is `None` for such an account, so no report runs (checked by reading, no request recorder in this crate)

## M4: the pimdir backend

- [x] `pimdir` cargo feature (default), io-pimdir pinned to 0c70144, `vendored` passed through; `Backend::Pimdir` and `-b pimdir`
- [x] The `pimdir` block: `root`, `calendar`, `watch.poll.interval`, `hook.on-alarm-trigger`; no wizard entry
- [x] The reader narrowed to one collection, the change cursor polled, a reset read whole again
- [x] `check` on a pimdir account: the store and the collection exist

## M5: tests

- [x] Unit: a firing at its trigger, the dropped kinds, a restart, the horizon, the grace window both ways, no double firing, an edit, a removal, an event gaining an alarm, the wait, RFC 3339 times, the alarm vocabulary, a command reading the firing
- [x] A scratch pimdir store: a missing store refused and not created, a non-calendar refused, a calendar read then followed through an update and a removal
- [x] CalDAV on Stalwart (tests/stalwart.sh): see the log entry
- [x] `cargo fmt`, `cargo test` (default and all features), `cargo clippy --all-features`, `--no-default-features`, `dav` alone, `pimdir` alone, `imap` alone

## M6: ceichas

- [ ] Moved to change alarm-wait-on-ceichas

## M7: land

- [x] config.sample.toml: a CalDAV and a pimdir alarm example
- [x] Spec intro names alarms
- [x] Delta folded into spec/daemon.md, log entry, CHANGELOG under Unreleased
- [x] alarmd: bootstrap change set `abandoned`, pointing here; removing the directory is the user's
- [ ] A reply on calendula discussion #29 (drafted for the user to post)
- [ ] ical-rs: the alarm-instants change, released in 0.7.0, still waits for its fold and log (in that repo)
