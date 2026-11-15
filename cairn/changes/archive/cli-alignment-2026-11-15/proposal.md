---
cairn: change
id: cli-alignment-2026-11-15
status: landed
created: 2026-11-15
---

# carillon on the family line of the 2026-11-15 alignment round

## Why

The CLI alignment audit of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15, findings F18, F19, F21, F33, F40, F43 and owner rulings D6 and P1) found carillon apart from himalaya, the reference, on seven points: a bare run and the standard commands with no argument answer differently, the hooks refuse `cmd` where the family reads it as the alias of `command`, a watched collection that does not exist carries no code and the pimdir codes relayed are a subset of himalaya's, the text events print untrusted strings raw, DAV has no pinned `home` and pimdir no `account`, the `check` text has its own spacing, and the standard commands have their own abouts while a default build links the system SQLite.

## What

The plan's lines R2 to R8 (R1, the `account` group, waits for pimalaya-cli 0.5.1):

- **R2** (F21, breaking): a bare `carillon` meets a missing configuration as any command does and prints the help once one exists (src/cli.rs `meet_bare_invocation`, as himalaya's); `manual` and `completion` lose `arg_required_else_help`, so with no argument they fail exit 1 asking for `--dir`, as `json-schema` and himalaya's do.
- **R3** (D6, "accept it everywhere instead, cmd is a cmon alias, no point to refuse it"): a hook's `command` is read under `cmd` too (`#[serde(alias = "cmd")]`), the refusal gone.
- **R4** (F33): `mailbox-not-found`, `calendar-not-found` and `addressbook-not-found` where a backend can tell a missing collection (IMAP `NONEXISTENT`, a Maildir folder that is no directory, a DAV 404 on the collection, a collection no JMAP, Gmail, Google Calendar, Google People listing names, a Graph collection unlisted and answered 404 or 400 `ErrorInvalidIdMalformed`, a pimdir calendar the store lacks); the pimdir relay table is himalaya's 29 codes.
- **R5** (F19): every untrusted string of a text event goes through pimalaya-cli's `sanitize::text`.
- **R6** (F40, additive keys only): `caldav.home` and `carddav.home` pin the home set as cardamum and calendula do, `server` becoming optional; `pimdir.account` as himalaya reads it, unset keeping today's behaviour.
- **R7** (F18): the `check` text on pimalaya-cli's `Fields`, as himalaya's `account check` prints it.
- **R8** (F43, P1): the standard commands under pimalaya-cli's abouts, a spec sentence on why there is no `repl` and no `mcp`, and `vendored` in the default features; the order of the standard commands follows himalaya's (P5, the carillon part of the plan's X1), the lines being rewritten anyway.

No configuration key is renamed or removed.
