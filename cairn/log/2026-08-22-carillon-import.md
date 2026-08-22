---
cairn: log
change: carillon-import
landed: 2026-08-22
---

# Imported the carillon daemon, and dropped io-email

mirador keeps its history and its four hooks, and gains everything the carillon CLI daemon had learned. io-email is gone from the dependency list, which is what forced the rest.

## What landed

`src/imap.rs`, `src/jmap.rs` and `src/maildir.rs` replace the single io-email client. IMAP drives io-imap's own `ImapMailboxWatch`, so this repository owns no watcher: the watcher lives in the protocol crate, emits the same four events it always did, and no longer requires QRESYNC (io-imap re-reads and diffs locally when the server lacks it). JMAP polls `Email/changes` and resolves the changed ids through `Email/get`, keeping the ones inside the watched mailbox and diffing their keywords. Maildir re-lists the mailbox and diffs file names and flag letters.

`src/event.rs` is the vocabulary all three speak, and flags are normalised on the way in, so `flags = ["Seen"]` fires against `\Seen`, `$seen` and the `S` letter alike. `src/driver.rs` is the per-account supervisor imported from carillon: one thread per account, capped exponential backoff that a healthy session resets, credentials resolved per attempt. `src/watch.rs` watches every configured account when none is named. `src/hook.rs` keeps the four hooks and their templates, now fed by the vocabulary rather than by io-email types, and gained `$date`.

Because io-imap reports a UID rather than an envelope, `$subject` and `$sender` are resolved on a second connection, and only for an account that configures `on-message-added`. That resolver is carillon's, simplified: the delta names the exact UID, so there is no watermark to track.

Dependencies moved to current: io-imap 0.5, io-jmap 0.3, io-maildir 0.3, io-sasl 0.1 (SASL left pimalaya-stream and now has its own crate), pimalaya-stream 0.3, pimalaya-cli 0.2, pimalaya-config 0.1.4, MSRV 1.89. `imap.sasl-ir` is new config, for the providers that advertise SASL-IR falsely.

## Verification

Build, clippy and fmt green on every feature combination (imap, jmap, maildir, and all three). No backend has been run against a live server yet; that is the open task on the change.

## Not done

The repository is still called mirador. The rename to carillon comes after, so this reads as a merge in the history rather than as a rename.

## Follow-up: the maildir mailbox was resolved by hand

The first cut joined the configured root with the mailbox name directly, which bypasses io-maildir's store. That store is what applies the layout, and under Maildir++ a subfolder is a dot-prefixed flat sibling (`.Archive`, `.a.b`), not a plain subdirectory. Watching `Archive` on such a store would have listed an empty directory forever, reporting nothing and erroring never.

It now resolves through `MaildirClient::load_maildir`, which applies the store's layout and checks that `cur`, `new` and `tmp` exist, so a wrong name fails at startup instead of watching a void. `.` and `INBOX` both name the store root. The config sample said Maildir++ where io-maildir defaults to real nested directories, the layout himalaya uses; it now says what actually happens.

## Verified: the Maildir backend, end to end

Maildir needs no server, so it is the one backend that can be run here, and it was. Against a real tree, the daemon reported an arrival, a read (the `new` to `cur` move with the `S` letter appended, which it correctly called one flag change rather than a departure and an arrival, since the id is the file name before `:2,`), and a deletion, firing the matching `cmd` hooks with the right variables each time. Ctrl+C returned in under ten milliseconds.

Five unit tests cover the same ground on the pure parts: the three diff transitions, the subfolder resolution that the fix above is about, and the read-is-a-flag-change case. The other two backends remain unverified against a live server.

One thing the run exposed: the supervisor logged "session ended, reopening" while it was shutting down, because it matched on the session's outcome before looking at the shutdown flag. It now checks the flag first, so a shutdown says so and a failure raced on the way out is a debug line rather than a warning about a session nobody is reopening.
