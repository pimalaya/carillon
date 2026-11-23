---
cairn: log
change: follows-storage-draft-17
landed: 2026-11-23
---

# carillon follows storage-draft-17

carillon reads the pimdir stores neverest writes on io-pimdir master (storage-draft-17), pinned by `[patch.crates-io]` to io-pimdir 17d4ea8 (0.20.0 release candidate, the io-pimdir lane's drop 3), the requirement already `version = "0.20.0"`, until io-pimdir 0.20.0 is released. Plan and evidence: cairn/changes/archive/follows-storage-draft-17/ (proposal.md, tasks.md, inventory.md, journal.md, report.md).

## What landed

- 7618eb3: the io-pimdir pin, Cargo.lock moving io-pimdir's entry alone (git source, `libc` added); the nix build vendors the git source through pimalaya/nix's `importCargoLock`.
- 3ae671f: url 2.5.9 and form_urlencoded 1.2.3, so `cargo deny check` passes again (RUSTSEC-2026-0592).
- d7e9b1f: a store of another draft is refused coded `store-version` in io-pimdir's `PimdirFormatRefusal` wording naming carillon and the draft it reads, where it said "this program".
- 7d8354b: the watch opens the store again at every poll, so a store moved aside and synced anew under `pimdir.root` is read on the next poll (it was never read: the reader held the old file); `AlarmFiring::key` no longer holds the item id, so a firing read again under a new id fires nothing twice (it fired again once a new store numbered the event anew).
- 91c8b5f: config.sample.toml names `~/.local/state/neverest`, the store neverest syncs.
- 96908db: a calendar listed again (reset, purge, calendar moved) reads only the bodies new or of another hash, through `alarms.update`, no second book built.
- 79c97a9: the polled backends' sleep, the reconnect backoff and the idle hook reaper wait on a `Condvar` the shutdown rings, in place of 100 and 200 ms steps; `poll` is compiled in every build.
- 6c29544 and b20c873: the pin moved to the io-pimdir lane's drops 2 and 3, the lock moving io-pimdir alone.
- d0b5e3d: `property-invalid` relayed (D4).
- 694709e: CHANGELOG, README's start-over paragraph, the spec fold.

## Spec

cairn/spec/daemon.md: "A pimdir calendar is watchable" (storage-draft-17, the reopen at each poll, the listing reading only what moved, the refusal naming carillon), "Alarms are held in memory" (a firing keyed without the item id), "Ctrl+C is prompt on every path" (no sleep steps), "A failure carries a stable code" (`store-version` wording, `property-invalid`).

## Verified

`cargo test --all-features` (193 passed, 9 ignored), clippy `-D warnings` over the 17 CI feature sets, `cargo test --no-default-features --features rustls-ring,maildir` (70 passed), `cargo fmt --check`, `cargo deny check`, `nix build .#default` and `nix flake check`, at 17d4ea8. Measures before and after in report.md.

Cross-app, on the local Radicale under the servers lock, neverest and carillon on 17d4ea8: carillon reads the calendar neverest synced into the shared store (one item holding an alarm). On a calendar of its own synced by neverest into a scratch store, an event's alarm fired once; the store was moved aside and synced anew while carillon ran, holding that event and a new one: the next poll read the new store, the new event's alarm fired, and the first never fired again.

## Left

- The purge poll at 10,000 events costs 2 to 3 ticks, over the 5% of the open the plan asked: what is left is io-pimdir's `list_items` listing (needs-io-pimdir.md N4).
- The release pin (io-pimdir 0.20.0) and carillon's release wait on the owner (tasks.md K).
