---
cairn: log
change: hook-under-backend
date: 2026-08-22
---

# Hooks moved under their backend and took their domain's name

The account-level `hooks` table is gone. Each backend now carries its own `hook` table (`hooks` reads as an alias), holding only the events that backend reports, so a hook it could never fire is refused when the configuration is read. Refusal names the line, the column and the events that backend has, which is what `carillon check` answers to `imap.hook.on-item-added` and to `carddav.hook.on-event-added`.

The events took the noun of what they carry. Mail fires `on-message-added` and `on-message-removed` over IMAP, JMAP and Maildir; CardDAV fires `on-card-added`, `on-card-removed` and `on-card-changed`; CalDAV fires the same three under `on-event-` and `on-task-`; a plain DAV collection keeps `on-item-`. The mirador-era `on-message-*` aliases on the old table went with the table.

Flag hooks became singular and fire per flag. `on-flag-added` and `on-flag-removed` fire once for each flag that moved, so `$flag` always names the flag the firing is about and `$flags` is gone; the `flags = [...]` filter is now a plain per-flag test rather than an any-match over a set. Verified against a local Stalwart: one `STORE +FLAGS (\Seen \Flagged)` fires the hook twice.

Naming CalDAV and CardDAV events apart only pays if the mismatch is refused at load, so the DAV backend split into `caldav`, `carddav` and `dav`, three blocks sharing one server, authentication and poll shape, and `-b/--backend` learned all three. A calendar is asked for its `supported-calendar-component-set` when the watch starts: holding one component, it answers for every member at no cost; holding both, a member the watch has not seen has its `getcontenttype` read and is routed by the `component` parameter of RFC 4791 §10.1. A member is never fetched to find out what it is. The collection picture became href to etag and domain, since a removal leaves only an href behind.

`WatchEvent` carries a `WatchDomain` and one flag per event, which is what lets the naming split without the runner splitting: `hook::run` takes the hook an event resolved to rather than a table, and the backend that reported the change is what resolved it.

Capabilities moved: daemon.

Not in this change: the JMAP datatype axis. `Email/changes`, `ContactCard/changes` and `CalendarEvent/changes` are separate calls, so a JMAP change knows its domain for free, and `jmap.hook` will gain `on-card-*` and `on-event-*` once the account has a datatype selector. `DAV:getcontenttype` is declared in carillon's dav module and belongs upstream beside io-webdav's own `GETETAG`.
