---
cairn: log
change: anonymous-dav-and-punycode-hosts
landed: 2026-11-17
---

# Anonymous DAV and Punycode hosts

Audit line COH-6, the owner's two rulings of 2026-11-17. Specs moved: daemon (added A DAV block may send no credential), packaging (new capability, added Hosts are ASCII).

- A DAV block with no `auth` already loaded and sent no `Authorization` header (`DavAuthConfig::None`, the default). A 401 to it failed with the bare `WebDAV server returned HTTP 401`: `dav::watch` and `dav::probe` now put "DAV server answered 401: it asks for credentials, and the account configures no `auth`" above it, the code left `null`.
- Cargo.toml pins `idna_adapter = { version = "=1.0.0", default-features = false }`, as Himalaya, Cardamum, Calendula and Ortie do. `cargo update -p idna_adapter` moved 1.2.2 to 1.0.0 and removed 21 crates from Cargo.lock: displaydoc, icu_collections, icu_locale_core, icu_normalizer, icu_normalizer_data, icu_properties, icu_properties_data, icu_provider, litemap, potential_utf, stable_deref_trait, synstructure, tinystr, writeable, yoke, yoke-derive, zerofrom, zerofrom-derive, zerotrie, zerovec, zerovec-derive.
- `config::parse_url` refuses a non-ASCII host with "Host `<host>` is not ASCII: write it in its Punycode form (xn--...)": `caldav.home` and `carddav.home` deserialize through it, `dav::open` parses `server` with it and `jmap::parse_server` too. BREAKING: such a host used to be turned into Punycode silently; MIGRATION.md has the before and after.
- Tests: src/config.rs `a_dav_block_without_auth_loads` (passing on the HEAD sources too, carillon already behaving) and `a_non_ascii_host_names_its_punycode_form`, src/dav.rs `a_401_without_auth_names_the_missing_auth` (a loopback server answering 401). The last two fail on a snapshot of the HEAD sources and lockfile (`https://bücher.example/jmap` parsed to host `xn--bcher-kva.example`; an error of `WebDAV server returned HTTP 401` alone) and pass after.
