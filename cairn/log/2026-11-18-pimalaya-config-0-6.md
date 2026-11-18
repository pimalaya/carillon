---
cairn: log
change: pimalaya-config-follow-up
landed: 2026-11-18
---

# pimalaya-config 0.6, the hook timeout, io-pimdir 0.17

pimalaya-config 0.6.0, pimalaya-cli 0.6.1, io-jmap 0.10.1 and io-pimdir 0.17.0 were released on 2026-11-18; carillon takes them from crates.io, no patch.

- Notify escaping (CRL-3 closed): pimalaya-config's `NotificationTemplate::render` writes a variable as text, markup escaped on every Unix but macOS. daemon.md's "A hook failure never stops the watch" says so, with the scenario "A subject holding markup"; src/hook.rs `an_absent_variable_expands_to_nothing_or_its_default` expects the escaped sender there and the raw one elsewhere, and failed on 0.6.0 until it did.
- Hook `timeout`: `ItemHook` and `FlagHook` read `timeout: Option<Timeout>`, `HookJob` carries it, and src/hook/command.rs's reaper sends SIGTERM to the command's process group once it passes, SIGKILL after `Timeout::GRACE` (5 seconds), the failure reported as "Hook command timed out after <timeout>" whatever the exit status. The shutdown's grace of one second is unchanged. daemon.md's "which carillon does not read yet" became the requirement, with the scenario "A hook command past its timeout" and a sample line. Guards: src/config.rs `a_hook_reads_its_timeout` (an unknown key before), src/hook/command.rs `a_command_past_its_timeout_is_terminated_and_reported` and `a_command_outliving_its_timeout_grace_is_killed`, both failing with the reaper's expiry removed.
- `SecretError::Output { command, reason }`: carillon matches no `SecretError` variant, nothing moved; the CHANGELOG names the new message.
- `CommandConfig`'s `Display` is not used for the hook queue key: `{:?}` keeps a shell line and the program it would name apart, where `Display` would print both alike and merge their queues.
- io-pimdir 0.17 is pimdir draft-14: the pimdir backend reads draft-14 stores and refuses draft-13 ones (`store-version`). daemon.md's pimdir requirement names the draft; the guard stays tests/exit.rs's refused draft-11 store, a draft-13 one refused by the same check. `enqueue_with_body` now strips `X-PIMDIR-` properties, which only a src/pimdir.rs test calls, with bodies holding none.

Capabilities moved: daemon (hooks, notifications, pimdir backend).
