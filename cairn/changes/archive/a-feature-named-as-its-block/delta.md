---
cairn: delta
change: a-feature-named-as-its-block
---

# Delta

The requirement is in cairn/spec/daemon.md; it gains the feature naming rule and the DAV scenario.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI. A backend the build leaves out SHALL NOT be such a key: its block SHALL load unread, so one file serves every build, and a command reaching it SHALL fail coded `backend-not-built`, naming the cargo feature to build with, each backend built by the feature named as its block (`caldav` and `carddav` apart, no `dav` feature, `caldav` alone building the alarms of a calendar) (`account check` in that backend's row, `watch` as one lost session of that account, never retried). A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hooks`), and the `dav` backend (`caldav` or `carddav`). A hook spelling of before 1.0 SHALL be refused the same way, naming what it became and MIGRATION.md: a backend's `hook` table (`hooks`), and every hook name ending in `-added`, `-removed` or `-changed` (`-add`, `-remove`, `-change`). Every such refusal SHALL read pimalaya-config's sentence for a removed key (`toml::Removed`): "The `<name>` key is removed, <why>: <what to write instead>, see MIGRATION.md", `table` in place of `key` for a table. A hook's `command` SHALL also be read under `cmd`, its alias wherever the family configures a command, a secret's included.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hooks.on-message-add` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

#### Scenario: A misspelt account key
- **GIVEN** an account holding `defualt = true`
- **WHEN** the configuration is read
- **THEN** it is refused naming `defualt` and its line, and no watch is started

#### Scenario: A backend the build leaves out
- **GIVEN** a build without the `imap` feature and an account holding an `imap` block
- **WHEN** `carillon account check` runs
- **THEN** the configuration loads, the imap row reads `ok: false`, `code: "backend-not-built"` and names `--features imap`, and the command exits 3, while a misspelt `imapp` block is still refused when read

#### Scenario: A DAV backend the build leaves out
- **GIVEN** a build with neither the `caldav` nor the `carddav` feature, and an account holding a `caldav` block
- **WHEN** `carillon account check` runs
- **THEN** the caldav row reads `code: "backend-not-built"` and names `--features caldav`, not `--features dav`; with a `carddav` block, the row names `--features carddav`

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-add.command`
- **WHEN** the configuration is read
- **THEN** it is refused as "The `hooks` table is removed, the hooks being their backend's: write `<backend>.hooks` instead (`imap.hooks`), see MIGRATION.md"

#### Scenario: A hook named as in 0.2
- **GIVEN** an account holding `imap.hook.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused naming `hooks` and MIGRATION.md, and once that is fixed `on-message-add`, whose `cmd` then reads as its `command`

#### Scenario: A hook command spelt `cmd`
- **GIVEN** an account holding `imap.hooks.on-message-add.cmd = "true"`
- **WHEN** the configuration is read
- **THEN** it loads as `imap.hooks.on-message-add.command = "true"`, and a hook naming both is refused

## REMOVED Requirements

None.
