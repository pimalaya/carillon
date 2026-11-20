---
cairn: tasks
change: the-contract-audit-rulings
---

- [x] src/stream.rs: `alarm-fired.trigger` in UTC, `hook-failed` with `hook` and `id`; src/event.rs `WatchEvent::hook`; unit test `an_alarm_fires_at_its_utc_instant`, tests/watch.rs
- [x] src/config.rs and src/config/*.rs: every `removed_key!` type and field removed; test `a_key_carillon_no_longer_reads_is_an_unknown_field`
- [x] src/config/tls.rs: `client`, `inherit`, `check`; src/config.rs: account `tls` and `inherit_tls`; test `the_account_tls_is_inherited_field_by_field`
- [x] src/account/check.rs: the list of accounts, `tls` per row; tests/account.rs `a_check_row_names_the_tls_its_backend_inherited`, tests/exit.rs, tests/backend_not_built.rs
- [x] src/main.rs, src/cli.rs: the kit's bare invocation; tests/first_run.rs, tests/exit.rs
- [x] config.sample.toml: account `tls`, `imap.tls.client`
- [x] spec daemon.md, CHANGELOG, MIGRATION.md deleted, log
