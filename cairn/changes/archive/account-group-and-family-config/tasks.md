---
cairn: tasks
change: account-group-and-family-config
---

- [x] pimalaya-cli and pimalaya-config 0.5.1 in Cargo.lock, the lock diff held to those two crates
- [x] R1: src/account/ (mod.rs, cli.rs, check.rs moved from src/check.rs, list.rs), src/cli.rs (`account` group, hidden `check`, the welcome), src/config.rs `AccountConfig::blocks`, `backends`, `not_built`, src/json_schema.rs (`carillon-account-list`, `carillon-account-check`, `carillon-check` kept); tests in src/cli.rs, src/account/check.rs, src/json_schema.rs, tests/account.rs, tests/backend_not_built.rs, the other tests on `account check`
- [x] F41: the refusers of src/config.rs as `pimalaya_config::removed_key!` types; the refusal tests on the new sentence
- [x] L8: `IdleWatchConfig::timeout`, `PushWatchConfig::ping`, `PollWatchConfig::interval` through `pimalaya_config::duration`; `a_duration_takes_seconds_or_a_unit`, `a_push_ping_takes_seconds_or_a_unit`; config.sample.toml and README
- [x] io-pimdir 0.13 in the spec's pimdir requirement, src/error.rs and the CHANGELOG
- [x] Fold the delta into cairn/spec/daemon.md, log entry, CHANGELOG under Unreleased, MIGRATION.md under "From v0.2 to v1.0"
- [x] `cargo fmt --check`, `cargo test --all-features` and on the reduced build, clippy `--all-targets -D warnings` over the all, default and reduced feature sets, `cargo deny check`, `sh cairn/verify.sh`
