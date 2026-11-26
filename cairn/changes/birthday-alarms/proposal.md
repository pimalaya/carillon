---
cairn: change
id: birthday-alarms
status: active
created: 2026-11-26
---

# Birthdays and anniversaries fire a hook

## Why

carillon fires the `VALARM`s of a watched calendar (cairn/spec/daemon.md, "A calendar fires its alarms"), but a birthday kept in a contact reminds nobody. On CardDAV servers (Fastmail, Nextcloud, Radicale, iCloud over CardDAV) a birthday lives only in the vCard's `BDAY`, and an anniversary only in its `ANNIVERSARY` (RFC 6350 §6.2.5, §6.2.6): no calendar carries them. Google, Microsoft and Apple generate a Birthdays calendar, but not for every account nor on every client. The NLnet milestone carillon 6.a names birthdays beside events and alarms; the owner ruled on 2026-11-26 that it lands before carillon 1.0.0.

The pieces exist: the alarm path holds firings in memory and waits on ceichas's wall-clock timer (survives suspend and clock changes), the hooks and their templates are shared, the `carddav` and `pimdir` backends already watch an addressbook, and vcard-rs parses cards in the family.

## What

- A new hook `on-birthday-trigger` on the backends that watch an addressbook, starting with `carddav` and `pimdir` (as `on-alarm-trigger` started with `caldav` and `pimdir`), firing once a year per `BDAY` and per `ANNIVERSARY` of every card of the watched addressbook.
- When it fires: at a local time of day the hook's config names (`at`, default `09:00`), on the date itself or a number of days before (`days-before`, default 0), in the machine's local zone (a vCard date is floating).
- Variables: `$name` (the card's `FN`), `$kind` (`birthday` or `anniversary`), `$date` (the date, year included when the card states one), `$age` (the years it completes, empty for a year-less date), `$uid`, plus the account and collection variables every hook has.
- Year-less dates (`--MMDD`, RFC 6350 §4.3.1, and Google People's year-less birthday) fire with `$age` empty. A 29 February fires on 28 February in a common year.
- Reading: a card the watch reports added or changed is read again and its dates replaced, a removed card's forgotten, as an edited calendar moves its alarms. On `carddav` this means reading the cards' bodies, which the watch does not do today (it reports etag moves only); on `pimdir` the card bodies are read from the store, since no summary carries a birthday.
- Firings are held in memory on the alarm path's horizon and never repeated after a restart. The grace window differs from alarms: a birthday reached late (the machine asleep at 09:00) fires on waking the same local day, since a date reminder is still useful later that day; later it is dropped with an info line, unless the hook's `late-days` (default 0) allows that many days more, the hook then carrying `$late` (the days late, 0 on time). A watch starting still fires nothing before its start, since carillon writes no state between runs: `late-days` covers a machine asleep while the watch runs, not one switched off.
- Out of scope here: `gpeople`, `msgraph` and `jmap` contacts (Google and Graph state birthdays in their own fields; a follow-up once the two first backends land), and a dedupe against a provider's Birthdays calendar, which the docs name instead (watch one or the other).

## Owner rulings (2026-11-26)

- One hook, `on-birthday-trigger`, with `$kind`.
- The late rule: the same local day, plus the `late-days` option.
