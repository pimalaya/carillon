---
cairn: change
id: alarm-hooks
status: landed
created: 2026-11-05
---

# Alarms fire hooks

## Why

calendula discussion #29 asks for a command run "when an event is happening or a task is due". alarmd, built for it on 2026-10-27 and never published (local directory, no git), reads a pimdir store only, so a person who does not sync gets nothing. carillon already watches a CalDAV calendar, and the etag feed it holds is exactly what keeps alarms current. carillon reacts to something and fires hooks; a clock becomes one more thing it reacts to, the server changes keeping what the clock waits for up to date.

No push is lost by staying on the poll: CalDAV has none a desktop can use. RFC 6578 is a poll, WebDAV-Push is an independent draft (`-00`, May 2025) served by Nextcloud's `dav_push` app alone and delivered over Web Push, which needs a public endpoint. An edit reaches an alarm one poll interval late at most (60 s for DAV).

## What

### The hook

`on-alarm-trigger`, under `caldav.hook` and `pimdir.hook`, in the shape every carillon hook has today (`notify`, `cmd`); hook-shape-follows-siblings renames the table and keys of every hook alike later.

The name is `on-<subject>-<event>`, as comodoro and ortie name theirs. An alarm is a part of an event or a task as a flag is of a message, so it takes its own subject (`on-flag-add`, not `on-message-flag-add`), and RFC 5545 3.8.6.3 says an alarm triggers. The name leaves room for `on-alarm-acknowledge` and `on-alarm-snooze`.

Variables: `$id`, `$collection` and `$calendar`, `$etag`, `$uid`, `$kind` (`event` or `task`), `$summary`, `$recurrenceId`, `$alarm` (the index of the `VALARM` in its component), `$alarmUid` (RFC 9074 4, when set), `$action` (`display` or `audio`), `$trigger` (when it fires) and `$at` (the start of the occurrence for an event, the due time for a task). Times are RFC 3339 with the system zone's offset, as calendula's `startsAt`. An `AUDIO` alarm asks the notification for the freedesktop sound alarmd used.

### What fires

alarmd's rules, unchanged: firings resolved by ical-rs (`IcalAlarms`, released in 0.7.0) over a 48 hour horizon, rebuilt on a change and every half horizon; a cancelled component, a completed task, an alarm `ACKNOWLEDGED` at or after its trigger, and an action other than `DISPLAY` and `AUDIO` never fire; a floating time resolves in the system zone.

### In memory, as the watch is

Firings live in memory and nothing is written between runs, as the etag picture already is. A watch starting fires nothing whose trigger is already past, so a restart never repeats one; alarms due while carillon was down are lost. A process surviving a suspend still holds its queue: a firing reached late by at most the grace window (15 minutes) fires, a later one is dropped with an info log. No state file, no socket server, no `list` command.

### Read-only

carillon writes nothing. Acknowledging and snoozing are later work, by a hook calling a writer such as calendula with the item's id and etag; the edit then comes back through the watch and drops or adds firings like any other.

### CalDAV

Bodies are read only when the account configures `on-alarm-trigger`, as an arrival's envelope is only read when its hook wants it. Once the baseline is taken, one `calendar-query` per component (`VEVENT`, then `VTODO`, the comp-filters of one query all having to match, RFC 4791 9.7.1) fetches the members holding an alarm (io-webdav `CaldavItemList` takes the filter), a server refusing the filter being asked for every member; after that, a member the poll reports added or changed is fetched with `calendar-multiget` and its firings replaced, and a removed one is forgotten. The etag picture is untouched, covering every member: an edit can give an alarm to an event that had none.

### A pimdir backend

A new `pimdir` block, behind a `pimdir` cargo feature: `pimdir.root`, `pimdir.calendar` (one collection, the id calendula's pimdir backend lists), and `pimdir.watch.poll.interval` (30 s by default, as alarmd). It is alarmd's reader: no lock, no write, committed rows only, the change feed followed. It declares `on-alarm-trigger` alone for now.

### The clock

Each account with an alarm hook gets an alarm thread beside its watch thread, waiting for the next firing, woken early by the watch when firings move and by the shutdown flag. The watch keeps its own interval.

The wait is on the wall clock, not on a duration. Every std sleep runs on a clock that stops while the machine sleeps (Linux `CLOCK_MONOTONIC`, the macOS uptime clock; Windows 8 and later leave low-power time out of wait timeouts), so sleeping `trigger - now` fires late by the length of a suspend and ignores a clock set by hand or by NTP. The thread waits in the 200 ms steps `poll::sleep` already takes for a prompt shutdown, reading `SystemTime::now()` at each one, so a firing is at most a step late after a resume or a clock change, on every platform and with no platform code.

The steps are a stand-in for ceichas (pimalaya/ceichas, change bootstrap), developed in parallel: one interface over the OS timers that fire at a wall time, with no polling. The wait therefore takes ceichas's shape from the start, one private function answering `Reached` or `Woken` for a `SystemTime`, with a waker the watch calls when firings move and the shutdown calls when it is requested; swapping in `CeichasTimer` is then that function's body and the shutdown handler calling the waker. Waking a suspended machine is out, here and in ceichas.

### Notifications

Shown through pimalaya-config's `NotificationTemplate`, as every carillon hook is since hook-templates-from-config. alarmd's fixed notification texts go: the account templates its own.

### Taken from alarmd, and left

Taken: src/firing.rs (the firing, without its JSON schema and `missed` flag), the iCalendar half of src/store.rs (a body to series to firings), the queue of src/daemon.rs (window, grace, horizon, the clock trait tests drive), the pimdir half of src/store.rs, and the scenarios of tests/daemon.rs.

Left: src/state.rs, src/server.rs, src/client.rs, src/jsonrpc20.rs, src/protocol.rs, src/transport.rs, src/cli/, src/notify/ and src/hook.rs (the firing as JSON on stdin, replaced by carillon's templates). ical-rs comes from crates.io at 0.7, ending alarmd's path patch; io-pimdir at the revision the other apps pin (0c70144).

## Not done

- Acknowledge and snooze through hooks. They need `$id` to be what calendula addresses: carillon's DAV `$id` is the full href (`/cal/alice/work/a.ics`), calendula's item id the resource name (`a.ics`).
- A summary of alarms missed beyond the grace window.
- Several calendars per account.
- Alarms from JMAP (JSCalendar alerts), Microsoft Graph (`reminderMinutesBeforeStart`) and Google Calendar (`reminders`).
- A sync round right before each firing, closing the one-interval gap.
- `on-event-*` and `on-task-*` from the pimdir change feed.
- A shorter pimdir poll: a change-cursor read is one local query, so 30 s is a start, to adjust.
- The swap to ceichas, once released: its last milestone here, a follow-up change if this one lands first.
- The 200 ms shutdown polling of the other watch threads (src/poll.rs), which ceichas's waker could also replace.
