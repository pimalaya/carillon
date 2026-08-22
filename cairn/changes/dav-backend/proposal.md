---
cairn: change
id: dav-backend
status: landed
created: 2026-08-22
---

# Watch a WebDAV collection

## Why

The three backends so far all watch mail, and the tool is heading somewhere wider: a calendar and an addressbook change too, and someone watching them wants the same hook they already wrote for a mailbox. CalDAV and CardDAV are WebDAV, so one backend covers all of it, and RFC 6578 answers exactly the question a watch asks: what moved since this token.

Poll is not a shortcut here. WebDAV-Push exists on paper and almost nowhere in practice, and nothing a client can subscribe to without a public endpoint. A `sync-collection` report costs one request and returns nothing when nothing changed, which is what makes an interval affordable.

## What it forces

The four events cannot express a WebDAV change. RFC 6578 reports created and updated members together as `changed`, plus a `vanished` list; keeping an href to etag picture of the collection splits the first two apart. But *updated* has nowhere to go: mail never needed it, a message being immutable, whereas a contact or an event is edited in place.

So the vocabulary gains a fifth event, and with it the hooks stop being named after mail. `on-message-added` and `on-message-removed` become `on-item-added` and `on-item-removed`, `on-item-changed` is new, and the old names keep working as aliases so no configuration already written breaks.

## What

- A `dav` backend behind its own cargo feature: `dav.server` is the collection URL, `dav.auth` is basic, bearer or nothing, `dav.poll` the interval.
- `WatchEvent::ItemChanged`, and the item-shaped hook names with the message-shaped ones as serde aliases.
- The connection carries a read deadline and no retry strategy, like every other backend since `prompt-shutdown`, so a poll against a silent server cannot hold a Ctrl+C.
- A truncated report is drained immediately rather than at the next interval; a rejected sync token re-enumerates and reports nothing, a re-baseline not being news.

## Not done

Discovery. A collection URL is pasted in rather than found from a bare domain through RFC 6764 and a home-set lookup, which io-pim-discovery already knows how to do. That is a natural follow-up, and it changes the config surface rather than the watch.
