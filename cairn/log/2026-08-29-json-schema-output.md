---
cairn: log
change: json-schema-output
landed: 2026-08-29
---

# Published the schema of what each command prints

## What landed

`carillon json-schema` (aliased `json-schemas`) generates the JSON Schema of a command's `--json` output, to stdout for one command or to `--dir` for every command at once. It is pimalaya-cli's `JsonSchemaCommand`, fed by a `src/json_schema.rs` registry keyed the way himalaya's is: the command path joined with hyphens, prefixed `carillon-`.

`CheckReport` is now `CheckOutput` and `GeneratedConfig` is now `ConfigureOutput`, both deriving `JsonSchema` beside the `Display` and `Serialize` they already had, and `BackendCheck` derives it too as the row `CheckOutput` holds.

Two entries, `carillon-check` and `carillon-configure`, which is every command that hands data to the printer.

## What is still true

`watch` is not in the registry and has nothing to put there: it reports through the hooks it fires and prints nothing. `manual` and `completion` write files rather than data, and are pimalaya-cli's own.

The config types derive no `JsonSchema`, as himalaya's do not: a schema describes what a command prints, and the TOML schema is what the sample documents.
