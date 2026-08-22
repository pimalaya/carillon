---
cairn: log
change: watch-under-backend
landed: 2026-08-22
---

# Put the watch method under its backend

The method was an account-level key naming every mechanism any backend might have, so a Maildir account could ask to idle. It was refused, but at watch time, by a runtime check this repository wrote and had to keep honest.

The methods belong to the backend, so that is where they now live: `imap.watch` has idle and poll, `jmap.watch` has push and poll, `maildir.watch` and `dav.watch` have poll. Each backend's enum holds only what that backend has, which turns the runtime refusal into a parse error against the offending line:

```
4 | maildir.watch.idle = {}
  |               ^^^^
unknown variant `idle`, expected `poll`
```

The driver lost its cross-backend dispatch and its error helper along with it: each arm matches its own backend's enum, and there is no branch left for a method that cannot be asked for.

## Verification

Build, clippy and fmt green on every feature combination. The refusal above is a real run, and a Maildir account with `maildir.watch.poll.interval = 1` still reported an arrival end to end.

## Also in this pass

The README lost its Interfaces section, its migration notes and much of its verbosity; the migration guide went with them, leaving one caution that carillon is `v0.x` and will break until it stabilises. The rename note in the changelog carries the one command anyone needed from that guide.
