---
cairn: tasks
change: follows-storage-draft-17
---

Every build: `nix develop --command`, `CARGO_TARGET_DIR=~/code/pimalaya/.lanes/carillon/target CARGO_PROFILE_DEV_DEBUG=0 taskset -c 8,9,24,25 cargo ... -j 4`. "Suite" below is `cargo test --all-features` (188 passed, 9 ignored at the start). Commit unsigned after each gated step (frame §4), decisions timed in journal.md; journal.md and report.md open with `cairn: change` frontmatter (`part: journal`, `part: report`), cairn/verify.sh accepting no other type. Line numbers are at ed02e89: re-read each file first.

## 0. Start

- [ ] 0.1 `git status` clean, HEAD ed02e89 or a descendant; if src/pimdir.rs, src/alarm.rs, src/poll.rs or tests/exit.rs moved, re-locate the lines below. Gate: suite as at the start.

## A. Dependency and build

- [ ] A1 Cargo.toml: append the frame §2 `[patch.crates-io]` io-pimdir line, no comment; run `cargo build`, never `cargo update -p io-pimdir`. Gate: `git diff Cargo.lock` is io-pimdir's entry alone (git source, no checksum, `libc` added); suite 188/9. Commit `build: follow io-pimdir 7f16b27 (storage-draft-17)`.
- [ ] A2 The CI matrix (.github/workflows/tests.yml `features`): clippy `--locked --all-targets -D warnings` over the 17 sets, then `cargo test --locked --no-default-features --features rustls-ring,maildir`. Gate: 17 exits 0, 70 passed (as on the snapshot, inventory.md).
- [ ] A3 (D6) `cargo update -p url --precise 2.5.9`. Gate: lock moves url and form_urlencoded alone; `cargo-deny check` passes (fails at HEAD on RUSTSEC-2026-0592). Commit `build: take url 2.5.9 (RUSTSEC-2026-0592)`.
- [ ] A4 `nix build .#default --cores 4 --max-jobs 1` and `nix flake check` from the repository. Gate: both exit 0, the git source fetched by `importCargoLock`.

## C. Refusal (B1)

- [ ] C1 Test first, tests/exit.rs:113-157: stamp `storage-draft-15` (the draft io-pimdir 0.19.1 wrote, held by users) in place of `storage-draft-11`; assert the row's `error` names `carillon` and `io_pimdir::sql::FORMAT_VERSION`; add a later-draft case (`storage-draft-99`, "upgrade carillon"). Gate: fails on "this program".
- [ ] C2 src/pimdir.rs `open` (:91-96): map `PimdirError::Version { found }` to a `CodedError` of `ErrorCode::of_pimdir` whose message is `PimdirFormatRefusal { found, own: FORMAT_VERSION, program: "carillon" }`, other errors kept, as cardamum/src/pimdir/client.rs:95-113. Gate: C1 passes, suite. Commit `fix(pimdir): name carillon in a refused store's message`.

## G. Bugs

- [ ] G1 (B2) Test first in src/pimdir.rs, `#[cfg(unix)]`: watch a scratch store at a 100 ms interval, rename its root aside, create a new store at the root holding an event alarming at another trigger; assert `alarms.triggers` shows it within two seconds. Gate: fails at ed02e89 (the new trigger never appears).
- [ ] G2 (B2, D2) src/pimdir.rs `watch` (:59-80): open the reader again at each poll and run `follow` on it, so another `store_id` reads as the reset it is; a failed open ends the session as any error does. Gate: G1 passes, suite, tests/exit.rs.
- [ ] G3 (B5) Tests first: src/alarm.rs, a book replaced by the same body under another key after the firing fires nothing on `pass(moved = true)`; src/pimdir.rs, the B5 shape (store replaced after a firing, next session reads it under a new seq). Gate: both fail at ed02e89 (two firings).
- [ ] G4 (B5, D3) src/alarm.rs `AlarmFiring::key` (:90-98): drop `self.id`. Gate: G3 passes, dav.rs alarm tests unchanged, suite. Commit G1 to G4 as `fix(pimdir): read a store synced anew under the watch, fire each alarm once`.
- [ ] G5 (B6) config.sample.toml:152-159: `pimdir.root = "~/.local/state/neverest"` "the store neverest syncs, on Linux" (neverest/src/sync/driver.rs:160-162, cardamum and calendula samples); `pimdir.calendar` "its id as `pimdir collection list` prints it". Gate: `the_sample_names_no_unknown_key`. Commit `docs(sample): point the pimdir block at neverest's store`.

