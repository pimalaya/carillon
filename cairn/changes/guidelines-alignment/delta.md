---
cairn: delta
change: guidelines-alignment
---

## MODIFIED Requirements

### Requirement: Every configured account, or a chosen one
Bare `carillon watch` SHALL watch every configured account at once, one thread each under a single shared shutdown. `-a/--account` SHALL narrow the watch to that account. A name no account carries SHALL be an error listing the accounts the configuration does hold, and a command needing one account with no default to pick SHALL name both ways of choosing one, so a failure to resolve an account always says what to do next. One account's watch failure SHALL be logged and retried on its own without stalling the others.

#### Scenario: Watch everything
- **GIVEN** a config with two accounts and no account flag
- **WHEN** `carillon watch` runs
- **THEN** both accounts are watched at once, each on its own collection, and Ctrl+C stops them together

#### Scenario: A name no account carries
- **GIVEN** a configuration holding the accounts `perso` and `work`
- **WHEN** `carillon -a wrok watch` runs
- **THEN** it fails naming `wrok` and listing `perso, work`

#### Scenario: One account's server is unreachable
- **GIVEN** two watched accounts, one of whose servers refuses connections
- **WHEN** that watch fails
- **THEN** the failure is logged and retried for that account alone, and the other account keeps watching
