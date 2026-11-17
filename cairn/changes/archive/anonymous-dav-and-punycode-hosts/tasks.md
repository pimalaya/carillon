---
cairn: tasks
change: anonymous-dav-and-punycode-hosts
---

# Tasks

- [x] Checked: `caldav.auth` and `carddav.auth` already default to `DavAuthConfig::None`, which sends no `Authorization` header (src/config.rs, src/dav.rs `auth`).
- [x] src/dav.rs: `unauthenticated` over `watch` and `probe`.
- [x] Cargo.toml `idna_adapter` pin, `cargo update -p idna_adapter` (21 crates leave Cargo.lock).
- [x] src/config.rs `parse_url`, `ascii_url` for `caldav.home` and `carddav.home`; src/dav.rs `open` and src/jmap.rs `parse_server` through `parse_url`.
- [x] Tests `a_dav_block_without_auth_loads`, `a_non_ascii_host_names_its_punycode_form` (src/config.rs), `a_401_without_auth_names_the_missing_auth` (src/dav.rs); the last two seen failing on the HEAD sources and lockfile.
- [x] Spec (daemon, packaging), CHANGELOG (Changed, Fixed), MIGRATION.md.
