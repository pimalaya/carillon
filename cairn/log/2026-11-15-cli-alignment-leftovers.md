---
cairn: log
change: cli-alignment-leftovers
landed: 2026-11-15
---

# The alignment leftovers, closed

The 2026-10-18 audit's five carillon items, re-checked against the code on 2026-11-15, needed no code: nothing behavioural was left. Spec moved: none.

- F2 (two wordings for an account that cannot be resolved) was done by the-family-line (d3f41c2): `watch -a` and `check` both go through pimalaya-config's `take_account`.
- F6 (the bare invocation met inside `Cli::execute`) was done by the-family-line (5388407): src/main.rs meets a bare `carillon` with the help and a missing configuration with the welcome, as himalaya's src/main.rs does.
- G2 (a fresh `SecretResolver` per reconnect) is kept by design (owner, 2026-11-15): pimalaya-config documents that the resolver holds the plaintext and should be dropped with the runtime account rather than kept for the life of the process, and a watch lives as long as the process. The spec already requires credentials resolved per attempt and a resolver living no longer than its account.
- A2 (the io-webdav git pin) is handled at release (owner, 2026-11-15): the owner releases the libraries and drops every `[patch.crates-io]` pin after the 1.0 round.
- H1 (the wizard's local save layer) is moot, the wizard being gone.
