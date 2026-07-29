---
cairn: tasks
id: multi-account-watch
---

# Tasks

- [x] `cli`: drop the global `AccountFlag`; drop `account_name` from `Command::execute`
- [x] `watch`: add a cumulative `-a/--account` (`Vec<String>`); select all when empty, the named subset otherwise (error on unknown name)
- [x] `watch`: spawn one thread per selected account over the shared shutdown, then join
- [x] `main`: stop threading `account_name` through
- [x] Update README (usage) and CHANGELOG
- [x] Fold the delta into `spec/daemon.md`; write the log entry; set status landed
- [x] `cargo check` / `clippy` / `fmt` green
