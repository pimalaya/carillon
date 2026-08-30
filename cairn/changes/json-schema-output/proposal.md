---
cairn: change
id: json-schema-output
status: landed
created: 2026-08-29
---

# Publish the schema of what each command prints

## Why

`carillon --json` hands a consumer a payload it has to reverse-engineer from the source. himalaya publishes one JSON Schema per command through a `json-schema` subcommand, and every other Pimalaya CLI is moving onto it; carillon derived `JsonSchema` nowhere and shipped no registry.

## What

- `check` and `configure`, the two commands that hand data to the printer, return `CheckOutput` and `ConfigureOutput`, renamed from `CheckReport` and `GeneratedConfig` so the type says what it is, and both derive `JsonSchema` beside `Display` and `Serialize`.
- `src/json_schema.rs` maps `carillon-check` and `carillon-configure` to their schemas, and `carillon json-schema` (aliased `json-schemas`) writes one to stdout or one file per command into `--dir`.
- `watch` stays out of the registry: it reports through its hooks and prints nothing. `manual` and `completion` write files rather than data.
