---
cairn: log
change: imap-flags-under-shared-names
landed: 2026-11-23
---

# IMAP flags under the shared names

Found by the live night of 2026-11-23: one STORE of `\Seen` and `\Answered` on the live Stalwart reached `$flag` and the `--json` events as `\Seen` and `\Answered` over IMAP, `Seen` and `Answered` over JMAP and Maildir; IMAP `$Forwarded` stayed raw where JMAP `$forwarded` read `Passed`. The flag filters matched both spellings, so only the names a hook and a JSON reader see were wrong.

## What landed

- src/imap.rs: `render_flags` maps the five IMAP system flags to their bare names and the `$Forwarded` keyword (any case) to `Passed`, other keywords as sent. Pinned by `imap::tests::a_flag_delta_is_reported_under_the_shared_names`, red before the change.
- Capability moved: daemon (the shared flag names are listed, and `$flag` and the events carry them).
