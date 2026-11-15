---
cairn: tasks
change: hook-under-backend
---

- [x] Give each backend its own hook table, holding only the events it reports, with `hooks` as an alias
- [x] Name the events after their domain: `on-message-*` for mail, `on-card-*`, `on-event-*`, `on-task-*`, `on-item-*` for an untyped collection
- [x] Fire `on-flag-added` and `on-flag-removed` once per flag, dropping `$flags` for `$flag`
- [x] Split the DAV backend into `caldav`, `carddav` and `dav`, sharing one server shape, and teach `-b/--backend` the three
- [x] Resolve a calendar's domain from `supported-calendar-component-set` at watch start, and remember it per href so a removal still knows what left
- [x] Give `WatchEvent` its domain and keep one hook runner
- [x] Drop the account-level table and hand the driver the active backend's hook
- [x] Document the per-backend events and variables in config.sample.toml, where the envelope ones are IMAP's alone
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify a card hook under `caldav` is refused at load, that a two-flag STORE fires twice, and that each backend still fires what it reports
- [x] Fold the delta into the spec and log the change
