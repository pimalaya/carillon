---
cairn: change
id: the-help-printed-as-written
status: landed
created: 2026-11-15
---

# The help printed as written

## Why

Rows L13 and L10 of the X3 check of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15/reports/x3-2026-11-15.md), line X7.

- **L13**: Cargo.toml took clap's default features plus `wrap_help`, so the help wrapped at the terminal width (100 columns with no terminal) and coloured itself on one, against pimalaya-cli's terminal.md, "Help is printed as written": each paragraph on one line, nothing asking the terminal for its size, a binary wanting colour or wrapping enabling it on its own clap line. himalaya, ortie, cardamum, calendula and neverest all take clap without its defaults.
- **L10**: the `account` group opened on "Inspect the accounts the configuration declares", where himalaya, ortie and cardamum read "Manage accounts defined in the TOML configuration file", and `account list` on "List the accounts declared in the configuration", where himalaya and cardamum read "List all accounts declared in the configuration".

## What

- Cargo.toml: clap with `default-features = false` and himalaya's features, `derive`, `error-context`, `help`, `std`, `suggestions`, `usage`. Cargo.lock drops what `color` and `wrap_help` pulled (anstream, anstyle-parse, anstyle-query, anstyle-wincon, colorchoice, is_terminal_polyfill, once_cell_polyfill, terminal_size, utf8parse), no version moving.
- src/account/cli.rs, src/account/list.rs: himalaya's first lines, carillon's own paragraphs kept after them.
- tests/help.rs `the_help_is_printed_as_written`: `--help` under `COLUMNS=60` and `CLICOLOR_FORCE=1` prints the second paragraph of `-b` on one line and no escape sequence; it fails on the code before. tests/account.rs `the_account_abouts_are_the_familys`: the two abouts, `ls` listed as the alias of `list` and listing as it does; it fails on the code before.
- src/cli.rs `the_backend_flag_lists_every_backend`: its whitespace folding, there because the help wrapped, is gone, and the long help's value line reads as clap aligns it (`- pimdir:  Force pimdir`).
- cairn/spec/daemon.md: "Help is plain text", himalaya's, cardamum's and calendula's requirement, with the clap features.
- CHANGELOG: one line under Changed.

## Decisions

- **Colour goes with the wrapping.** Taking himalaya's feature list drops `color` too: pimalaya-cli's terminal.md says what it prints for a human is plain text, and every other family binary already builds clap without it. A refused command line under `--json` was already printed without styles.
- **The abouts are not specified.** The spec names commands and outputs, not help wording, so only the help requirement moves.
- **`account check` keeps its about** ("Validate the account configuration"): row L10 names the group and its list only.
