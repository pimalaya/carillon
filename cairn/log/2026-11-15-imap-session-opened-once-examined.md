---
cairn: log
change: watch-streams-events
landed: 2026-11-15
---

# An IMAP session is reported open once its mailbox is examined

The IMAP leftover of session-opened-once-established: io-imap examined the mailbox on its worker without reporting readiness, so a refused EXAMINE printed `session-opened` then `session-lost`. Spec moved: `daemon` (A watch streams what it observes: the IMAP exception removed).

- Cargo.lock: io-imap 0.11.0 to 0.11.1, which adds `ImapMailboxWatchStream::wait_opened` and `opened`.
- src/imap.rs: the watch waits on `wait_opened` a tick at a time, checking the shutdown flag, and calls `on_open` once the mailbox is examined; a worker ending unopened is drained, its EXAMINE error ending the session as `session-lost` alone.
- Tests: src/imap.rs `a_refused_examine_never_opens_the_session` (a local server answering EXAMINE with `NO [NONEXISTENT]`), which failed with `on_open` called right after `watch_mailbox`, the order then put back.
