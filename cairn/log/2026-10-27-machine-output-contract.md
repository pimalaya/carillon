---
cairn: log
change: machine-output-contract
landed: 2026-10-27
---

# Adopted the shared machine output contract

pimalaya-cli is pinned at `55b6693` until 0.3.0. `json-schema` gained `carillon-error`, the schema of the JSON failure object, and each schema now carries `$id` = `https://pimalaya.org/schemas/carillon/v0.2/<key>.json`. A test asserts every payload root is an object; all already were, so no output moved. Piped `--json` is now one value per line, from the pimalaya-cli printer.

Verified: 78 tests pass with all features, clippy clean on all features and targets, `--no-default-features` builds.

Spec updated: cairn/spec/daemon.md (ADDED: The JSON contract is described and versioned).
