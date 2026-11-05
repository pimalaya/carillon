---
cairn: change
id: hook-shape-follows-siblings
status: active
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

One shape, `on-<subject>-<event>` with the subject left out when there is one, and the event a bare verb, is what comodoro and ortie already share. carillon is the one to move, being below 1.0, and before alarm-hooks gives it a new hook.

## What

- **Names**: the event becomes a bare verb. `on-message-add`, `on-message-remove`, `on-message-change`, `on-flag-add`, `on-flag-remove`, and the same three verbs under `on-card-`, `on-event-` and `on-task-`.
- **Table**: `hooks` under each backend, `hook` read as an alias.
- **Command**: `command`, `cmd` read as an alias.
- **Aliases**: every former name and key is read until 1.0, so a 0.2 configuration still loads; the sample, the spec, the JSON schema and every message use the new ones.
- **Templates and variables**: landed separately in hook-templates-from-config (pimalaya-config templates, camelCase variables, `${name:-default}`, the `notify` feature).
- **Open, for later breaking-change work** (user, 2026-11-05): whether commands keep receiving the former snake_case variables beside the camelCase ones until 1.0, a missing name reading as an empty string in a shell.

## Not done

- ortie moves to the template and camelCase variables in its own change, within its 3.0.0.
