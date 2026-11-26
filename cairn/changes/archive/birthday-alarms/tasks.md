---
cairn: tasks
change: birthday-alarms
---

# Tasks

Each task lands with a test failing before it, and the gates (fmt, clippy over every CI feature set with `-D warnings`, `cargo test --locked --all-features`) green.

- [x] T1: the owner's two rulings (hook name, late rule): one hook with `$kind`, same local day plus `late-days` (2026-11-26).
- [x] T2: a date resolver: a card's `BDAY` and `ANNIVERSARY` (date, year-less, 29 February, the `VALUE=text` form ignored) to its firings over the horizon at `at` minus `days-before`, in the local zone. Unit tests per form.
- [x] T3: `pimdir`: read the watched addressbook's card bodies through the store, hold their firings, replace them on a change and forget them on a removal; scenarios over a scratch store.
- [x] T4: `carddav`: read the bodies of added and changed cards, the same picture; scenarios over the scripted DAV server.
- [x] T5: the hook: `on-birthday-trigger` with its variables, config keys `at` and `days-before`, refused on a backend that watches no addressbook; config.sample.toml lines under carddav and pimdir.
- [x] T6: the late rule and the restart rule, on ceichas's timer: a suspend over 09:00 fires on waking the same day, not the next; with `late-days = 2` a suspend over the whole day fires the next day with `$late` 1; a restart fires nothing twice.
- [x] T7: live, owner's grant only: a card with a birthday today on Fastmail CardDAV and through a neverest-synced pimdir store, the hook fires once. Run 2026-11-26 on 2d6beaa (release build): one `birthday-fired` and one hook line (`$age` 30, `$late` 0) over CardDAV and over pimdir (neverest v1.0.0 store), none after a restart; the card deleted and synced out.
- [x] T8: fold delta.md into cairn/spec/daemon.md, log entry, CHANGELOG line under `[1.0.0]`, README feature line.

Landed 2026-11-26 (log 2026-11-26-birthday-alarms): T2 to T6 and T8; T7 run live the same day on the owner's grant.
