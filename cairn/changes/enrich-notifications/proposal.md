---
cairn: change
id: enrich-notifications
status: landed
created: 2026-07-28
---

# Enrich notifications with the new message, locally

## Why

carillon-core's ring is content-free by design: it says *the mailbox changed*
and nothing about what changed. A desktop notification that only says "INBOX
changed" is nearly useless — the whole point of a mail notification is to see
*who* wrote and *about what* without opening the client.

The spec already names the fix: resolution is the consumer's job, done "with
the consumer's own credentials and its own protocol client" — the consumer
"re-derives what changed by going to look for itself". The notify consumer is
a *terminal* consumer (it cannot fetch on its own), so the CLI must resolve on
its behalf. This is a purely local enrichment: it stays entirely in the CLI
frontend, never touches carillon-core, and never reaches the server, whose
zero-knowledge story is preserved untouched.

## What

On a change, when a watch has a `notify` hook, the CLI opens its **own** short
IMAP connection (the consumer's own protocol client), EXAMINEs the mailbox,
and `UID FETCH`es the ENVELOPE of the newly-arrived messages — the UID range
`[last_uid_next, uid_next)`. It extracts subject, from name, from address, and
date, and fires one notification per new message, expanding those fields into
the notify templates.

The `cmd` hook is deliberately **not** enriched: its only realistic use is
kicking a sync tool, which re-derives everything itself and does not want the
extra round-trip. It keeps receiving the raw content-free event via the
`CARILLON_*` environment.

Enrichment runs only when a `notify` hook is configured, so a `cmd`-only or
hookless watch pays no extra connection. On a change with no *new* mail (a
flag or delete on a CONDSTORE server), there is nothing new to show, so no
notification fires.

### Scope

- CLI-only. carillon-core is not modified; the ring stays content-free.
- A new resolver opens a fresh connection per change (a login each time).
  A persistent resolver connection is a later optimization, not this change.
- First change of a session, with no prior `uid_next`, shows the single newest
  message; subsequent changes show the precise new range.
