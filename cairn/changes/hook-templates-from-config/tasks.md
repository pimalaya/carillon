---
cairn: tasks
change: hook-templates-from-config
---

- [x] pimalaya-config 0.3 through a path patch; subst and notify-rust dropped
- [x] `NotificationTemplate` on every hook, checked at load, rendered at firing
- [x] camelCase envelope variables, notifications and command environments alike
- [x] `notify` cargo feature, `vendored` forwarded; the wizard writes no notification without it
- [x] Tests: a former spelling refused, a default not excusing an unknown name, an absent value empty or defaulted, a headless build refusing a `notify` table
- [x] `cargo fmt`, `cargo test --all-features`, `cargo clippy --all-features`, a build without `notify`
- [x] config.sample.toml, spec folded, log entry, CHANGELOG
- [x] The path patch replaced by a git pin on pimalaya-config 09f5060
- [ ] The git pin replaced by a pimalaya-config release (user)
