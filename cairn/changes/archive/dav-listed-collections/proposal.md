---
cairn: change
id: dav-listed-collections
status: landed
created: 2026-10-04
---

# Watch a DAV collection by the id its listing gives, and keep watching

## Why

An application that syncs an account and watches it with carillon lists the account's collections once, keys them by the id the listing gives, and writes the watch from that. A DAV listing keys a calendar or an addressbook by its path segment under the home set the server's principal names; carillon read `caldav.calendar` and `carddav.addressbook` as a path under `server`, so the same id named another place, and the caller had to translate it.

Watching a calendar or an addressbook for minutes at a time also met four gaps a mail watch never did:

- the connection sits idle the whole interval and a server is free to close it, which ended the session; the next one re-baselined, so what moved in between went unreported;
- a server with no `sync-collection` (RFC 6578 is an extension) could not be watched at all;
- a rejected sync token re-enumerated silently, so what moved in the gap was lost, where the vendor backends report what differs from the known picture;
- a calendar holding events and tasks whose server sends no `component` parameter on its members skipped every member.

## What

- A bare collection, one path segment, is looked up under the calendar or addressbook home set (principal, then home set, discovered from `server` over the watch's own connection); an absolute path is taken as it stands; any other relative path is read under `server` as before. A server naming no home set falls back to reading the segment under `server`.
- A failed round is run again on a fresh connection before the session is given up, the picture and token kept, as the vendor backends do.
- A server refusing `sync-collection` is listed with a `PROPFIND` (Depth 1, `getetag`) on every round, read against the picture; a truncated listing reports no removal.
- A rejected token enumerates again and reports what differs from the picture.
- A mixed calendar's member naming no component is taken for the one domain the hooks name, an event when they name both.
- `check` resolves the collection and falls back to the listing the same way.
