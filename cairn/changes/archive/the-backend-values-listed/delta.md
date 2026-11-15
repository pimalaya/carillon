---
cairn: delta
change: the-backend-values-listed
---

# Delta

The requirement is in cairn/spec/daemon.md, which named no rule for `-b` before.

## ADDED Requirements

### Requirement: The backend is picked by `-b`
`-b/--backend` SHALL take `auto`, its default, or one backend by name: `imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople` or `pimdir`, every one of them in every build, a backend the build leaves out failing coded `backend-not-built` once a command reaches its block. `auto` SHALL let `watch` take the first block the account declares in that order and `account check` check every one; a name SHALL narrow both to that block. The help SHALL state the order and list every value as clap's possible values, and a value outside them SHALL be refused as a command line the grammar refuses, naming the possible values.

#### Scenario: A backend no build knows
- **WHEN** `carillon -b nope watch` runs
- **THEN** it exits 2 before any configuration is read, its message naming `[possible values: auto, imap, jmap, maildir, caldav, carddav, msgraph, gmail, gcal, gpeople, pimdir]`

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
