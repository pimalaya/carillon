---
cairn: tasks
change: camel-case-json-output
---

- [x] Add `rename_all = "camelCase"` to `CheckOutput`, `BackendCheck` and `ConfigureOutput`
- [x] Leave the `src/config.rs` kebab-case attributes alone
- [x] Regenerate the schemas and read the `carillon-check` and `carillon-configure` properties
- [x] Fold the delta into the spec and log the change
