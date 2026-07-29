---
cairn: log
change: multi-account-watch
landed: 2026-07-28
---

# Watch every configured account, or a chosen subset

`carillon watch` watched one account; a watch daemon should watch everything
you configured. Now bare `carillon watch` watches every account at once, and
positional account names narrow the set.

## What landed

The single global `-a` account flag on the top-level parser is gone; account
selection moved onto the `watch` command as positional `ACCOUNT` names
(`Vec<String>`). `Command::execute` no longer threads an `account_name`
through. `watch` selects all accounts when none are named (drained from the
config's named map, in name order) or exactly the named subset otherwise,
erroring on any unknown name. Each selected account runs the existing blocking
driver on its own thread, named after the account, under one shared shutdown;
the command installs the Ctrl+C handler once and joins them. A single account's
driver error is logged on its thread and does not stall the others.

Capability moved: **daemon** — a new requirement pins watch-all / named-subset
selection.

## Deviations and notes

carillon-core and the driver are untouched; this is purely the command layer.
There is no mailbox flag: each account's mailbox comes from its own config
(its `mailbox`, defaulting to INBOX), since accounts watch different mailboxes.
Green on check, clippy, and fmt.
