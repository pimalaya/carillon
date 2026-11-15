---
cairn: change
id: watch-streams-events
status: landed
created: 2026-11-15
---

# A watch streams what it observes

## Why

pimalaya-cli 51ce8b1 (pushed) adds the family requirement "A long-lived command streams events" (its cairn/spec/output.md): a watch prints one event line per thing it observes, then exactly one result line; SIGINT or SIGTERM stops it cleanly, exiting 0; its text events reach a pipe. It ships the pieces: `signal::Shutdown`, `StdoutPrinter::streaming` and `JsonSchemaRegistry::insert_events`, and a shared `error::with_code`. carillon's `watch` printed nothing, reported through its hooks and logs alone (a hook failure and a lost session were `warn` lines on stderr), and held its own `ctrlc` handler; a status bar or a supervisor had nothing to read.

## What

- Cargo.toml: pimalaya-cli pinned to git 51ce8b1 with its `signal` feature; carillon's own `ctrlc` dependency is gone (pimalaya-cli holds the one handler).
- src/stream.rs: `WatchStreamEvent` (`session-opened`, `session-lost`, `changed`, `hook-failed`, `alarm-fired`), `WatchOutput` (`{"stoppedBy": "signal"}`), and `WatchEvents`, the sender an account thread and its alarm thread report through.
- src/watch.rs: `Shutdown::install` replaces the `ctrlc` handler; the account threads send their events over a channel to the command's thread, which alone holds the printer, polls the request every 100 ms, sets the internal flag and rings the alarm wakers, joins, then prints the result. A stdout no one reads stops the watch the same way and fails the command. The `session lost` warning and the `watching` info lines became events.
- src/hook.rs: `run` and `run_alarm` hand back the reactions that failed (`HookFailure`), a command exiting non-zero being one, where they were logged. A hook command's stdout is carillon's stderr, so nothing but the events and the result reaches the watch's stdout.
- src/event.rs: `WatchEvent` and `WatchDomain` serialise (tagged `kind`, kebab-case), with a text form.
- src/main.rs: the printer streams for `watch`; src/json_schema.rs registers `carillon-watch` and `carillon-watch.events`.
- Item 13 of the round, in the same pass: the local `error::with_code` and its `chained` helper are deleted, src/main.rs and the imap test calling `pimalaya_cli::error::with_code(err, error::code)`; no behaviour moves.
- Spec, README, CHANGELOG, MIGRATION.md.

## Open

- Resolved the same day (cairn/log/2026-11-15-session-opened-once-established.md): `session-opened` waits until the session is established, so a server refusing it prints `session-lost` alone. Still open: an IMAP mailbox the server refuses to EXAMINE prints `session-opened` then `session-lost`, io-imap examining it on its worker without reporting readiness.
