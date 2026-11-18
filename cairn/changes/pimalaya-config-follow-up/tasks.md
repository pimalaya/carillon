---
cairn: tasks
change: pimalaya-config-follow-up
---

## Once pimalaya-config 0.5.3 or 0.6 is released

- [ ] Bump `pimalaya-config` in `[dependencies]` and `[dev-dependencies]`, `cargo update -p pimalaya-config`
- [ ] Notify escaping: update the expected summary of src/hook.rs `an_absent_variable_expands_to_nothing_or_its_default`, state in the daemon spec that a variable reaches a notification as text, close CRL-3
- [ ] Hook `timeout`: read `timeout: Option<Timeout>` on `ItemHook` and `FlagHook`, carry it in `HookJob`, SIGTERM then SIGKILL after `Timeout::GRACE` in src/hook/command.rs, report a `hook-failed` naming it, with a test failing first
- [ ] Fold the `timeout` requirement into cairn/spec/daemon.md, replacing "which carillon does not read yet", and add its sample line
- [ ] `SecretError::Output`: grep for the old variant, expect none, name the new message in the CHANGELOG
- [ ] Optional: key the hook command queue with `CommandConfig`'s `Display`
- [ ] CHANGELOG line for the bump, log entry, archive this change

## Owner rulings

- [x] COH-4 row 1: ruled 2026-11-18, a Google daily limit stays `rate-limited` (`quota-exceeded` keeps its storage meaning); nothing moves in carillon, himalaya alone changes
- [x] Installers, `curl -f`: ruled 2026-11-18 and landed, install.sh downloads with `curl -fsSLo`
- [ ] Installers, checksums: once pimalaya/nix publishes them (ruled 2026-11-18, later)
