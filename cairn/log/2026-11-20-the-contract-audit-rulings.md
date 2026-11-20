---
cairn: log
change: the-contract-audit-rulings
landed: 2026-11-20
---

# The contract audit rulings

carillon took its share of the CLI contract audit of 2026-11-20 and of the owner's rulings on it, against pimalaya-cli's unreleased working tree (a path patch until the kit's next release).

## What reaches users

- `alarm-fired` prints `"trigger": "2026-11-20T15:22:22Z"` where it printed the system's offset (CRL-X1); `$trigger` and `$at` in a hook keep the offset.
- `hook-failed` names `hook` and `id` (CRL-X4), its text `[a] on-message-add command failed for <id>: ...`.
- A key carillon read before 1.0 reads serde's "unknown field `<key>`" (CRL-X2, ruling A5); MIGRATION.md is gone, 0.x to 1.0 having no guide.
- An account-level `tls` table, inherited field by field by each backend's, and `tls.client.cert`/`tls.client.key` (ruling A1); pimalaya-stream 0.4 presents the client certificate under rustls and native-tls.
- `account check --json` prints `{"accounts": [{"account", "backends"}]}`, each row with `tls` (ruling A2).
- `carillon --json` alone exits 2 coded `usage-invalid`; `manual`, `completion` and `json-schema` without argument exit 2, refused by clap (pimalaya-cli).

## Capabilities moved

daemon (see the change's delta).

## Verified

`cargo test` (default features, and `--no-default-features --features rustls-ring,maildir` as CI runs it), every new test failing on the code before the change (run from a snapshot of HEAD with the new tests copied in, under `TZ=Europe/Paris` for the trigger), `cargo clippy --locked --all-targets -D warnings` over the 17 feature sets of .github/workflows/tests.yml, `cargo fmt`. The regenerated `json-schema` differs from the audit's dump in `carillon-account-check` and `carillon-check` (the list of accounts, `tls`) and `carillon-watch.events` (`hook-failed.hook`, `hook-failed.id`, the `trigger` description) alone.
