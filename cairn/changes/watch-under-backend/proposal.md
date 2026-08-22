---
cairn: change
id: watch-under-backend
status: landed
created: 2026-08-22
---

# Put the watch method under its backend

## Why

The method was an account-level key listing every mechanism any backend might have, so a Maildir account could ask to idle and a WebDAV one to be pushed to. Both were caught, but at watch time, by a runtime refusal this repository had to write and keep honest.

The methods are not the account's, they are the backend's: IMAP has two, JMAP has two, the other two have one. Configured under the backend, each declares only what it has, and asking for the rest stops being a runtime concern.

## What

- `watch` moves under each backend (`imap.watch`, `jmap.watch`, `maildir.watch`, `dav.watch`), each with an enum of the methods that backend has.
- The runtime refusal goes: serde answers `unknown variant 'idle', expected 'poll'` against the line and column, when the file is read.
- The driver matches each backend's own enum, which removes the cross-backend dispatch it needed before.
