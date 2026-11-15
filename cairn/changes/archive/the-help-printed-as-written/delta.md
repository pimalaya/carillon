---
cairn: delta
change: the-help-printed-as-written
---

# Delta

The requirement is in cairn/spec/daemon.md, after "A refused command line exits 2".

## ADDED Requirements

### Requirement: Help is plain text
`--help` and `-h` SHALL print without colour, each paragraph on one line, never wrapped to the terminal width, the top-level help ending with the Pimalaya footer (where to report bugs, how to sponsor). clap SHALL be built with himalaya's features, its defaults off: `derive`, `error-context`, `help`, `std`, `suggestions` and `usage`, neither `color` nor `wrap_help` (pimalaya-cli terminal.md, "Help is printed as written").

#### Scenario: A narrow terminal asking for colour
- **GIVEN** `COLUMNS=60` and `CLICOLOR_FORCE=1`
- **WHEN** `carillon --help` prints
- **THEN** the help of `-b` prints its second paragraph on one line, longer than 60 columns, and no escape sequence

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
