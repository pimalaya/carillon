---
cairn: tasks
change: hook-shape-follows-siblings
---

- [ ] Proposal reviewed (user): the verbs, aliases until 1.0
- [ ] delta.md: every requirement and scenario of spec/daemon.md naming a hook, a `hook` table or `cmd` (39 hook names), as MODIFIED
- [ ] src/config.rs: hook fields renamed, former names as serde aliases; `hooks` primary under every backend; `command` primary, `cmd` alias
- [x] Variables camelCase, the template and the `notify` feature: landed in hook-templates-from-config
- [ ] Messages naming a hook (refusals, `check`) and the JSON schemas use the new names
- [ ] config.sample.toml, README, tests (tests/google.rs, tests/jmap.rs, tests/msgraph.rs) on the new names; one test loading a 0.2 configuration through the aliases
- [ ] `cargo fmt`, `cargo test --all-features`, `cargo clippy --all-features`, `cargo build --no-default-features`
- [ ] Fold the delta, log entry, CHANGELOG under Unreleased (Changed, with the former names listed)
