---
cairn: delta
change: the-family-line
---

# Delta

Every requirement is in cairn/spec/daemon.md.

## ADDED Requirements

### Requirement: A first run points at the sample
Running `carillon` with no command SHALL print the help and exit 0, with a configuration or without one: carillon writes no configuration, and an account is written by hand against the documented sample, config.sample.toml. A command that finds no configuration SHALL fail, exit 1, naming the path it looked at (the first `-c` path when given, else the default one) and the sample; a person at a terminal SHALL meet pimalaya-cli's welcome above that failure, a script or a `--json` caller the failure alone. A configuration that fails to parse SHALL be an error rather than read as absent.

#### Scenario: A newcomer
- **GIVEN** no configuration file
- **WHEN** `carillon watch` runs
- **THEN** it exits 1 naming the path it looked at and config.sample.toml, and writes nothing

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. The account block SHALL keep the shape himalaya CLI and himalaya TUI read, and unknown keys SHALL be ignored there rather than refused, so an account can be recognised across the binaries. A whole file SHALL NOT be claimed to load in all three, since every backend block is strict on both sides and each has keys the other does not know.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hook.on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.12 reads (pimdir draft-11, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL poll the store's change cursor (pimdir STORAGE §4.5) every `pimdir.watch.poll.interval` (30 seconds by default), read again the items stamped above it and forget the deleted or retained ones, and read the whole collection again when the store was reset, a purge count moved or the collection itself moved. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, one holding no calendars, or a store of another draft SHALL fail the watch and `check` naming it.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45

## REMOVED Requirements

### Requirement: A first account is generated, never hand-written from scratch
The family removed its wizards (pimalaya-cli 0.4.0 removed the wizard kit): "A first run points at the sample" replaces it.
