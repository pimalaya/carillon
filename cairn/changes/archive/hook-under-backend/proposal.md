---
cairn: change
id: hook-under-backend
status: landed
created: 2026-08-22
---

# Put the hooks under their backend, and name them after what they carry

## Why

The hooks sit on the account, one table listing every event any backend might report, under one noun that fits none of them well. Three problems follow.

A backend is asked for events it cannot report, and nothing says so: `dav.hooks.on-flags-added` parses, loads and never fires, because a `sync-collection` report reads etags and a WebDAV collection has no flags. The same holds for `on-item-changed` on the three mail backends, a message being immutable. Both are refused nowhere, which is exactly the runtime-versus-load-time split [watch-under-backend](../watch-under-backend/proposal.md) already closed for the method.

The template vocabulary fails silently and correctly at once. `$subject`, `$sender`, `$recipient` and `$date` are resolved over IMAP alone. An account declaring both `imap` and `maildir`, which is the shape `-b/--backend` exists for, has one `on-item-added` hook whose summary reads `New mail from $sender` under IMAP and expands to nothing under `-b maildir`. Neither reading is wrong; the table is just written against a backend it does not name.

And `item` is a word chosen to be true of everything, which costs it the ability to say anything. A hook firing on a message, a vCard and a VEVENT under one name reads as a lowest common denominator in the config file, where the person writing it knows perfectly well which of the three they are watching.

The hooks are not the account's. They are the backend's, and their names are their domain's.

## What

The table moves under each backend as `imap.hook`, `jmap.hook`, `maildir.hook`, `caldav.hook`, `carddav.hook` and `dav.hook`, singular, each accepting `hooks` as an alias, and each declaring only the events that backend reports.

The events take their domain's noun. Mail is `on-message-added` and `on-message-removed`, which is what IMAP, JMAP and Maildir all carry and the name they shipped under before the generic rename. CardDAV is `on-card-*`. CalDAV is `on-event-*` and `on-task-*`. A plain DAV collection, which has no domain to name, keeps `on-item-*`.

Flags stay singular and fire per flag. `on-flag-added` and `on-flag-removed` fire once for each flag in the delta rather than once for the delta, so `$flag` is always the flag that moved and `$flags` goes. The `flags = [...]` filter narrows which of them fire, which is now a plain per-flag test rather than the any-match over a set that src/hook.rs:74 does today.

`WatchEvent` in src/event.rs stays the one vocabulary, gaining the domain the backend fills. One runner still serves every backend; what splits is the naming and the configuration, not the dispatch.

## The DAV domain split needs the backend split too

Naming CalDAV and CardDAV events apart only pays if a calendar hook on an addressbook is refused, and that has to happen when the file is read. If the domain is merely implied by which hooks were configured, the mismatch is a runtime discovery, which is the thing this change exists to stop. So `dav` becomes three blocks sharing one server shape: `caldav`, `carddav`, and `dav` for an untyped collection. `-b/--backend` names them the same way.

Within a calendar, VEVENT and VTODO are the hard case, because a `sync-collection` report carries an href and an etag and nothing else (io_webdav::rfc6578::sync_collection::WebdavSyncChange), so the component of a changed member is not knowable without reading it, and a vanished member has nothing left to read. Three ways out, in the order they should be tried:

Read `supported-calendar-component-set` on the collection once at watch start. A calendar advertising a single component makes every member that component, which is the common case, and the hooks of the other one are refused there. Then, for a calendar advertising both, ask the report for `DAV:getcontenttype` beside `getetag` and route on the `component=` parameter RFC 4791 §10.1 allows, which needs io-webdav to carry a member's other properties rather than just its etag. What must not happen is fetching the changed member to find out, which would break the promise that a poll never carries a contact or an event.

Either way the href to etag picture in src/dav.rs:170 becomes an href to etag and domain picture, since a removal has only the href left and the domain has to have been remembered.

## What this is not

Not the JMAP datatype axis. JMAP is the clean case, since `Email/changes`, `ContactCard/changes` and `CalendarEvent/changes` are separate calls and the domain of a change is known without reading anything, so `jmap.hook` will gain `on-card-*` and `on-event-*` beside `on-message-*` when the account grows a datatype selector. That wants its own change. This one is its prerequisite.
