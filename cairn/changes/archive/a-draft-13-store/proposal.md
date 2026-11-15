---
cairn: change
id: a-draft-13-store
status: landed
created: 2026-11-15
---

# A draft-13 store

## Why

Line X14 of the family's CLI alignment round (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), the dependency pass. pimalaya-cli 0.6.0, io-imap 0.11.2, io-gpeople 0.8.1, io-msgraph 0.9.2 and io-gcal 0.5.1 are on crates.io, and the frontends read pimdir draft-13 through io-pimdir 0.14.0, released the same night. A draft-13 store refuses draft-11 code, so a pimdir watch over the store neverest now writes would fail with `store-version`.

## What

- Cargo.toml: pimalaya-cli `0.6.0` (dependency and build-dependency), io-imap `0.11.2`, io-gpeople `0.8.1`, io-msgraph `0.9.2`, io-gcal `0.5.1`, io-pimdir `0.14.0` (pimdir draft-13), with no `[patch.crates-io]` section. Cargo.lock moves those six crates alone.
- The pimdir watch reads draft-13 stores and refuses a draft-11 one coded `store-version`. Its reader calls (`PimdirReader::open`, `change_cursor`, `list_items`, `items_changed_since`, `collections_changed_since`, `get_item`, `PimdirBlobs::get`) and `PimdirError` did not move between io-pimdir 0.13.0 and 0.14.0: no code changes.
- tests/exit.rs `a_check_over_a_store_of_another_draft_is_coded_store_version` stamps the store `storage-draft-11`, where it stamped `storage-draft-08`: it fails on the code before, which read draft-11.
- cairn/spec/daemon.md, "A pimdir calendar is watchable": draft-13, io-pimdir 0.14.
- CHANGELOG (the pimdir backend line, one line per bump under Changed), MIGRATION.md and config.sample.toml name draft-13.

## Decisions

- **No breaking line.** The `pimdir` backend is new since 0.2, so the draft it reads moves in its Added line and in MIGRATION.md's git-build note, not under a BREAKING line.
- **pimalaya-cli 0.6.0 moves no output.** Its one change is `listing::Listing`, which carillon does not print.
- **io-gcal 0.5.1, io-msgraph 0.9.2 and io-gpeople 0.8.1 move no watch.** Their changes are series writes (`GcalEventSeries`, `MsgraphEventSeries`), `GcalEvent::to_ical` and `GpeoplePerson::unremovable_properties`, none of which carillon calls.
