---
cairn: delta
change: camel-case-json-output
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: Every printed output has a published schema
Every command handing data to the printer SHALL return a named `*Output` type deriving `Display`, `Serialize` and `JsonSchema`, and `carillon json-schema` SHALL publish the schema of each, keyed by the command path joined with hyphens and prefixed `carillon-`. Every type reaching the printer SHALL spell its keys in camelCase, declared as `rename_all` on the type, which is the convention of the `--json` output alone and not of the TOML configuration, whose keys stay kebab-case. A command that writes files rather than data SHALL stay out of the registry, and `watch` SHALL print nothing, reporting through its hooks alone.

#### Scenario: A consumer reading the check payload
- **GIVEN** `carillon json-schema carillon-check`
- **WHEN** it runs
- **THEN** the JSON Schema of the `--json` payload of `carillon check` is printed on stdout

#### Scenario: Every schema at once
- **GIVEN** `carillon json-schema --dir <DIR>`
- **WHEN** it runs
- **THEN** one file per command is written there, the directory being created if it is not already

## REMOVED Requirements
