---
cairn: change
id: rename-to-carillon
status: landed
created: 2026-08-22
---

# Rename mirador to carillon

## Why

The tool grew into what Carillon names. It started as a mailbox watcher, which is what mirador meant; it now watches a calendar and an addressbook too, and it is the local half of a product whose hosted half already carries the Carillon name. Two names for one thing is one too many, and the one that stops being true is the telescope.

The rename comes last on purpose. Doing it before the merge would have made the history read as a rename that happened to absorb another tool; doing it after leaves a history that reads as what happened: mirador absorbed the carillon daemon, then took its name.

## What

- The crate, the binary, the config directory, the `CARILLON_CONFIG` variable, the systemd unit and every URL.
- The spec, which is current truth, and the user-facing docs.
- Not the log entries or the landed changes: they record what was true when they were written, and Cairn logs are immutable.

## Consequences

`carillon/cli`, the daemon this repository absorbed, is now a second repository claiming the name. It has nothing left that this one does not, so it is to be archived rather than kept in step.
