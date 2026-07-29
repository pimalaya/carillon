---
cairn: change
id: multi-account-watch
status: landed
created: 2026-07-28
---

# Watch every configured account, or a chosen subset

## Why

A daemon should watch everything you told it about. Today `carillon watch`
watches exactly one account — the default, or the one named by the single
global `-a` — so a second mailbox needs a second process. That is not what a
watch daemon is for.

## What

Bare `carillon watch` watches **all** configured accounts at once, one thread
per account, under a single shared shutdown. Accounts named as **positional**
arguments narrow the watch to a chosen subset, e.g.
`carillon watch work personal`. An unknown account name is an error, not a
silent skip. Each account's watched mailbox comes from its own config (there
is no mailbox flag), since accounts watch different mailboxes.

Each account runs the existing blocking driver on its own thread; the main
thread installs the Ctrl+C handler and joins. One account's failure (a lost
connection, a bad credential) is logged and reconnects on its own thread
without stalling the others.

### Scope

- CLI-only; carillon-core and the driver are unchanged.
- The single global `-a` account flag on the top-level parser is removed; the
  `watch` command takes positional account names instead (the other
  subcommands never used the flag).
- No config change: accounts are already a named map, each carrying its mailbox.
