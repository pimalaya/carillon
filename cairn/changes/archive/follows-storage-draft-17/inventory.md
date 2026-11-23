---
cairn: change
id: follows-storage-draft-17
part: inventory
---

# Evidence, measured 2026-11-23

Snapshots of carillon ed02e89 by `git archive`, at ~/code/pimalaya/.pimdir-migration/carillon/: base/ unchanged, src/ with the frame §2 patch in Cargo.toml alone. Logs, seeder and harness sit beside them (seeder/src/main.rs, behaviour.sh, behaviour-b5.sh, perf.sh, ci-matrix.sh). Builds pinned to CPUs 8,9,24,25 while other lanes built.

## Build and tests

| | base (0.19.1) | src (7f16b27) |
|---|---|---|
| `cargo test --all-features --no-run` | 0 errors, 0 warnings | 0 errors, 0 warnings (1 min 38 s) |
| `cargo test --all-features` | 188 passed, 9 ignored | 188 passed, 9 ignored |
| clippy, the 17 CI sets, `--locked --all-targets -D warnings` | not rerun (CI Tests run 44879871020 green on 2026-11-21) | 17 of 17 exit 0 |
| `cargo test --locked --no-default-features --features rustls-ring,maildir` | | 70 passed |
| `cargo-deny check` | advisories FAILED (RUSTSEC-2026-0592, url 2.5.8; CI Audit run 44879870944 passed on 2026-11-21, before the advisory), bans, licenses, sources ok | the same; sources ok with the git source (`allow-org.github = ["pimalaya"]`) |

Compile breaks: 0. Failing tests: 0. The six pimdir tests (src/pimdir.rs) and tests/exit.rs `a_check_over_a_store_of_another_draft_is_coded_store_version` pass on the pin.

Lock: `cargo build` adding the patch moves io-pimdir's entry alone (git source, checksum gone, `libc` added). `cargo update -p io-pimdir` also switched errno, rustix, rustls-platform-verifier, tempfile and winapi-util from windows-sys 0.61.2 to 0.52.0. `cargo update -p url --dry-run`: url 2.5.8 to 2.5.9 and form_urlencoded 1.2.2 to 1.2.3, MSRV 1.89 kept.

Nix, read from code, not built: pimalaya/nix 337abcd default.nix:192-195 vendors with `importCargoLock { allowBuiltinFetchGit = true; }`; carillon's package.nix, as neverest's (git pin b080e14) and himalaya's (dc19b8d), names no hash of its own.

## What carillon reads and writes

src/pimdir.rs calls `PimdirReader::open` (:94), `change_cursor` (:135, :185), `collection_kind` (:107), `collection_account` (:113), `list_items` (:144), `collections_changed_since` (:197), `items_changed_since` (:211), `get_item` (:228), `blobs` and `PimdirBlobs::get` (:266). It writes nothing; src/error.rs:221-254 is the relayed code table, no queue access. Features: `io-pimdir/client` and `vendored`.

SQL text and table names in carillon: tests/exit.rs:123-127 alone (`UPDATE store_meta SET format_version = ?1`, `storage-draft-11`); `store_meta.format_version` stays in storage-draft-17. Draft names stamped: tests/exit.rs:127; cairn/spec/daemon.md:611 (io-pimdir 0.19, draft-15); CHANGELOG.md Unreleased lines. No index, chunk or thread access.

io-pimdir error codes, 0.19.1 to 7f16b27: `property-invalid` added, none removed.

## Behaviour (release builds `--no-default-features --features rustls-ring,pimdir,vendored`, stores seeded by io-pimdir itself)

- B1, refusal wording: a real storage-draft-15 store (seeded by 0.19.1) under the new build prints `session-lost` coded `store-version`, retried at 2, 4, 8 s, its source "Pimdir store is storage-draft-15, written by an earlier sync engine, and this program reads storage-draft-17 alone: ...". The second account, on a draft-17 store, opened and fired its alarm on time. himalaya, cardamum and calendula name themselves (himalaya/src/error.rs:447-462, cardamum/src/pimdir/client.rs:95-113, calendula/src/pimdir/client.rs:230-238).
- B2, a store replaced under a running watch is never read: store at the root moved aside, a new store synced there holding `moved` at +15 s, `fresh` at +20 s and `keep` at +40 s. Neither +15 nor +20 fired; `keep` fired under id 1, its seq in the moved-aside store (3 in the new one). The base build does the same (B5 on base: id 1, the old store, until its session ended): the reader holds its connection for the session (src/pimdir.rs:65).
- B3, an event removed then added again with the same UID keeps its seq (1 before and after): one firing, no defect.
- B4, a purge in another collection: the next poll logs "pimdir store reset or calendar moved, reading it again" and reads the calendar whole (src/pimdir.rs:191-204).
- B5, one firing twice: `keep` fired at :18 (id 1); the store was replaced, the running session ended (its calendar renamed in the moved-aside store, `calendar-not-found`), the next session opened the new store, where `keep` is seq 2, and the same firing (trigger 14:43:38Z) fired again at :24. Same on base (14:43:57Z twice). Cause: `AlarmFiring::key` holds the item id (src/alarm.rs:90-98), the queue living across sessions.
- B6, the sample: config.sample.toml:153 names `~/.local/share/pimdir`; neverest's default root is `~/.local/state/neverest` on Linux (neverest/src/sync/driver.rs:160-162), its collection ids `<source>/<name>` (:154-156).

