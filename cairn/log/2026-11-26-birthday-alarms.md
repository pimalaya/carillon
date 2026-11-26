---
cairn: log
change: birthday-alarms
landed: 2026-11-26
---

# Birthdays and anniversaries fire a hook

carillon fires `on-birthday-trigger` once a year for each `BDAY` and `ANNIVERSARY` of a watched addressbook, over `carddav` and `pimdir`, on the owner's rulings of 2026-11-26 (one hook with `$kind`; the same local day plus `late-days`). Spec moved: `daemon` (ADDED: An addressbook fires its birthdays and anniversaries, An edited addressbook moves its dates, A late birthday fires the same day only, Cards are read only when a birthday hook wants them; MODIFIED: A watch streams what it observes, A WebDAV collection is watchable, An event is named after its domain, The collection belongs to the backend under its own name, Alarms are held in memory, A pimdir calendar is watchable, A failure carries a stable code; the intro names birthdays).

- src/birthday.rs reads a card through vcard-rs 0.6.1 (`parser` alone, the version tcard and cardamum pin), behind `carddav` and `pimdir`: `BDAY` and `ANNIVERSARY` as a full date (basic or extended), a year-less `--MMDD` or `--MM-DD`, a time after the date dropped, `VALUE=text` and a date naming no day ignored. A date resolves to its local day minus `days-before` at `at`, through jiff in the system zone, 29 February on 28 February in a common year; its deadline is the local midnight `late-days + 1` days after its day.
- src/firing.rs is the alarm machinery taken out of src/alarm.rs and shared: `FiringBook` holds the `AlarmBook` and the `BirthdayBook` of an account (each field behind its cfg) and the date rule, `FiringQueue` the horizon, the fired keys and the start bound, each `Firing` with its own late rule (15 minutes for an alarm, the deadline for a date), `firing::run` the ceichas wait. The rebuild window starts at the later of the watch's start and the longest late rule before now, so a suspend longer than the horizon still reaches a date `late-days` lets fire. build.rs gains the `birthday` (carddav, pimdir) and `firing` (either) cfgs; `carddav` now pulls ceichas, jiff and vcard-rs.
- src/watch.rs runs one `FiringRunner` thread per account, firing `on-alarm-trigger` and `on-birthday-trigger` alike; `$late` is counted against the local day the hook fires on. A date prints `birthday-fired` (src/stream.rs: `collection`, `id`, `uid` null without one, `kind`, `name`, `date`, `age`, `late`, `fired` in UTC with `Z`) before its hook runs, a hook failure `hook-failed` naming `on-birthday-trigger`; the event is in the `carillon-watch` schema.
- src/dav.rs reads card bodies only for the hook: one `addressbook-query` with `BDAY` and `ANNIVERSARY` prop-filters under `anyof`, every card on a refused filter, `addressbook-multiget` for what a 507 left out and for what each round reports moved.
- `pimdir.calendar` is optional beside the new `pimdir.addressbook`, at least one; each collection needs its hook and each hook its collection. src/pimdir.rs follows both collections from one change cursor, a missing addressbook coded `addressbook-not-found`, one holding no `text/vcard` refused naming it.
- config: `on-birthday-trigger` takes `notify`, `command` (`cmd`), `timeout`, `at` (`HH:MM`, `"09:00"` unset), `days-before` and `late-days` (0 unset); its templates read `$name`, `$kind`, `$date`, `$age`, `$late`, `$uid`, `$id`, `$collection` and `$addressbook`. config.sample.toml, the README and the 1.0.0 changelog name it.

Each task's test was shown failing with its own code disabled (the resolver stubbed, the late and start bounds mutated, the addressbook fold and the card reads removed, the birthday vocabulary emptied), then passing.

Left: T7, live against Fastmail CardDAV and a neverest-synced store, on the owner's grant; `gpeople`, `msgraph` and `jmap` contacts, and an Apple `X-ABDATE` or vCard 3.0 `X-ANNIVERSARY`, none read.
