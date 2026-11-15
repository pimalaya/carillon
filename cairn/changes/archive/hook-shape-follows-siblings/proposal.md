---
cairn: change
id: hook-shape-follows-siblings
status: landed
created: 2026-11-05
---

# Hooks shaped as comodoro's and ortie's

## Why

The three hook-based projects disagree on every part of a hook:

| | comodoro | ortie | carillon |
| --- | --- | --- | --- |
| table | `hooks` | `hooks` | `<backend>.hook` (`hooks` alias) |
| name | `on-<subject>-<event>` (`on-timer-start`, `on-work-begin`) | `on-<event>` (`on-issue`, `on-refresh`) | `on-<subject>-<past participle>` (`on-message-added`) |
| command key | `command` | `command` | `cmd` |
| notification | pimalaya-config `notify` adapter | its own `NotifyConfig` on notify-rust | its own `NotifyConfig` on notify-rust |

One shape, `on-<subject>-<event>` with the subject left out when there is one, and the event a bare verb, is what comodoro and ortie already share. carillon is the one to move, and its 1.0 is when (owner, 2026-11-15).

## What

- **Names**: the event becomes a bare verb. `on-message-add`, `on-message-remove`, `on-message-change`, `on-flag-add`, `on-flag-remove`, and the same three verbs under `on-card-`, `on-event-` and `on-task-`.
- **Table**: `hooks` under each backend.
- **Command**: `command`, a pimalaya-config `CommandConfig` as comodoro's hooks hold one.
- **Former spellings refused** (owner, 2026-11-15, superseding the 2026-11-05 aliases until 1.0): a backend's `hook` table, a hook's `cmd` and every `-added`, `-removed` and `-changed` name are refused by name, pointing at what replaced them and MIGRATION.md, the way unknown-keys-are-refused refuses the account keys that moved (uninhabited `RemovedKeyConfig` fields whose `deserialize_with` fails). The sample, the spec, the README and every message use the new ones.
- **Templates and variables**: landed separately in hook-templates-from-config (pimalaya-config templates, camelCase variables, `${name:-default}`, the `notify` feature).
- **The snake_case variables** (open 2026-11-05): settled by the code, hook-templates-from-config having already moved commands to the camelCase names alone; 1.0 adds nothing back.

## Not done

- ortie moves to the template and camelCase variables in its own change, within its 3.0.0.

## Checked against the code (2026-11-15)

- The plan's "messages naming a hook (`check`) and the JSON schemas": `check` names no hook and `json-schema` describes the `--json` outputs, not the configuration, so neither had anything to rename. The messages that do name a hook are the load-time refusals of src/config.rs and the pimdir watch's, all renamed.
- The plan's "one test loading a 0.2 configuration through the aliases" became the opposite: a test that each former spelling is refused by name, failing on the code before.
- `HookCmd`, carillon's own `Command` wrapper rebuilding the command on every clone, gave way to `CommandConfig`, which keeps the shape as written and builds the command when it runs.