## Performance

Measured while other lanes built: carillon release builds (`--no-default-features --features rustls-ring,pimdir,vendored`, the shipped profile) pinned to CPU 15 under the measure lock, the harness and its store writes on the lane's CPUs, no A/A pair. Stores seeded through `PimdirProducer` and the drain (the generator draws no `VALARM`, needs-io-pimdir.md N3): one calendar, each event one `-PT15M` display alarm, starts spread over 30 days, one in ten weekly (`COUNT=20`). CPU in ticks of 10 ms (utime + stime of the process), memory VmHWM. Median of 3 runs for open; 1 run otherwise. Script: ~/code/pimalaya/.pimdir-migration/carillon/perf.sh and perf2.sh.

| Measure | 1k base | 1k new | 10k base | 10k new | 100k new |
|---|---|---|---|---|---|
| start to `session-opened`, ms | 15.5 | 12.7 | 84.7 | 81.5 | 822.9 |
| CPU of the open, ticks | 0 to 1 | 0 to 1 | 7 to 8 | 7 | 81 |
| peak memory after the open, MB | 14.0 | 13.9 | 61.0 | 61.1 | 517.4 |
| idle 31 s (one poll), context switches a second | 24.9 | 24.9 | 25.0 | 25.0 | 24.9 |
| idle 31 s, CPU ticks | 0 to 1 | 0 to 1 | 0 to 2 | 0 to 1 | 8 (the alarm thread's first queue) |
| `PimdirReader::open` + `change_cursor`, µs (D2's cost a poll) | 714 | 633 | 725 | 636 | 828 |
| poll after a purge in the mail collection (whole re-read), CPU ticks | 1 | 2 | 9 | 9 | 92 |
| peak memory after that poll, MB | 19.0 | 18.8 | 112.1 | 112.1 | 1,029.8 |

The book holds about 5 KB an event (parsed alarms and recurrence timelines, src/alarm.rs:270-297), and a whole re-read builds a second book before the first is dropped (src/pimdir.rs:139-167).

Idle wake-ups, read from code and matching 25 a second: the command's signal loop (src/watch.rs:166, 100 ms), the hook reaper with nothing running (src/hook/command.rs:230, 100 ms), the poll sleep (src/poll.rs:32, 200 ms); the alarm thread sleeps until its firing.

Time to the first alarm after a store change, 10k events, interval 30 s, the event moved so its alarm is due 2 s after the commit, 3 rounds: base 30.06, 30.05 and 30.06 s, new 30.06, 30.03 and 30.03 s, each poll 2 to 4 ticks (the event read, then the alarm queue rebuilt over the whole book). The rounds fall in step with the poll, so this is the worst case, the interval. After 10,000 mails added and one event moved (1k events, interval 120 s): that poll 7 ticks on both builds, the alarm fired at 85.2 s (base) and 82.0 s (new) after the commit, where the poll fell.

## Acceptance (H4)

- Open time and peak memory on the pin at most the base's plus 5% at 1k, 10k and 100k (the 100k base store to seed in the quiet window, 12 minutes).
- Reopen and cursor under 1 ms at 10k events (D2).
- After P1: the poll after a purge at most 5% of the open's CPU at 10k and 100k, and its peak memory the open's (no second book).
- After P2: idle at most 11 context switches a second.
- Commit to `alarm-fired` at most the interval plus 1 s; the poll after 10,000 mail changes at most the base's plus 5% (7 ticks).

## Open lines triaged

- CRL-1 to CRL-10 (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md): all closed by carillon logs of 2026-11-18, none on the pimdir surface.
- cairn/log Left lines: "the 200 ms shutdown polling of the watch threads" (2026-11-05-alarm-wait-on-ceichas.md): P2. A wall-clock test mid-wait, acknowledge and snooze (2026-11-05): out, not the pimdir surface. Resuming JMAP, DAV, Graph, Calendar and People sessions (2026-11-13): out.
- Ledgers: cli-alignment-2026-11-15 R4, R6, N31, X14 done; cli-contract-audit-2026-11-20 CRL-X1 (UTC `trigger`) landed, no pimdir line open.
- pimdir audits and the-freeze-plan: no line names carillon. GitHub: #7 open (IMAP NOTIFY), out.
