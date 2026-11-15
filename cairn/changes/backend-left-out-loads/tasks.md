---
cairn: tasks
change: backend-left-out-loads
---

# Tasks

- [x] src/config.rs: `IgnoredAny` twins of the backend fields and of the account `proxy`
- [x] src/error.rs: `backend-not-built` and its message naming the feature
- [x] src/watch.rs, src/check.rs: the block reached fails coded, in the priority order
- [x] tests/backend_not_built.rs, built without `imap` (`cargo test --no-default-features --features rustls-ring,maildir`): `check` fails the imap row coded and naming `--features imap`, `-b jmap` leaves it unread, `imapp` still refused, `watch` over two accounts loses the imap one coded while the Maildir one opens, then stops on SIGINT; no second `session-lost` past the first backoff; a watch with nothing left fails coded, exit 1
- [x] Fold into spec/daemon.md, log entry, CHANGELOG, MIGRATION.md
