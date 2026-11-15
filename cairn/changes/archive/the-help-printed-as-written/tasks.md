---
cairn: tasks
change: the-help-printed-as-written
---

# Tasks

- [x] Cargo.toml: clap with himalaya's features, its defaults off; Cargo.lock drops what `color` and `wrap_help` pulled, no version moving
- [x] src/account/cli.rs, src/account/list.rs: himalaya's first lines for the `account` and `account list` abouts
- [x] Tests: tests/help.rs `the_help_is_printed_as_written`, tests/account.rs `the_account_abouts_are_the_familys`, each failing on the code before; src/cli.rs `the_backend_flag_lists_every_backend` reads the help unfolded
- [x] Add the requirement to cairn/spec/daemon.md, log entry, CHANGELOG under Changed
- [ ] An agent runs no git write command; the owning session commits
