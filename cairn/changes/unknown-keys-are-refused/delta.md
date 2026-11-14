---
cairn: delta
change: unknown-keys-are-refused
---

# Delta

Every requirement is in cairn/spec/daemon.md.

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
The daemon SHALL read its accounts from a TOML config file, resolved from the paths `-c` names, one path per flag, repeated to layer files left to right and never split on `:`, else from the standard user paths; no environment variable names one. Each account SHALL carry at least one backend block (`imap`, `jmap`, `maildir`, `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`), and each block SHALL carry the collection it watches, how it watches, and the hooks it fires. Every table SHALL refuse a key it does not read, at every level, the root and the account included, naming the key and its line, so a file is not shared with himalaya CLI or himalaya TUI; a backend the build leaves out is such a key. A key carillon read before and moved SHALL be refused naming where it went and MIGRATION.md: the account-level `folder`, `mailbox` and `collection` (the backend's collection key), `watch` (`<backend>.watch`) and `hooks` (`<backend>.hook`), and the `dav` backend (`caldav` or `carddav`). The aliases the hooks are read under SHALL stay accepted.

#### Scenario: A local watch from a config file
- **GIVEN** a config describing one IMAP account with an `imap.mailbox` and an `imap.hook.on-message-added` notify hook
- **WHEN** the daemon runs and a message arrives
- **THEN** a desktop notification fires, with no network delivery and no account with any service

#### Scenario: A misspelt account key
- **GIVEN** an account holding `defualt = true`
- **WHEN** the configuration is read
- **THEN** it is refused naming `defualt` and its line, and no watch is started

#### Scenario: An account-level hooks table
- **GIVEN** an account holding `hooks.on-message-added.cmd`
- **WHEN** the configuration is read
- **THEN** it is refused naming `<backend>.hook` and MIGRATION.md
