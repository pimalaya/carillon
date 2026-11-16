---
cairn: change
id: a-feature-named-as-its-block
status: landed
created: 2026-11-16
---

# A feature named as its block

## Why

Row L21 of the X3 check of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15/reports/x3-2026-11-15.md), ruled by the owner as D33 on 2026-11-16: a cargo feature matches its configuration table, as cardamum's `carddav` and calendula's `caldav` do, and the family has no alias features. carillon built both of its DAV blocks, `caldav` and `carddav`, under one `dav` feature, so a build leaving them out told the user to rebuild with `--features dav` for a block named otherwise.

## What

- Cargo.toml: `dav` becomes `caldav` (ceichas, ical-rs, io-http, io-webdav, jiff: the calendar and its alarms) and `carddav` (io-http, io-webdav), both in the defaults. No `dav` alias.
- build.rs: a `dav` cfg, set by either feature, gates what both kinds share (the dav module, the server, auth and watch tables, the poll, the refusal reader); `network` and `alarm` list the two features.
- src/config.rs, src/watch.rs, src/account/check.rs: each block, its hooks and its branch under its own feature.
- src/dav.rs: the alarm reading (`calendar-query`, `calendar-multiget`, the book) under `caldav`; the alarms move into `DavTarget`, a field only a `caldav` build has.
- src/error.rs: `backend_not_built` takes the block alone and names the feature as the block, which every backend now is.
- Tests: src/backend.rs `every_backend_is_built_by_the_feature_named_as_its_block` (every `-b` value is a default feature of Cargo.toml, and `dav` none); tests/backend_not_built.rs `a_dav_check_names_the_feature_of_its_block`. Both fail on the code before.
- CI (.github/workflows/tests.yml), CONTRIBUTING.md, README.md, the spec, CHANGELOG (BREAKING) and MIGRATION.md (packagers).

## Decisions

- **Split, not renamed to `caldav`.** D33's line says carillon's `dav` becomes `caldav` on the ground that carillon speaks CalDAV only; it reads a `carddav` block too and watches addressbooks (src/watch.rs, `carddav.hooks.on-card-*`). A `caldav` feature building the `carddav` block would break the rule D33 sets, so carillon splits as neverest does.
- **`carddav` alone reads no alarm.** Alarms are a calendar's, so a `carddav` build pulls neither ical-rs, ceichas nor jiff. The calendar half of the shared poll (its component detection) stays compiled in a `carddav` build, allowed dead there rather than gated item by item.
- **The refusal names the block.** With every feature named as its block, the second argument of `backend_not_built` always repeated the first, so it is gone.
- **No configuration key changes**: the `dav` table stays refused as before.
