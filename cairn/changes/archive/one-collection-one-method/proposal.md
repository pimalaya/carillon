---
cairn: change
id: one-collection-one-method
status: landed
created: 2026-08-22
---

# One account, one collection, one watch method

## Why

The account model had drifted. What an account watched could come from its config or from `-m/--mailbox`, which was refused whenever more than one account was watched, so the flag was already half-dead. What was watched was called a mailbox, which is wrong for a calendar. And the DAV backend hid its collection inside `dav.server`, so it alone answered "what do you watch" differently from the rest.

How an account watched had drifted the other way: nothing was configurable. Maildir and WebDAV polled on constants, and JMAP polled at all only because the EventSource push it shipped with in 0.1.0 was lost in the io-email removal. A server whose IDLE misbehaves had no answer.

Both settle on one sentence: an account is one backend, watching one collection, one way.

## What

- `collection` replaces `mailbox`, required, with `mailbox` kept as its former name. `-m/--mailbox` is gone: what an account watches is its config, and watching a second collection is a second account, which is also how it gets its own hooks.
- `dav.server` becomes the server URL and the account's `collection` the path under it, so every backend answers the question the same way. An absolute collection is taken as it stands.
- `watch` names the method, keyed by mechanism like `imap.sasl` and `jmap.auth` already are: `watch.idle`, `watch.push.ping`, `watch.poll.interval`. Unset takes the best the backend has. A backend refuses a method it does not have rather than quietly using another.
- JMAP is pushed to again, over the EventSource stream io-jmap already had, asking the server to close after each state change so the same socket carries the `Email/changes` round that follows. The poll stays, one method among the others rather than the only one.
- IMAP gained a polling watch, which needed io-imap: waiting is an effect, so its watch coroutine now yields `WantsWait` and the std worker sleeps the interval, leaving how long to wait to whoever drives it.
- `$collection` is the hook variable; `$mailbox` still reaches a hook as its former name.

## Not done

Discovery, still: a DAV collection is written out rather than found from a domain.
