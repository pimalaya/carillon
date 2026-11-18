---
cairn: change
id: pimalaya-config-follow-up
status: active
created: 2026-11-18
---

# The pimalaya-config release, and what waits on the owner

## Why

pimalaya-config's master (8f3ebbd, read on 2026-11-18) holds four changes carillon must answer once they ship in 0.5.3 or 0.6, and carillon does not git-pin the crate to take them early. The family analysis of 2026-11-15 (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md) also left carillon two lines that need an owner ruling rather than a patch. Both lists lived in a session's chat; this change keeps them in the repository until they land.

## What, once pimalaya-config is released

- Notify escaping (KIT-3, carillon's CRL-3; config 4678c0b): `NotificationTemplate::render` writes a value as text, `&`, `<`, `>`, `"` and `'` escaped where the daemon reads markup (XDG platforms). The src/hook.rs test `an_absent_variable_expands_to_nothing_or_its_default` asserts the raw `" from Alice <alice@example.org>"` and takes the escaped form, by `render`'s platform rule. CRL-3 then closes for carillon: a CHANGELOG line, and the daemon spec saying a variable reaches a notification as text.
- The hook `timeout` (config 07e2c37, `command::Timeout`): `ItemHook` and `FlagHook` refuse unknown keys, so `timeout` stays refused until they read `timeout: Option<Timeout>`. `HookJob` carries it, and src/hook/command.rs's reaper sends SIGTERM to the command's process group once it passes, SIGKILL after `Timeout::GRACE` (5 seconds), and reports a `hook-failed` naming the timeout. The shutdown's own grace of one second does not move. The daemon spec's "a per-hook `timeout` belongs to pimalaya-config's command table, which carillon does not read yet" becomes the requirement, with a sample line and a CHANGELOG entry.
- `SecretError::Output { command, reason }` (COH-1, breaking in pimalaya-config; config 8f3ebbd): carillon matches no `SecretError` variant, so no code moves; a failing secret command now ends its message with ", run `<command>` yourself to see why", which the CHANGELOG line of the bump names.
- `Display` for `CommandConfig`: optional; `WatchEvents::fired` keys a command's queue with `{:?}`, which `Display` would replace to no visible effect.
- TOML parse errors no longer print the source line: no carillon test reads that text, nothing moves.
- The bump: `pimalaya-config` in `[dependencies]` and `[dev-dependencies]` (the `sample` feature), and one CHANGELOG line naming the items above.

## What waits on the owner

- COH-4, row 1: which code a Google daily limit takes. carillon's `error::google_code` codes a 429, or a 403 naming `rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded`, `dailyLimitExceeded` or `RATE_LIMIT_EXCEEDED`, `rate-limited`, as cardamum, calendula and neverest do; himalaya alone, through io-gmail's `is_rate_limited`, makes a daily limit `permission-denied`. Ruled `quota-exceeded`, `dailyLimitExceeded` and `quotaExceeded` leave `names_rate_limit` for a `quota-exceeded` arm, a code change marked BREAKING before 1.0.
- PKG-2's remainder: every family install.sh downloads with `curl -sLo`, no `-f`, so a 404 page is saved and `tar` then fails, and verifies no checksum. carillon's is neverest's with the name changed (cairn/log/2026-11-18-the-installer-fetches-the-release.md); failing on an HTTP error and checking a published checksum is one move across the family's installers and pimalaya/nix's release workflow, which publishes no checksum today.