## H. Performance

- [ ] H1 (P1) Test first in src/pimdir.rs: after a purge in another collection, the next poll reads no calendar body (count reads through a `body` counter under `#[cfg(test)]`, or delete a blob file and assert its alarm stays). Gate: fails at ed02e89 (every body read).
- [ ] H2 (P1) src/pimdir.rs: keep a picture seq to object hash beside the book; on a reset, a purge or the collection moving, list the collection and read only the items new or of another hash, forget the missing ones, through `alarms.update`. Gate: H1, suite; perf.sh purge poll at 10k events at most 5% of the 10k open's CPU.
- [ ] H3 (P2, D5) src/poll.rs `sleep` (:23-37) and src/watch.rs `sleep_backoff` (:835-851) wait on a `Condvar` the shutdown branch (src/watch.rs:174-189) rings, re-reading the flag, in place of 200 ms steps; the hook reaper (src/hook/command.rs:230) waits on its state's `Condvar` while no command runs, in place of 100 ms steps. Gate: tests/watch.rs, tests/first_run.rs and src/hook/command.rs tests (exit within a second of SIGINT, a hook timeout still killed on time); a pimdir-only watch idle 31 s at most 11 switches a second (24.9 measured: main loop 10, reaper 10, poll 5). Commit `perf(watch): sleep until the next round or the shutdown`.
- [ ] H4 Re-run ~/code/pimalaya/.pimdir-migration/carillon/perf.sh (open, idle, reopen, purge) at 1k, 10k and 100k, and `seeder measure-latency` (interval 30 s, 3 rounds; 10,000 mails of noise at interval 120 s), base and new release builds (`--no-default-features --features rustls-ring,pimdir,vendored`), under the measure lock on CPU 15, an A/A pair first, quiet window. Gate: thresholds of inventory.md "Acceptance"; tables in report.md.

## I. Docs

- [ ] I1 CHANGELOG.md Unreleased: **BREAKING** Changed "Bumped io-pimdir to 0.20.0 (storage-draft-17)", reading neverest's draft-17 stores, refusing storage-draft-16 and earlier (`store-version`) naming carillon, the start-over note (neverest syncs a new store once the old is moved aside; carillon reads it on its next poll; restart a carillon older than this release); Fixed lines for B2, B5, B6; Changed lines for P1, P2; Added `property-invalid` (D4).
- [ ] I2 README.md: one line under the pimdir backend for the start-over note. Gate: `sh cairn/verify.sh`.
- [ ] I3 Fold delta.md into cairn/spec/daemon.md (:207-219, :540-552, :610-622, :651-684); log cairn/log/<date>-follows-storage-draft-17.md; status `landed`, folder to changes/archive/. Gate: `sh cairn/verify.sh`.

## J. Tests

- [ ] J1 Offline: suite, A2's matrix, `cargo fmt --check`. Gate: all pass.
- [ ] J2 Cross-app, under the `servers` lock: tests/stalwart.sh, neverest at the same io-pimdir revision syncs its CalDAV calendar into a scratch store, carillon (`pimdir.root` there) fires an event's alarm; then the store is moved aside and synced anew while carillon runs: the next poll reads it, nothing fires twice. Gate: the `alarm-fired` lines, logged in journal.md. No queue row exists to drain (carillon writes none).
- [ ] J3 Live: none; no live suite reaches io-pimdir.

## K. Release readiness (owner)

- [ ] K1 Before any release: neverest, himalaya, cardamum, calendula and carillon pin one io-pimdir revision; carillon's `PIMDIR_CODES` equals himalaya's (D4).
- [ ] K2 Once io-pimdir 0.20.0 is released by its tag workflow: `io-pimdir = { version = "0.20.0", ... }`, the `[patch.crates-io]` line gone, lock moving io-pimdir alone; spec "io-pimdir 0.20"; A2, J1. Commit `build: move to io-pimdir 0.20.0`.
- [ ] K3 The pimdir draft tags, then carillon's own release (1.0), the owner's.

## L. Report

- [ ] L1 report.md: what landed, the measures before and after, what is parked, what waits on the owner.
