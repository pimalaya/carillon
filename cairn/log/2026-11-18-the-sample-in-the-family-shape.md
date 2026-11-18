---
cairn: log
change: the-sample-in-the-family-shape
landed: 2026-11-18
---

# The sample in the family shape

The PKG-8 clause left for the owner on 2026-11-18 (cairn/log/2026-11-18-deny-without-stale-entries.md), done on the coordinator's go the same day. No spec moved: daemon.md names config.sample.toml as the documented sample, not its layout.

- config.sample.toml held a banner and prose paragraphs over 388 lines, an IMAP account active with a `REPLACE_ME` password. It now takes the shape neverest's owner review gave neverest's (neverest 3ffdf17), himalaya's: `[accounts.example]` with `default = true` active and every other key commented, one per line with a trailing note, in sections opened by a one-line heading. The keys several backends share are shown once, under imap, with a line naming the backends taking the same keys: the hook keys (`notify.summary`, `notify.body`, `command` as a shell line and as an argv list, `flags`) and the network keys (`tls.*`, `alpn`, `proxy.url`). 129 lines.
- What moved out: the per-backend repeats of those shared keys, and a few hook lines repeating a shape already shown (29 keys). What came in: `gmail.user-id`, the four `imap.id.fields.*` one per line where the old sample wrote them as one inline table, and `password.command` for `login` and `scram-sha-256` in place of `password.raw`. Every one of the fifteen hook names stays in the sample at least once.
- README.md says to uncomment one backend's keys and the hooks wanted, where it said to keep one backend and delete the rest.
- The argv hook keeps its `sh -c` form (CRL-8), so `a_sample_argv_hook_naming_a_variable_runs_through_a_shell` holds; `the_sample_names_no_unknown_key` loads the sample as written and every variant pimalaya-config's `sample::variants` makes of it, still over a hundred, none refused as unknown or removed; tests/first_run.rs, which names the sample, passes.
