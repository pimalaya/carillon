---
cairn: log
change: jmap-domains
landed: 2026-10-02
---

# Let one JMAP account watch its contacts and its calendar, not only its mail

## What landed

`jmap.mailbox` became optional beside `jmap.addressbook` and `jmap.calendar`, at least one required, checked in `JmapConfig::validate` with the hook-to-domain check: a hook whose domain has no collection is refused at load, naming the hook and the key it needs. The JMAP hook table gained `on-card-{added,removed,changed}` and `on-event-{added,removed,changed}`, templating against `$addressbook` and `$calendar`; there is no `on-task-*`.

The watch holds one `Watched` per configured domain, each with its collection id, its picture and its state, read through `Email/changes`, `ContactCard/changes` or `CalendarEvent/changes` and resolved through the matching `/get` with only `id` and the membership property. A known card or event named again while still inside is an `ItemChanged`; a message only moves keywords. The baseline lists the addressbook through `ContactCard/query` and the calendar through `CalendarEvent/query`, one id per series. The addressbook and the calendar match by name case-insensitively or by id.

All domains share one session and one connection, a failed round reconnecting once before the session is given up, the poll and the push paths now alike. The push subscription names every configured type on one stream, and a state change runs a round per domain. `HookCollection` became per event: `JmapConfig::collection(event.domain())` answers which collection a hook templates against, and `WatchEvent::domain` was added for it.

## What is still true

A configuration naming only `jmap.mailbox` loads, renders and watches as before. The wizard still writes a mailbox-only account.

## Note for verification

Build, clippy and the suite are green on every feature combination, with unit tests covering reconciliation per domain, collection matching and the load-time refusals. Nothing was verified against a live server: no contact or event was edited to watch a hook fire, so the task stays open. Fastmail serves RFC 9610 contacts; the calendars draft needs a server implementing it, Stalwart for one.
