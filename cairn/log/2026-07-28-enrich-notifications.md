---
cairn: log
change: enrich-notifications
landed: 2026-07-28
---

# Enrich notifications with the new message, locally

The content-free ring made desktop notifications nearly useless — "INBOX
changed" and nothing more. Fixed it the way the spec already prescribes:
resolution is the consumer's job, done with the consumer's own protocol
client. The notify consumer is terminal, so the CLI resolves on its behalf,
locally. carillon-core is untouched; the ring stays content-free and the
server's zero-knowledge story is unaffected.

## What landed

A new `resolve` module opens the CLI's **own** short IMAP connection on a
change (greet, authenticate, EXAMINE, `UID FETCH` the ENVELOPE of the
newly-arrived UID range), extracts subject / from name / from address / date
into a `CarillonImapMessage`, and returns them with the current `uid_next` so
the driver can track the next range. It drives io-imap's coroutines with a
small generic blocking pump, mirroring the watch driver. A `net` module now
holds the shared TLS connect used by both the driver and the resolver.

The driver enriches only when a watch has a notify hook: on a change it calls
the resolver, then fires one notification per new message through `hook`,
which expands `$from` / `$from_name` / `$from_addr` / `$subject` / `$date` /
`$count`. The cmd hook is deliberately left raw — it still receives only the
content-free `CARILLON_*` event — and a cmd-only watch opens no extra
connection. A change carrying no new mail fires no notification.

Capability moved: **daemon** — the "two built-in local consumers" requirement
now enriches notify and leaves cmd raw, plus a new requirement pins the
local-resolution boundary (own connection, never core).

## Deviations and notes

The resolver opens a fresh connection (a login) per change; a persistent
resolver connection is a later optimization. The first change of a session,
with no prior watermark, resolves only the single newest message; subsequent
changes resolve the precise `[last_uid_next, uid_next)` range. The watermark
resets on reconnect. Green on check, clippy, and fmt.
