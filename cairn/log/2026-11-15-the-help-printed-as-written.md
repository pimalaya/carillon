---
cairn: log
change: the-help-printed-as-written
landed: 2026-11-15
---

# The help printed as written

Rows L13 and L10 of the X3 check of the 2026-11-15 CLI alignment round landed (line X7). Spec moved: daemon (added Help is plain text).

- clap is built with himalaya's features, its defaults off: the help prints each paragraph on one line whatever the terminal width, and without colour, where it wrapped at the terminal width (100 columns with none) and coloured itself on a terminal. Cargo.lock drops the nine crates `color` and `wrap_help` pulled.
- The `account` group opens on "Manage accounts defined in the TOML configuration file.", and `account list` on "List all accounts declared in the configuration.", carillon's own paragraphs kept after them.
- Tests: tests/help.rs `the_help_is_printed_as_written`, tests/account.rs `the_account_abouts_are_the_familys`.
