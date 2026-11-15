---
cairn: change
id: backend-left-out-loads
status: landed
created: 2026-11-15
---

# A backend the build leaves out loads

## Why

The owner ruled on 2026-11-15 (item 16 of the family round) that one configuration file serves every build: the table of a backend the build leaves out is accepted at load, and a command reaching it fails naming the cargo feature, while real typos stay refused. unknown-keys-are-refused had made carillon refuse such a table as `unknown field imap`, the open point it left.

## What

- src/config.rs: each backend field of `AccountConfig` has a twin under `cfg(not(feature))` typed `Option<serde::de::IgnoredAny>`, so the block loads unread and stays present; the account-level `proxy` likewise in a build without any network backend, where nothing can reach it. `deny_unknown_fields` still refuses any other key.
- src/error.rs: `backend-not-built` and `backend_not_built(backend, feature)`, "This build leaves out the `<backend>` backend, rebuild with `--features <feature>`".
- src/watch.rs: where `auto` or `-b` reaches such a block, in the priority order, the session fails with it. A build does not grow a backend, so the account is not retried (main session ruling, 2026-11-15): its `session-lost` event coded `backend-not-built` is printed once and its thread stops for the run, the other accounts watching on. When every account stopped so, the watch fails with that failure, exit 1.
- src/check.rs: such a block is a failed row coded `backend-not-built`, so `check` exits 3.
- Spec, CHANGELOG, MIGRATION.md.
