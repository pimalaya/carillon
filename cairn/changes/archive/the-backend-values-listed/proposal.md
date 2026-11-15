---
cairn: change
id: the-backend-values-listed
status: landed
created: 2026-11-15
---

# The backend values listed

## Why

Line R9 of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), found by live testing that day: the help of `-b` gave the order `auto` follows without `pimdir`, though `-b pimdir` was accepted, and `-b nope` answered "Invalid backend nope" naming no value, `Backend` being parsed through its own `FromStr`.

## What

- `Backend` (src/backend.rs) derives clap's `ValueEnum`, as himalaya's does, its own `FromStr` dropped: the help lists `[possible values: auto, imap, jmap, maildir, caldav, carddav, msgraph, gmail, gcal, gpeople, pimdir]`, the long help each with its line, and a value outside them is refused naming them.
- Every backend is a value in every build, as in himalaya: a backend the build leaves out is still named by `-b`, a command reaching its block failing `backend-not-built`, which is what `-b` gave it before.
- The help of `-b` (src/cli.rs) ends its order with pimdir.
