---
cairn: log
change: maildir-check-opens-the-mailbox
landed: 2026-11-25
---

# The maildir check opens the mailbox

## What landed

- src/maildir.rs: `probe` resolves the configured mailbox and lists it once, as `watch` does before its first wait.
- src/account/check.rs: the `maildir` row runs `probe`, where it only checked that the root was a directory, so a root without cur/, new/ and tmp/ passed the check and the watch then lost its session.
- A root that is no Maildir, missing included, or a mailbox it does not hold fails the row coded `mailbox-not-found`, as the watch's `session-lost` is; tests/exit.rs pinned the former uncoded failure and now expects the code.
- Pinned by tests/account.rs `a_maildir_check_fails_on_a_root_that_is_no_maildir` and `a_maildir_check_codes_a_missing_mailbox`, both red before the change (exit 0, the row passing).
- Capability moved: daemon (`account check` states the maildir row).
