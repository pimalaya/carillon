---
cairn: delta
id: multi-account-watch
---

## ADDED Requirements

### Requirement: The daemon watches every configured account, or a chosen subset
Bare `carillon watch` SHALL watch every account in the config concurrently,
one independent watch per account under a single shared shutdown. Account
names given as positional arguments SHALL narrow the watch to exactly those
accounts, and the daemon SHALL fail when a named account is absent from the
config. One account's watch failure SHALL be logged and recovered on its own
without stalling the others. Each account's watched mailbox SHALL come from
its own config (there is no mailbox flag), since accounts watch different
mailboxes.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no positional argument
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its configured mailbox,
  and Ctrl+C stops them together

#### Scenario: Watch a chosen subset
- **GIVEN** a config with three accounts
- **WHEN** `carillon watch work personal` runs
- **THEN** only those two accounts are watched, and an unknown name is an error
