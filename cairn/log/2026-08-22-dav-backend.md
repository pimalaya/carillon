---
cairn: log
change: dav-backend
landed: 2026-08-22
---

# Watch a WebDAV collection, and stop naming events after mail

A fourth backend, and the vocabulary change it forced.

## What landed

`src/dav.rs` polls an RFC 6578 `sync-collection` report over any WebDAV collection, which is what a CalDAV calendar and a CardDAV addressbook are. The report asks for `getetag` and nothing else, so a poll carries no vCard and no VEVENT: the watch says a member moved, and a hook that wants its content goes and reads it. `dav.server` is the collection URL, `dav.auth` is basic, bearer or nothing, `dav.poll` the interval, a minute by default. It is behind a `dav` cargo feature, and `check` probes it with one report, which proves the transport, the credential and that the collection is where the config says.

RFC 6578 reports created and updated members together, so the backend keeps an href to etag picture of the collection and reads the difference: a member never seen is an arrival, a known member whose etag moved is an edit. A truncated report is drained straight away rather than at the next interval, since the rest is already waiting behind the token the server just handed back. A token the server refuses causes a re-enumeration, and that reports nothing, a re-baseline not being news.

That edit had nowhere to go. The four events were mail-shaped, and a message is immutable where a contact is not, so the vocabulary gained a fifth event and stopped being named after mail: `on-message-added` and `on-message-removed` are now `on-item-added` and `on-item-removed`, `on-item-changed` is new, and the old names still work as aliases, so no configuration already written breaks. `WatchEvent` follows, and the enum carries an allow for the variants a reduced feature set cannot construct, since which events exist is the vocabulary's business and which are reachable is the enabled backends'.

The connection is opened the way `prompt-shutdown` established: a read deadline and no retry strategy, so a poll against a server that stopped answering ends at the next deadline rather than holding the thread.

## Verification

Build, clippy and fmt green on every feature combination (imap, jmap, maildir, dav, and all four), and the five Maildir tests still pass. No CalDAV or CardDAV server has been watched yet; that is the open task on the change, alongside the same one for IMAP and JMAP.

## Not done

Discovery. The collection URL is pasted in rather than found from a bare domain through RFC 6764 and a home-set lookup, which io-pim-discovery already does for the other Pimalaya tools. It changes the config surface rather than the watch, so it fits as a follow-up.
