---
cairn: log
change: collection-under-backend
date: 2026-08-22
---

# The collection moved under its backend and took that backend's word

`collection` was the last account-level key describing something only a backend knows, required whatever the account declared, so a mail account spelled its mailbox under a word chosen to also cover a calendar and a calendar account spelled its path under a word chosen to also cover a mailbox.

Each backend now takes the one collection it watches, required, under the name its domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar`, `carddav.addressbook` and `dav.collection`, the last generic because a plain DAV collection has no domain to name. The account-level key is gone, along with the `mailbox` alias it carried, so an account block holds nothing that needs a backend to be understood.

A hook templates against the same word its backend configures. `$id` is unchanged, being the one thing every backend means the same way, and the collection arrives as `$mailbox`, `$calendar`, `$addressbook` or `$collection` according to what holds it. Naming another backend's word is refused when the file is read, the same as any other variable an event cannot fill:

```
caldav.hook.on-event-added.notify.summary: No such variable: $mailbox. This hook can use $id, $calendar
```

The name is declared once per backend, as `COLLECTION` on its config type, and read by both halves: the accessor that hands the runner a value under that name, and the vocabulary the loader validates against. `Vocabulary` stopped being three constants and became a shape carrying the collection word plus whether an envelope and a flag belong to the hook.

Verified against a local Stalwart: an arrival, a flag change and a removal all expand `$mailbox`, and a calendar hook naming `$mailbox` is refused at load.

Capabilities moved: daemon.

Two things surfaced on the way and were not fixed here. The claim that one file loads in carillon, himalaya CLI and himalaya TUI was already false before this change: himalaya's `ImapConfig` is `deny_unknown_fields` and has no `watch` and no `hook`, and carillon's is equally strict and has no `alpn` and no `sort`. The claim is corrected in the module header, the README and the requirement above rather than left standing; restoring it would mean dropping `deny_unknown_fields` from carillon's backend blocks, which would cost nothing this repository built, the load-time refusals living on the nested `watch` and `hook` tables. Separately, io-imap cannot watch a mailbox that has never been written: Stalwart answers `EXAMINE (CONDSTORE)` on a fresh INBOX with `HIGHESTMODSEQ 0`, which is not a `NonZeroU64`, so the watch fails with `MissingHighestModSeq` and retries until something writes to the mailbox.
