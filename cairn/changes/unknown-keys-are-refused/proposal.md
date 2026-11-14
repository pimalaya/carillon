---
cairn: change
id: unknown-keys-are-refused
status: landed
created: 2026-11-14
---

# Unknown keys are refused

## Why

The owner decided on 2026-11-14, for the whole family, that a configuration refuses every key it does not read, at every level. Carillon refused one in every backend block and below, but not at the root nor in the account block, which ignored any key so that an account could sit in a file himalaya CLI and himalaya TUI read. That tolerance never bought a shared file (every backend block is strict on both sides), and it hid mistakes: a misspelt account key, a backend the build leaves out, and the keys carillon moved under each backend before 0.1 (mirador's `folder`, `mailbox` and account-level `hooks`, then `collection`, `watch` and the `dav` backend) were all read as nothing. An account-level `hooks` table loaded and never fired.

## What

- `deny_unknown_fields` on `Config` and `AccountConfig`, the two tables without it. No configuration type flattens another, so no table needed restructuring.
- The keys carillon read before and moved are refused by name, pointing at MIGRATION.md, through uninhabited fields as Calendula refuses its removed tables: the account-level `folder`, `mailbox` and `collection` (now the backend's collection key), `watch` (now `<backend>.watch`) and `hooks` (now `<backend>.hook`), and the `dav` backend (now `caldav` or `carddav`). The wizard wrote no key carillon no longer reads: it rendered the current schema.
- The hook aliases stay: `hooks` for a backend's `hook` table, per hook-shape-follows-siblings (owner, 2026-11-05: every former hook name and key read until 1.0). The SASL `username` and `password` spellings of `authcid` and `passwd` stay too, the sample writing them.
- Free maps stay free: `accounts` and `imap.id.fields` hold names, not schema.

## Open

- The family rule says a removed key is refused; hook-shape-follows-siblings keeps the former hook names read until 1.0. Today that is only `hooks` (its renames have not landed), so nothing conflicts yet, but the owner decides which wins before that change lands its `cmd` and `on-*-added` aliases.
- A backend the build leaves out is refused as an unknown key (`unknown field imap`) rather than naming its cargo feature, as a `notify` table does.
- serde lists the refused keys among the expected ones when it refuses another unknown key, as in comodoro, Himalaya and Calendula.
