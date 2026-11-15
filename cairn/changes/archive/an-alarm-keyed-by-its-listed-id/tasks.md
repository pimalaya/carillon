---
cairn: tasks
change: an-alarm-keyed-by-its-listed-id
---

# Tasks

- [x] io-webdav 0.10.0 in Cargo.lock, the lock diff held to that crate; Cargo.toml asks for "0.10.0"
- [x] src/dav.rs: `resource` through io-webdav's `id_from_location`, `refresh_alarms` naming each member once
- [x] Test `an_alarm_follows_a_member_whose_href_holds_an_escape` (src/dav.rs, the scripted server now serving `calendar-query` and `calendar-multiget`): fails on io-webdav 0.10 with the raw segment (the alarm stays at 08:45), passes with the fix; `AlarmShared::triggers` built for every alarm test
- [x] Hooks, state and `under` checked: no id changes spelling
- [x] Fold the delta into cairn/spec/daemon.md, log entry, CHANGELOG under Unreleased
