---
cairn: tasks
change: default-is-a-name
---

# Tasks

- [x] Cargo.toml: pimalaya-config git d5a33cb under `[patch.crates-io]`, `cargo fetch`, Cargo.lock moving pimalaya-config's source line only
- [x] src/error.rs: the three account codes and their reader
- [x] tests/account.rs: through the binary under `--json`, each code (`watch` and `check` for `account-missing`), `-a default` refused with the omit-`-a` hint while an account marked default exists, an account named `default` taken by name
- [x] src/json_schema.rs: the account codes in the failure schema test
- [x] Fold into spec/daemon.md, log entry, CHANGELOG, MIGRATION.md
