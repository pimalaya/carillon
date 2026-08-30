---
cairn: change
id: camel-case-json-output
status: landed
created: 2026-08-29
---

# Print JSON keys in camelCase

## Why

The `--json` output of every Pimalaya CLI is moving to camelCase keys, matching the wire formats these tools wrap (JMAP, the Google and Microsoft APIs, JSCalendar and JSContact) and sparing a consumer the `."key-name"` quoting jq needs for a hyphenated key. carillon is pre-1.0 and its output types carry no `rename_all` at all, so they emit serde's default snake_case and the convention is unstated in the source.

## What

- `CheckOutput`, `BackendCheck` and `ConfigureOutput`, which is every type reaching `printer.out`, carry `#[serde(rename_all = "camelCase")]`, so the convention is declared where the payload is defined rather than inferred from field names that happen to be single words.
- No compatibility alias: carillon switches now rather than carrying two spellings.
- The TOML configuration is untouched. The `rename_all = "kebab-case"` throughout `src/config.rs` is the config vocabulary, which is a different surface with a different convention.
