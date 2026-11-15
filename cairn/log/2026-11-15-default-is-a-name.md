---
cairn: log
change: default-is-a-name
landed: 2026-11-15
---

# `default` is an account name like any other

`-a default` takes the account named `default`, no `-a` the account marked default, and the three ways of failing are coded. Spec moved: `daemon` (MODIFIED: Every configured account, or a chosen one; A failure carries a stable code).

- Cargo.toml: pimalaya-config pinned to git d5a33cb (`take_account` reading `default` as a name, `Missing` for `default` hinting to omit `-a`); Cargo.lock moved its source line only.
- src/error.rs: `AccountMissing`, `AccountDefaultMissing` and `AccountDefaultSeveral`, spelled `account-missing`, `account-default-missing` and `account-default-several`, read off pimalaya-config's `AccountError` by `account_code`, the first reader of the chain walk, so `with_code` rebuilds the failure object over them.
- tests/account.rs (5 tests, offline, through the binary under `--json`): each code, `watch` and `check` alike for `account-missing`; `-a default` with only `perso` marked default refused naming the hint; an account named `default` taken by name. Each failed before: no code was given, and `-a default` took `perso`.
