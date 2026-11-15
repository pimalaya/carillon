---
cairn: log
change: hook-shape-follows-siblings
landed: 2026-11-15
---

# Hooks shaped as Comodoro's, the former spellings refused

carillon's hooks take the shape Comodoro's have, for 1.0 (owner, 2026-11-15: refuse rather than read the former spellings as aliases until 1.0). Spec moved: `daemon` (MODIFIED: every requirement naming a hook, a `hook` table or `cmd`, 18 in all; in substance A watch runs from a TOML file, A hook failure never stops the watch, The hooks belong to the backend and An event is named after its domain).

- src/config.rs: every backend's hook table is the `hooks` field, `#[serde(alias = "hooks")]` gone; the hook fields end in a bare verb (`on_message_add`, `on_flag_remove`, `on_card_change`…), `on_alarm_trigger` unchanged; `ItemHook` and `FlagHook` hold `command: Option<CommandConfig>` (pimalaya-config's, as comodoro's `TimerHook` does), carillon's `HookCmd` wrapper and its rebuilding `Clone` gone. The former spellings are refused by name through `Option<RemovedKeyConfig>` fields whose `deserialize_with` fails, as unknown-keys-are-refused refuses the keys that moved: `hook` on each of the ten backends ("The hook table `hook` is now `hooks`, see MIGRATION.md"), `cmd` on both hook shapes, and the 14 `-added`, `-removed` and `-changed` names on each hook table that read them in 0.2 (38 fields), one refuser each from the `refuse_renamed_hooks!` macro. The account-level `hooks` refusal now names `<backend>.hooks`. Every message naming a hook uses the new spelling.
- src/hook.rs runs `CommandConfig::to_command`; its logs say "command hook". src/watch.rs and src/pimdir.rs follow the field renames.
- config.sample.toml, README.md, tests/common/mod.rs, tests/exit.rs and the live suites (tests/google.rs, tests/jmap.rs, tests/msgraph.rs) use the new spelling. CHANGELOG: one **BREAKING** Changed line listing every former name, the Added lines of this release renamed; MIGRATION.md covers it with a before and after.
- Checked against the plan: `check` names no hook and the JSON schemas describe no configuration, so neither moved; the plan's alias-loading test became a refusal test; the snake_case variables the proposal left open were already gone since hook-templates-from-config.

Verified by `cargo test --all-features` (99 unit tests, 5 in tests/exit.rs, 9 in tests/first_run.rs, the live suites built and ignored), `cargo test` on the default and `rustls-ring,imap` sets, `cargo clippy --all-targets -- -D warnings` over the all, default, empty, `rustls-ring` and per-backend feature sets, `cargo deny check` and `sh cairn/verify.sh`. New tests in src/config.rs: `a_hook_spelling_before_1_0_is_refused_by_name` (`imap.hook`, `on-message-added`, `on-flag-removed`, `cmd` on an item and on a flag hook, CalDAV's `on-task-changed`, JMAP's `on-card-removed`), which fails on the code before, every case loading there; `the_sample_names_no_unknown_key` now also fails on a sample line naming a refused spelling. The 0.2.0 config.sample.toml was run through the built binary: refused on `imap.hook`, then loading once the MIGRATION.md one-liner rewrote it and its `${subject:...}` removal hook was dropped. No live suite was run.
