---
cairn: log
change: jmap-repair
date: 2026-08-22
---

# The JMAP watch stopped losing its session on every change, and started reporting what it had already read

The JMAP push watch died on every change it reported. The EventSource subscription was read over `client.stream`, the connection the client also sends API requests on, and it asks the server to close after the first state change (RFC 8620 §7.3 `closeafter=state`). So the socket was dead by the time the subscription returned, and the `Email/changes` that followed wrote into it:

```
session lost: cannot read email changes: JMAP Email/changes failed: JMAP send failed:
HTTP/1.1 send failed: reached unexpected EOF: ...
```

The supervisor then reopened the watch and re-baselined, and a baseline reports nothing, so the change that woke the stream was swallowed. A JMAP account on the default method fired no hook at all and said so only at `warn`. The comment claiming the close "frees the socket for the `Email/changes` round that follows" was exactly backwards.

The subscription now holds a connection of its own, so what the server hangs up on is not what the round needs. A round that fails is retried once on a fresh connection, which is what a polling watch needs when its server closed the connection it slept on; the round builds its events and reports none of them until every request it makes has answered, so running it twice reports nothing twice.

JMAP arrivals carry an envelope. The round already calls `Email/get` on every changed id, so `subject`, `receivedAt`, `from` and `to` are added to that request when an arrival hook is configured, and the summary rides back with the arrival. `jmap.hook.on-message-added` may now name `$subject` and `$sender`, which the loader refused before because nothing filled them. A backend reports a change together with whatever it already knows about it, which made the callback one shape across all of them: IMAP still passes nothing and resolves on a second connection, a UID delta naming nothing else.

The untyped `dav` backend is gone, with its `DavHookConfig`, its `on-item-*` events and the `Item` domain. It was the whole WebDAV backend before CalDAV and CardDAV were named out of it, and what was left was a collection that is neither, whose members were called items because nothing better could be said.

Capabilities moved: daemon.

Not verified against a live JMAP server. The two transport fixes are argued from the failure the user reported and from the shape of the code, not from a run: Stalwart speaks JMAP but advertises its container hostname over https as `apiUrl` and `eventSourceUrl`, so it cannot be reached from the host without reconfiguring what the session document says. The envelope half is covered by unit tests over `summarize` and the property list.
