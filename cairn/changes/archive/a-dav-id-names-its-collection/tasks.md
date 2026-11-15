---
cairn: tasks
change: a-dav-id-names-its-collection
---

# Tasks

- [x] io-webdav 0.9.1 in Cargo.lock, the lock diff held to that crate; Cargo.toml asks for "0.9.1"
- [x] src/dav.rs: `check` (io-webdav's `check_collection_id`, coded through `error::not_found_from`) at the top of `watch` and `probe`; `under` on io-webdav's `collection_path`, `path` folded into it; `named` shared with `missing`
- [x] Tests: `a_collection_id_naming_no_collection_sends_nothing` and `a_collection_id_is_sent_percent_encoded` fail with the check and the encoding neutralised and pass with them; the path assertions of `a_collection_path_is_read_under_its_base`, `a_calendar_named_by_its_id_is_watched_by_listing` and `a_404_on_the_collection_is_its_domain_not_found` take the trailing slash; src/alarm.rs `a_body_read_whole_keeps_its_alarm`
- [x] Fold the delta into cairn/spec/daemon.md, log entry, CHANGELOG under Unreleased
