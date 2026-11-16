---
cairn: tasks
change: a-feature-named-as-its-block
---

# Tasks

- [x] Cargo.toml: `dav` split into `caldav` and `carddav`, both default, no alias
- [x] build.rs `dav` cfg; src/config.rs, src/watch.rs, src/account/check.rs, src/error.rs, src/main.rs, src/dav.rs gated by kind, alarms under `caldav`
- [x] `backend_not_built` names the feature as the block
- [x] Tests: src/backend.rs `every_backend_is_built_by_the_feature_named_as_its_block`, tests/backend_not_built.rs `a_dav_check_names_the_feature_of_its_block`, both failing on the code before; tests/account.rs gates on `caldav`
- [x] clippy `-D warnings` with `rustls-ring,caldav` and `rustls-ring,carddav` alone, and every set CI runs
- [x] CI feature sets, CONTRIBUTING.md, README.md, spec, log, CHANGELOG BREAKING line, MIGRATION.md entry
- [ ] An agent runs no git write command; the owning session commits
