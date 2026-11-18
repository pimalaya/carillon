---
cairn: log
change: the-sample-argv-hook-expands
landed: 2026-11-18
---

# The sample's argv hook expands its variables

Finding CRL-8 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5), shown on 2026-11-16 by a run on both binaries. Spec moved: daemon (A hook templates against what its event carries, a sentence).

- config.sample.toml's active `imap.hooks.on-flag-add.command = ["notify-send", "$flag set on $id"]` notified the literal `$flag set on $id`: an argv command runs as written, its variables reaching it in its environment only, as the spec already said. The line is now `["sh", "-c", 'notify-send "$flag set on $id"']`, and the sample's hook paragraph says a command gets the variables in its environment, a shell line expands them, and a list runs as written. carillon's behaviour does not move: expanding into argv would put server-controlled values into arguments, which the family keeps out (the analysis lists "no value is ever expanded into a shell line" as sound).
- Test: src/config.rs `a_sample_argv_hook_naming_a_variable_runs_through_a_shell` reads every hook command list of the sample, commented or not, and asks one naming a `$` in an argument to start with `sh -c`. It fails on the HEAD sample (9cb2658) and passes after; `the_sample_names_no_unknown_key` still loads the new line.
