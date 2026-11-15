---
cairn: tasks
change: hook-shape-follows-siblings
---

- [x] Proposal reviewed (owner, 2026-11-15): the verbs, and the former spellings refused at 1.0 rather than read as aliases
- [x] delta.md: every requirement of spec/daemon.md naming a hook, a `hook` table or `cmd`, as MODIFIED
- [x] src/config.rs: hook fields renamed to the bare verbs, `hooks` under every backend, `command` a `CommandConfig`; `hook`, `cmd` and the 14 former names refused by name through `RemovedKeyConfig` fields
- [x] Variables camelCase, the template and the `notify` feature: landed in hook-templates-from-config
- [x] Messages naming a hook use the new names (`check` and the JSON schemas name none)
- [x] config.sample.toml, README, tests (tests/common, tests/google.rs, tests/jmap.rs, tests/msgraph.rs, tests/exit.rs) on the new names; a test refusing each former spelling by name, failing on the code before
- [x] `cargo fmt`, `cargo test --all-features`, clippy `-D warnings` over the all, default and reduced feature sets, `cargo deny check`, `sh cairn/verify.sh`
- [x] Fold the delta, log entry, CHANGELOG under Unreleased (Changed, **BREAKING**, with the former names listed), MIGRATION.md
