---
cairn: log
change: a-feature-named-as-its-block
landed: 2026-11-16
---

# A feature named as its block

Row L21 of the X3 check of the 2026-11-15 CLI alignment round landed, as ruled by D33 (2026-11-16). Spec moved: daemon (modified A watch runs from a TOML file, added the scenario A DAV backend the build leaves out).

- The `dav` cargo feature is split into `caldav` and `carddav`, each named as the block it builds, both default. `caldav` carries the alarms of a calendar (ceichas, ical-rs, jiff); `carddav` pulls io-http and io-webdav only. `--features dav` no longer builds: BREAKING for packagers, MIGRATION.md has the before and after. No configuration key changes.
- D33's line read carillon as CalDAV-only and renamed `dav` to `caldav`; carillon watches addressbooks too, so it splits as neverest does, which is the rule D33 states.
- A build leaving a backend out names the feature as the block: `backend_not_built` takes the block alone.
- Tests: src/backend.rs `every_backend_is_built_by_the_feature_named_as_its_block`, tests/backend_not_built.rs `a_dav_check_names_the_feature_of_its_block`, both run against a snapshot of the code before and failing there (`no caldav feature`, `--features dav`).
- CI's clippy sets build `caldav,rustls-aws` and each of `caldav`, `carddav` alone with rustls-ring.
