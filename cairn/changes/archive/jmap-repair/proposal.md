---
cairn: change
id: jmap-repair
status: landed
created: 2026-08-22
---

# Make the JMAP watch survive its own event, resolve what it already read, and retire the untyped DAV backend

## Why

The JMAP push watch loses its session on every change it reports. The EventSource subscription is read over `client.stream`, the one connection the client also sends its API requests on, and it asks the server to close after the first state change (RFC 8620 §7.3 `closeafter=state`). So the socket is dead by the time the subscription returns, and the `Email/changes` that follows writes into it:

```
session lost: cannot read email changes: JMAP Email/changes failed: JMAP send failed:
HTTP/1.1 send failed: reached unexpected EOF: ...
```

The supervisor then reopens the watch and re-baselines, and a baseline reports nothing, so the change that woke the stream is swallowed. A JMAP account on the default method therefore fires no hook at all, and says so only at `warn`. The comment in the code claims the close "frees the socket for the `Email/changes` round that follows", which is exactly backwards: a closed socket is not a free one.

The polling method has the same failure more slowly. It holds one connection across the interval, and a server that closes an idle connection leaves the next round writing into a dead one.

Separately, a JMAP arrival hook has no `$subject` and no `$sender`. Only IMAP resolves an envelope, on a second connection, because an IMAP delta names a UID and nothing else. JMAP is not in that position: the round already calls `Email/get` on every changed id, and the response carries `subject`, `from`, `to` and `receivedAt` for the asking. The envelope is one property list away and is currently thrown away.

And `dav` has outlived its purpose. It was the whole WebDAV backend before CalDAV and CardDAV were split out of it, and what is left is a collection that is neither a calendar nor an addressbook, whose members are called items because nothing better can be said. Nobody watches such a collection with a PIM notifier: the two domains that exist have their own backend now.

## What

- The EventSource gets its own connection, so the subscription closing has nothing to do with the connection the API runs on. The client is handed a fresh stream after a subscription that saw a change, its session being cached and not needing to be read again.
- A round that fails reconnects and runs once more before giving up, which covers the idle connection a polling watch finds closed. A round is atomic against its own state, so running it twice reports nothing twice.
- JMAP asks `Email/get` for `subject`, `from`, `to` and `receivedAt` when an arrival hook is configured, and reports the envelope with the arrival. A backend now reports a change together with whatever it already knows about it, which is one callback shape for all four rather than IMAP's lazy second connection being the only way an envelope can arrive.
- The `dav` backend, its `DavHookConfig`, its `on-item-*` events and the `Item` domain go. CalDAV and CardDAV keep the module, the poll and the domain resolution they already share.
