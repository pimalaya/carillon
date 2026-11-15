---
cairn: tasks
change: watch-streams-events
---

# Tasks

- [x] Cargo.toml: pimalaya-cli git 51ce8b1 with `signal`, `ctrlc` dropped; Cargo.lock moves pimalaya-cli's source line and `ctrlc` between the two packages only
- [x] src/error.rs: local `with_code` and `chained` deleted, pimalaya-cli's called at both sites
- [x] src/stream.rs: the events, the result and the sender
- [x] src/watch.rs: `Shutdown`, the channel, the result; src/hook.rs: failures handed back
- [x] src/json_schema.rs: `carillon-watch` and its events registered, `watch` no longer silent; a test naming every event
- [x] tests/watch.rs (unix, maildir): `--json` events then one result line on SIGINT, exit 0; text events through a pipe
- [x] src/hook.rs test: a command exiting non-zero handed back
- [x] src/hook.rs: a hook command's stdout is carillon's stderr; tests/watch.rs `a_hook_printing_to_stdout_reaches_stderr`
- [x] Fold into spec/daemon.md, log entry, README, CHANGELOG, MIGRATION.md
