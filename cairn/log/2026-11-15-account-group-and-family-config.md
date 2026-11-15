---
cairn: log
change: account-group-and-family-config
landed: 2026-11-15
---

# The account group and the family's configuration sentences

The second pass of the 2026-11-15 CLI alignment round landed, on pimalaya-cli 0.5.1 and pimalaya-config 0.5.1. Spec moved: daemon (added The accounts can be listed; modified A watch runs from a TOML file, Every configured account, or a chosen one, The watch method belongs to the backend, The account can be checked before it is watched, Every printed output has a published schema, A pimdir calendar is watchable, A refused command line exits 2, A failure carries a stable code, The standard commands are the family's).

- `account check` checks an account and `account list` lists every account with the backends its build serves and the ones it leaves out, both on pimalaya-cli's shared outputs; `check` runs on as a hidden alias, its schema key kept.
- Every removed key or table is refused in pimalaya-config's one sentence, ending "see MIGRATION.md"; the keys refused did not change.
- The watch durations take `"90s"`, `"5m"` or `"2h"` beside integer seconds.
- The spec names io-pimdir 0.13, the version carillon builds on.
