---
cairn: change
id: carillon-import
status: landed
created: 2026-08-22
---

# Import the carillon daemon, and drop io-email

## Why

Two watchers were being maintained: this one, released as v0.1.0 on io-email, and the carillon CLI daemon, unreleased but ahead on everything that is not the watch itself. Keeping both means writing the supervision, the config handling and the hook plumbing twice, and mirador is the one with the git history worth keeping.

The forcing constraint is io-email: it is frozen, it pins an old generation of every protocol crate, and it is the reason this repository sits two majors behind on io-imap and pimalaya-stream. Removing it is not optional, and once it is gone the watch has to come from somewhere.

It comes from io-imap. `watch::ImapMailboxWatch` emits exactly the four events this tool has always exposed, and it no longer requires QRESYNC: without it, io-imap re-reads the mailbox and diffs locally. So the watcher moves down into the protocol crate where it belongs, mirador keeps its hooks, and carillon's bespoke watch coroutine is not imported at all.

## What

- io-email is gone. Each backend now speaks to its own protocol crate: io-imap for the IDLE watch, io-jmap for an `Email/changes` poll, io-maildir for a listing poll.
- The four events become one backend-independent vocabulary, with flags named the same way whatever the backend spells them as, so a hook filter is written once.
- Imported from the carillon daemon: watching every configured account at once on its own thread, the reconnect supervisor with a capped backoff, credentials resolved per attempt, and resolving an arrival's envelope on a second connection so a notification can name the sender.
- Every Pimalaya dependency moves to current: io-imap 0.5, io-jmap 0.3, io-maildir 0.3, io-sasl 0.1 (SASL moved out of pimalaya-stream), pimalaya-stream 0.3, pimalaya-cli 0.2, pimalaya-config 0.1.4, MSRV 1.89.

## What this does not do

The repository is still called mirador. Renaming it to carillon is a separate step, deliberately after this one, so the history of the merge reads as a merge rather than as a rename.
