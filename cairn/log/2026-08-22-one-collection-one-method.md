---
cairn: log
change: one-collection-one-method
landed: 2026-08-22
---

# One account, one collection, one watch method

The account model now reads as one sentence: an account is one backend, watching one collection, one way.

## What landed

`collection` replaces `mailbox`, required, with `mailbox` kept as its former name so no config breaks, and `-m/--mailbox` is gone: the flag was already refused whenever more than one account was watched, and what an account watches belongs in its config. Watching a second collection is a second account, which is also how it gets its own hooks. `dav.server` became the DAV root and the account's collection the path under it, so the DAV backend stopped answering "what do you watch" differently from the other three. The hook variable is `$collection`; `$mailbox` still reaches a hook under its former name.

`watch` names the method, keyed by mechanism the way `imap.sasl` and `jmap.auth` already are: `watch.idle`, `watch.push.ping`, `watch.poll.interval`. Unset takes the best the backend has, and every backend offers the poll, whose interval each one defaults for itself: two seconds for a directory read, a minute for a remote collection. A backend asked for a method it cannot honour refuses to start and says what it offers, since a watch silently downgraded to a poll is how someone ends up wondering why their mail arrives a minute late.

JMAP is pushed to again. The EventSource support was already in io-jmap, a streaming coroutine composing the HTTP read, the chunked decode, the SSE framing and the StateChange parse; what was missing was this side of it. The subscription asks the server to close after each state change (RFC 8620 §7.3 `closeafter=state`), which frees the socket for the `Email/changes` round that follows and makes the loop read like an IDLE: subscribe, wait, read what moved, subscribe again. The poll it replaces stays, as one method among the others rather than the only one, and the regression against 0.1.0 is closed.

IMAP gained a polling watch, which needed io-imap: waiting is an effect an I/O-free coroutine cannot perform, so `ImapMailboxWatch` now yields `WantsWait` when asked to poll, and the std watch worker sleeps the configured interval in shutdown-poll steps. How long to wait stays the driver's, which is what lets an async driver race it against a cancellation instead.

## Verification

Build, clippy and fmt green on every feature combination. The Maildir path was run end to end against a real tree with the new shape: `collection = "."`, `watch.poll.interval = 1`, an arrival and a read reported, `$collection` reaching the hook, Ctrl+C in four milliseconds. That last run is what caught the hook still exporting the old variable name.

The IMAP, JMAP and WebDAV methods remain unverified against live servers.

## Note for the release

`Cargo.toml` patches io-imap to a local path, since the watch options and the polling mode are unreleased. That patch comes out once io-imap ships.
