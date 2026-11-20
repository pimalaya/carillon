---
cairn: change
id: the-contract-audit-rulings
status: landed
created: 2026-11-20
---

# The contract audit rulings

## Why

The CLI contract audit of 2026-11-20 left carillon one item blocking 1.0 (CRL-X1, an instant printed in the system's offset where the family prints UTC) and two owner rulings not yet applied (A5: no by-name refusal of a removed key and no 0.x to v1 guide). The owner's rulings of the same day add one TLS table spelled as himalaya spells it, inherited from the account, named on every check row, and `account check` printing a list of accounts (A1, A2). pimalaya-cli carries the kit side (the check row's `tls`, the list of accounts, the bare invocation under `--json`, the standard commands requiring their argument).

## What

- CRL-X1: `alarm-fired.trigger` is RFC 3339 in UTC with `Z`; the hook variables `$trigger` and `$at` keep the system zone's offset.
- CRL-X2, A5: every `removed_key!` refusal goes (the account-level `folder`, `mailbox`, `collection`, `watch`, `hooks`, `dav`, a backend's `hook`, the fourteen `-added`, `-removed`, `-changed` hook names), each now serde's unknown field; MIGRATION.md is deleted, and the CHANGELOG lines pointing at it reworded.
- CRL-X4: `hook-failed` carries `hook`, the hook's name (`on-message-add`, `on-alarm-trigger`), and `id`, the item it fired for.
- A1: an account-level `tls` table, each backend's `tls` taking from it, field by field, what it leaves unset (`client` as one); `tls.client.cert` and `tls.client.key` everywhere; each `account check` row carries `tls`, `null` for `maildir`, `pimdir`, an `imap://` server without `starttls` and an `http://` JMAP or DAV server.
- A2: `account check` prints the kit's list of accounts, `{"accounts": [{"account", "backends"}]}`.
- Kit item B: a bare `carillon` answers through `pimalaya_cli::clap::meet_bare_invocation` (under `--json`, `usage-invalid`, exit 2); `manual`, `completion` and `json-schema` without argument are refused by clap, exit 2, as the kit now requires.
