---
cairn: log
change: prompt-shutdown
landed: 2026-08-22
---

# Made Ctrl+C prompt on every path

Comparing the imported daemon against the carillon one it came from turned up two paths where a shutdown could wait a minute, and one where it waited five seconds.

## What landed

io-imap's `ImapClientStd::watch_mailbox` now takes an `ImapMailboxWatchStreamOptions` whose `shutdown_poll` is the read deadline its worker arms, and therefore the worst case for closing the stream. It used to be five seconds with no way to say otherwise; mirador passes one. The default is unchanged, so nothing else moves.

The envelope resolver stopped going through the client's blocking runner, which never sees a shutdown flag. It pumps its own coroutines, checking the flag between reads, over a connection carrying a one-second read deadline with retries turned off. That is the same arrangement the watch worker makes, and the same one the carillon daemon made before the import.

The JMAP connection hands back its not-ready failures too. io-jmap already arms a five-second read deadline so a caller can be woken up, but pimalaya-stream retries such a wakeup away for a minute by default, which is exactly the failure mode the deadline exists to prevent.

## What is still true

A wind-down still drops the connection rather than sending `IDLE DONE` and waiting for the tagged reply: io-imap's worker returns as soon as it sees the flag on a timed-out read. Both this tool and the carillon daemon have always done that, and a server handles it fine, but the coroutine does support a clean wind-down and the worker does not use it. Fixing that belongs in io-imap, bounded so a dead server cannot hold the join.

## Note for the release

`Cargo.toml` patches io-imap to a local path, since the option this change needs is unreleased. That patch has to come out, and io-imap has to ship, before mirador can be released.
