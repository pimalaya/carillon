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

- [ ] COH-4 row 1: a Google daily limit, `rate-limited` (today) or `quota-exceeded`
- [ ] Installers: `curl -f` and a checksum across the family, with pimalaya/nix publishing checksums
