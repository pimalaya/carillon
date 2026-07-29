---
cairn: tasks
id: enrich-notifications
---

# Tasks

- [x] Add io-imap as a direct CLI dependency (the resolver's own protocol client)
- [x] `net`: extract the shared TLS connect helper used by driver and resolver
- [x] `resolve`: `CarillonImapMessage` type + a generic io-imap pump + `resolve_new_messages` (greet, auth, EXAMINE, UID FETCH ENVELOPE over a fresh connection)
- [x] `driver`: track `resolver_uid_next`; on a change with a notify hook, resolve and pass the new messages to the hook
- [x] `hook`: fire one notification per new message with `$from` / `$from_name` / `$from_addr` / `$subject` / `$date` / `$count`; keep `cmd` raw
- [x] Update `config.sample.toml` notify templates and the documented variables
- [x] Update README and CHANGELOG
- [x] Fold the delta into `spec/daemon.md`; write the log entry; set status landed
- [x] `cargo check` / `clippy` / `fmt` green
