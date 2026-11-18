---
cairn: log
change: deny-without-stale-entries
landed: 2026-11-18
---

# deny.toml without stale entries

Line PKG-8 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4), carillon's part, its calendula clause left aside as plan.md:885 rules it wrong. No spec moved, no CHANGELOG line: deny.toml is the repository's audit, not what a user runs.

- RUSTSEC-2023-0071: on 23766ea carillon still ignored it, where neverest's owner review dropped the ignore (neverest/cairn/log/2026-11-16-the-v1-review-deletions.md). The advisory database now lists rsa `>= 0.10.0` as patched and the lock holds rsa 0.10.0, so `cargo deny --all-features check` warned `advisory-not-detected` on the entry and passed with it removed: the ignore was stale, not an audit hiding a failure. The `0BSD` allowance was unmatched too (`license-not-encountered`), as neverest found. deny.toml is now neverest's, byte for byte; `cargo deny --all-features check` reads advisories, bans, licenses and sources ok.
- The other clauses, re-checked: jiff-tzdb is 1.0.0 in carillon, neverest and himalaya, one tz database; carillon marks breaking lines `**BREAKING**`, the spelling neverest and himalaya use; carillon's flake.lock carries the inputs neverest's does (fenix and nixpkgs of 2026-11-01), calendula's alone being older.
- Left for the owner: config.sample.toml keeps its banner and prose (388 lines) where neverest's was rewritten on himalaya's compact model (115 lines, one commented key per line with a trailing note) in the owner's own v1 review (neverest 3ffdf17). That rewrite is a pass of the same kind on carillon's, not a mechanical change.
