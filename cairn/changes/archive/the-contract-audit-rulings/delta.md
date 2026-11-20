---
cairn: delta
id: the-contract-audit-rulings
---

# Delta

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: A watch runs from a TOML file
A key carillon read before 1.0 is refused by serde alone, as any key it does not read, never by name nor with a pointer to MIGRATION.md. The scenarios *An account-level hooks table* and *A hook named as in 0.2* expect the unknown field.

### Requirement: The account can be checked before it is watched
The report is a list of one account, `{"accounts": [{"account", "backends"}]}`, each row carrying `tls` (`null` for a local backend or a cleartext connection), its text `, TLS <parts>`. New scenario *A backend inheriting the account's TLS*.

### Requirement: The TLS handshake is configured under its backend
An account MAY carry a `tls` table, which each backend's takes from, field by field; every `tls` table holds `provider`, `rustls.crypto`, `cert` and `client.cert`/`client.key`. New scenario *A backend table over the account's*.

### Requirement: An alarm hook templates against its firing
The hook variables keep the system zone's offset; the `alarm-fired` event's `trigger` does not.

### Requirement: `watch` streams events (the watch output)
`hook-failed` carries `hook` and `id`; `alarm-fired.trigger` is an instant in UTC with `Z`.

### Requirement: A first run points at the sample
A bare run answers through pimalaya-cli's `clap::meet_bare_invocation`: under `--json`, `usage-invalid`, exit 2.

### Requirement: The standard commands are the family's
Without argument, `completion`, `manual` and `json-schema` are refused by clap naming `--dir`, exit 2, `usage-invalid` under `--json`.

## REMOVED Requirements

None.
