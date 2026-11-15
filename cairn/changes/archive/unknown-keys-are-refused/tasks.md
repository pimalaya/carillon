---
cairn: tasks
change: unknown-keys-are-refused
---

# Tasks

- [x] src/config.rs: `deny_unknown_fields` on `Config` and `AccountConfig`; every other configuration table already carried it, none flattens
- [x] src/config.rs: `folder`, `mailbox`, `collection`, `watch`, `hooks` and `dav` refused by name on the account, pointing at MIGRATION.md
- [x] Tests failing on the code before: an unknown key at the root and on the account (with guards at the backend, TLS, SASL, watch, hook table and hook levels, already refused), each removed key refused by name, the same through `Config::load`
- [x] Tests: the aliases still load (`imap.hooks`, SASL `username` and `password`, the secret's `cmd`); the sample loads as written and none of its pimalaya-config `sample::variants` meets an unknown key
- [x] Cargo.toml: pimalaya-config's `sample` feature as a dev-dependency
- [x] Cargo.toml: pimalaya-cli patched to the local ../cli (79c0d9e), a commit of its own
- [x] `cargo fmt`, `cargo test --all-features`, clippy `-D warnings` over the default, all, empty and per-backend feature sets, `cargo deny check`, `sh cairn/verify.sh`
- [x] Fold the delta into spec/daemon.md, log entry, CHANGELOG under Unreleased (Changed, **BREAKING**), MIGRATION.md
