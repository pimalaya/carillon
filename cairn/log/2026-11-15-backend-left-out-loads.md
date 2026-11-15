---
cairn: log
change: backend-left-out-loads
landed: 2026-11-15
---

# A backend the build leaves out loads

The block of a backend the build leaves out loads unread, and `watch` or `check` reaching it fails coded `backend-not-built`, naming the cargo feature. Spec moved: `daemon` (MODIFIED: A watch runs from a TOML file; Every configured account, or a chosen one; The daemon owns the connection lifecycle; The account can be checked before it is watched; A failure carries a stable code).

- src/config.rs: `Option<de::IgnoredAny>` twins of the ten backend fields under `cfg(not(feature))`, and of the account `proxy` under `cfg(not(network))`. A misspelt key is still refused.
- src/error.rs: `BackendNotBuilt` and `backend_not_built`; src/watch.rs and src/check.rs fail the block they reach with it, at its place in the `auto` order. `watch_account` prints that account's `session-lost` event (watch-streams-events) once and returns the failure rather than retrying it, the others watching on; when every thread returned one, `watch` fails with the first, coded, exit 1.
- tests/backend_not_built.rs (5 tests, compiled only without the `imap` feature, run with `--no-default-features --features rustls-ring,maildir`): all failed before, the file being refused as `unknown field imap`, but the `imapp` guard. With the retry put back by hand, `a_watch_over_every_account_loses_that_account_alone` (no second `session-lost` past the first backoff) failed and `a_watch_with_nothing_left_fails_backend_not_built` never ended.
