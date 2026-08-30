---
cairn: log
change: camel-case-json-output
landed: 2026-08-29
---

# Printed JSON keys in camelCase

## What landed

`CheckOutput`, `BackendCheck` and `ConfigureOutput`, which is every type reaching `printer.out`, carry `#[serde(rename_all = "camelCase")]`. The `--json` output of every Pimalaya CLI is moving onto that convention, which matches the wire formats these tools wrap and never produces a hyphenated key a consumer has to quote in jq.

No key moved. Every field on the three types is a single word (`account`, `backends`, `backend`, `ok`, `error`, `name`, `default`, `document`), so camelCase renders exactly what serde's default snake_case already did, and the regenerated `carillon-check` and `carillon-configure` schemas are unchanged. What landed is the convention declared at the type rather than left to be inferred, so a field named in two words next is spelled right without anyone thinking about it. Nothing was aliased for compatibility: carillon is pre-1.0 and there is no second spelling to keep.

## What is still true

The TOML configuration is a different surface with a different convention, and the 36 `rename_all = "kebab-case"` attributes in `src/config.rs` stay as they are. A config key is read from a file a human writes; an output key is handed to a program.

`watch` is still out of the registry, printing nothing and reporting through its hooks, and `manual` and `completion` write files rather than data.
