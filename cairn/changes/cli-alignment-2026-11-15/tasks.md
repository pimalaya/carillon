---
cairn: tasks
change: cli-alignment-2026-11-15
---

- [x] R2: src/main.rs and src/cli.rs meet a bare run as himalaya; `arg_required_else_help` dropped from `manual` and `completion`; tests/first_run.rs and tests/exit.rs, failing on the code before
- [x] R3: `cmd` the alias of a hook's `command` (src/config.rs), `refuse_cmd` gone; the refusal cases replaced by `a_hook_reads_cmd_as_its_command`
- [x] R4: the three not-found codes (src/error.rs `not_found`, `not_found_from`) raised by src/imap.rs, src/maildir.rs, src/dav.rs, src/jmap.rs, src/gmail.rs, src/gcal.rs, src/gpeople.rs, src/msgraph.rs and src/pimdir.rs; himalaya's pimdir relay table; a test per backend but JMAP, and tests/watch.rs reading the code of a missing Maildir folder
- [x] R5: src/stream.rs and src/event.rs sanitize what a text event prints; `a_text_event_sanitizes_what_it_was_sent`
- [x] R6: `caldav.home`, `carddav.home` (src/config.rs, src/dav.rs `open` and `locate`), `pimdir.account` (src/pimdir.rs `check_calendar`); config.sample.toml
- [x] R7: src/check.rs on `Fields`, compared at runtime with himalaya's `account check` on a scratch Maildir
- [x] R8: the abouts and himalaya's order (src/cli.rs), `vendored` in the defaults (Cargo.toml, package.nix), the spec sentence on `repl` and `mcp`
- [x] Fold the delta into cairn/spec/daemon.md, log entry, CHANGELOG under Unreleased, MIGRATION.md under "From 0.2 to 1.0" for R2 and the reversed R3 line
- [x] `cargo fmt --check`, `cargo test --all-features`, clippy `--all-targets -D warnings` over the all, default and reduced feature sets, `cargo deny check`, `sh cairn/verify.sh`
