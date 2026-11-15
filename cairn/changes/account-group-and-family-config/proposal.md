---
cairn: change
id: account-group-and-family-config
status: landed
created: 2026-11-15
---

# The account group and the family's configuration sentences

## Why

The second pass of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15, line R1 and the carillon parts of L7 and L8, findings F1, F40, F41, F44, owner ruling D1: "yes to account group, needs to be aligned with the rest") waited on pimalaya-cli 0.5.1 and pimalaya-config 0.5.1, both released that day. carillon checked an account with a top-level `check` where every family binary has an `account` group, could not list its accounts, printed its own copy of the check report, refused a removed key in two sentences of its own, and read a duration as integer seconds only where the family reads `"90d"` too.

## What

- **R1** (D1, not breaking): an `account` group holding `account list` and `account check`, on pimalaya-cli's `account::AccountListOutput`, `AccountRow`, `AccountCheckOutput` and `BackendCheck`, so the JSON and the text are the family's. `check` stays a hidden alias of `account check`, its `carillon-check` schema key kept beside `carillon-account-check` (the registry requires a key per runnable command, a hidden one included). The check rows keep carillon's codes, typed causes included, through `pimalaya_cli::error::with_code`. The welcome names `account check`. carillon takes no `account capability`, having no unified API.
- **F41**: every removed-key refusal (the account-level `folder`, `mailbox`, `collection`, `watch`, `hooks`, the `dav` backend, a backend's `hook` table and the hook names ending in `-added`, `-removed`, `-changed`) is a `pimalaya_config::removed_key!` type, reading "The `<name>` key is removed, <why>: <what to write instead>, see MIGRATION.md" (`table` for a table). The keys refused are unchanged.
- **L8**: `<backend>.watch.poll.interval`, `imap.watch.idle.timeout` and `jmap.watch.push.ping` read through `pimalaya_config::duration`: integer seconds as before, or `"90s"`, `"5m"`, `"2h"`.
- pimalaya-cli and pimalaya-config move to 0.5.1 (Cargo.lock only); the spec's pimdir requirement names io-pimdir 0.13, the version Cargo.toml asks for.

No configuration key is renamed or removed.
