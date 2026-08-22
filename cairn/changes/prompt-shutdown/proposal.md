---
cairn: change
id: prompt-shutdown
status: landed
created: 2026-08-22
---

# Make Ctrl+C prompt on every path

## Why

Watching is a foreground command someone runs in a terminal, so how long it takes to stop is part of how it feels to use. After the import, three paths could hold a Ctrl+C far longer than the watch loop itself:

- the IMAP watch worker polled its shutdown flag on a five-second read deadline, hardcoded in io-imap;
- the envelope resolver ran its coroutines through the client's blocking runner, which never sees a shutdown flag, over a stream left on pimalaya-stream's default strategy of retrying a not-ready read for a minute;
- the JMAP poll had the same problem: io-jmap arms a five-second read deadline, but the default retry strategy swallows those wakeups for a minute.

Two of those are worse than what the carillon daemon did before the import, which is what prompted the comparison.

## What

- io-imap's `watch_mailbox` takes the shutdown-poll interval as an option rather than hardcoding five seconds; mirador passes one second.
- The resolver pumps its own coroutines with the shutdown flag in hand, over a connection with a one-second read deadline and no retry strategy, so a resolve against a silent server ends at the next deadline instead of holding the thread.
- The JMAP client hands back the not-ready failures its read deadline produces, which bounds a stalled poll at five seconds.

A watch that is winding down still drops its connection rather than sending `IDLE DONE` and waiting for the tagged reply. That is what both tools did before, and making it polite is a separate change in io-imap, where the worker lives.
