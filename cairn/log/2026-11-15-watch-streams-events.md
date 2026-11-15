---
cairn: log
change: watch-streams-events
landed: 2026-11-15
---

# A watch streams what it observes

`carillon watch` prints one event line per thing it observes, then one result line once SIGINT, SIGTERM or SIGHUP asks it to stop, exiting 0, after pimalaya-cli 51ce8b1's "A long-lived command streams events". Spec moved: `daemon` (ADDED: A watch streams what it observes; MODIFIED: Every configured account, or a chosen one; Ctrl+C is prompt on every path; A hook failure never stops the watch; Every printed output has a published schema; The JSON contract is described and versioned; A failure carries a stable code).

- Events: `session-opened` (`backend`, `method`, `collections`), `session-lost` (`code`, `error`, `sources`), `changed` (`collection`, `change` tagged `kind`), `hook-failed` (`reaction`, `code`, `error`, `sources`), `alarm-fired` (`collection`, `id`, `uid`, `kind`, `summary`, `action`, `trigger`), each naming its `account`; result `{"stoppedBy":"signal"}`. Registered as `carillon-watch` and `carillon-watch.events`.
- src/watch.rs: pimalaya-cli's `Shutdown` replaces carillon's `ctrlc` handler (the dependency is gone); the account and alarm threads send over one channel to the command's thread, the only one printing. On the request it sets the shutdown flag the sessions already poll and rings the alarm wakers, as the handler did.
- src/hook.rs: a hook's failed reactions are handed back rather than logged, a command exiting non-zero now among them (it was a `warn` with no failure). A hook command's stdout is carillon's stderr (`Stdio::from(io::stderr())`), so its output no longer lands between the event lines.
- `with_code` (item 13): the local copy and its `chained` helper are deleted for `pimalaya_cli::error::with_code(err, error::code)`. Same behaviour: the local one returned `err` unchanged when it carried a carillon code or when `error::code` found none, else rebuilt it; pimalaya-cli's does the same, and also returns unchanged a chain coded `usage-invalid` or holding a `PartialOutcome`, which carillon's `code` never coded either (no reader matches them). The tests asserting codes through the failure object (src/imap.rs `a_watch_refused_with_noperm_is_permission_denied`, tests/account.rs, tests/exit.rs) pass on it.
- Tests: tests/watch.rs (unix, maildir) runs the binary on a temporary Maildir: under `--json`, `session-opened`, `changed` `item-added`, `hook-failed` `command`, then SIGINT, one `{"stoppedBy":"signal"}` and exit 0; in text, the line "[a] watching mailbox `.` over maildir, polling" through a pipe, then "Watch stopped by a signal". Before, nothing reached stdout and Ctrl+C was carillon's own handler exiting 0 without a line. tests/watch.rs `a_hook_printing_to_stdout_reaches_stderr`, which failed with the redirect taken out (the echo broke the JSON lines). src/hook.rs `a_failing_command_is_handed_back`, src/json_schema.rs `the_watch_events_are_described`.

Left: `session-opened` precedes the connection.
