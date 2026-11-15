---
cairn: change
id: default-is-a-name
status: landed
created: 2026-11-15
---

# `default` is an account name like any other

## Why

The owner ruled on 2026-11-15 (item 24 of the family round) that no account name is special. Since the-family-line (d3f41c2) carillon resolved `-a` through pimalaya-config 0.4's `take_account`, which read `-a default` as the account marked default, in `watch` as `check` already did in 0.2. A name and a flag were then one word, and an account called `default` could not be named. pimalaya-config master d5a33cb (pushed, unreleased) reads `default` as a name: an absent or empty name takes the account marked default, and `AccountError::Missing` for `default` hints to omit `-a`. The three ways an account cannot be taken also carried no code, so a script could only match their wording.

## What

- Cargo.toml: pimalaya-config pinned to git d5a33cb in `[patch.crates-io]`; Cargo.lock moves its source line only.
- src/error.rs: `account-missing`, `account-default-missing` and `account-default-several` in `ErrorCode`, read off `AccountError` (`Missing`, `NoDefault`, `SeveralDefaults`) by a reader beside the backends', so the failure object carries them through `with_code`.
- Bare `watch` still watches every account and bare `check` takes the one marked default (src/watch.rs `select_accounts`, src/check.rs); `-a default` in either now takes the account named `default`.
- Spec, CHANGELOG (**BREAKING** for `check -a default`, which took the default in 0.2), MIGRATION.md, the codes in the json-schema test.
