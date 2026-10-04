---
cairn: log
change: dav-listed-collections
landed: 2026-10-04
---

# Watched a DAV collection by its listed id, and kept watching

## What landed

`caldav.calendar` and `carddav.addressbook` given as one path segment are looked up under the calendar or addressbook home set: the principal (RFC 5397) and the home set are discovered from `server` over the watch's own connection, through a redirect-aware pump honouring the shutdown flag, and the path is composed as io-webdav composes one under a home set. A server naming no home set, or failing discovery, reads the segment under `server` as before; absolute paths and multi-segment relative paths are unchanged, so every existing configuration resolves where it did.

The DAV watch became a `Watched` state like the vendor backends'. A failed round is run again on a fresh connection, picture and token kept. A server refusing `sync-collection` (`UnsupportedReport`) is listed with io-webdav's `PROPFIND` fallback on every round. A rejected token, the listing and the baseline all go through one `rebase` over a full snapshot, which reports what differs and, on a truncated snapshot, no removal. `Domains::Mixed` carries the domain a component-less member is taken for. `check` resolves and falls back the same way. `DavServer` is now `Copy`.

## What is still true

The first enumeration reports nothing, and a session that is given up re-baselines, so what moved while no session ran goes unreported, as on every backend.

## Note for verification

Unit tests cover the snapshot diff, the truncated snapshot and the path composition; a fake DAV server in the dav tests answers discovery and refuses `sync-collection`, and the watch finds `work` under the discovered home set, then reports an edit, an arrival and a removal from successive listings. On Stalwart 0.16, one `carillon watch` held `caldav.calendar = "default"` and `carddav.addressbook = "default"` (server `/dav/cal` and `/dav/card`) beside an unreachable third account: an event added, edited and deleted and a card added and deleted each fired its hook, with `id` the member's href and `collection = default`, while the third account backed off alone. Stalwart advertises VEVENT and VTODO and sends no `component` parameter, which is what the mixed-calendar fallback was found for. The connection-drop retry was not seen live.
