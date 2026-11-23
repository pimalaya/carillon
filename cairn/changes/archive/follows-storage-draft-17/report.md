---
cairn: change
id: follows-storage-draft-17
part: report
---

# Report

carillon reads storage-draft-17 stores on io-pimdir 17d4ea8 (0.20.0 RC). The requirement is `version = "0.20.0"` and the git patch stays until the release. No silent loss was found. Decisions and timings are in journal.md.

## What landed

- B1, D1: a refused store's message names carillon and the draft it reads.
- B2, D2: the store is opened again at every poll, so a store synced anew is read.
- B5, D3: an alarm already fired does not fire again when the store renumbers its items.
- B6: the sample points at neverest's store.
- P1: listing the calendar again reads only the bodies that moved.
- P2, D5: no sleep steps remain in the poll, the backoff or the idle hook reaper.
- D4: `property-invalid` is relayed.
- D6: url moved to 2.5.9.
- CHANGELOG and README carry the start-over note, the spec is folded, and the log is written.
- Gates at 17d4ea8: suite 193 passed, 9 ignored; 17 clippy sets exit 0; no-imap suite 70 passed; fmt, deny, `nix build` and `nix flake check` clean.
- J2, cross-app (drop 3, local Radicale): the shared store neverest wrote was read in place (one item holding an alarm, none queued, the event being beyond the 48 h horizon); on a scratch store synced by neverest from a calendar of carillon's own, `j2-early` fired once; the store was then moved aside and synced anew while carillon ran: the next poll logged the reset, `j2-late` fired once and `j2-early` never again.

## Measures (H4, under load)

Two bit-identical builds differ by 2.7% in open time. Base is ed02e89 on io-pimdir 0.19.1 (draft-15 stores); new is 79c97a9 on cc3021b (draft-17). Open time is the median of 3 runs.

| Measure | 1k base | 1k new | 10k base | 10k new | 100k base | 100k new |
|---|---|---|---|---|---|---|
| open, ms | 17.0 | 15.5 | 96.2 | 106.8 | 757.0 | 801.3 |
| open CPU, ticks | 1 | 0 | 8 | 10 | 75 | 79 |
| peak after open, MB | 14.0 | 14.1 | 60.9 | 62.9 | 517.7 | 534.2 |
| idle wake-ups a second | 24.9 | 10.0 | 25.0 | 10.0 | 25.0 | 10.2 |
| reopen and cursor, µs | 942 | 1006 | 788 | 609 | 702 | 676 |
| purge poll, CPU ticks | 1 | 2 | 9 | 2 | 89 | 17 |
| peak after purge poll, MB | 19.1 | 14.1 | 112.0 | 63.7 | 1,030.0 | 543.5 |

- Latency at 10k, 30 s interval: 30.0 s on both builds.
- 10,000 mails of noise on the 1k store: base 52.1 s (2 ticks), new 45.2 s (0 ticks).
- Met: reopen under 1 ms, no second book, idle at most 11 a second, latency, the noise poll, memory within 5% (+3.2% to +3.3%).
- Not met: open time within 5% (+11% at 10k with runs from 85 to 126 ms; +5.9% at 100k), measured under load; re-measure in a quiet window.
- Not met: the purge poll at 5% of the open's CPU. It is about 20%, and what is left is io-pimdir's listing (needs-io-pimdir.md N4).

## Waiting on the owner

- K1: the five CLIs on one io-pimdir revision, and carillon's `PIMDIR_CODES` equal to himalaya's.
- K2: once 0.20.0 is released, drop the git patch.
- K3: the pimdir tags, then carillon's release.
- D3 changes one thing users can see: two items of one calendar with the same `UID` alarming at the same instant fire once.
