---
cairn: tasks
change: alarm-wait-on-ceichas
---

- [x] `run` waits on a `CeichasTimer` (default backend, `Timerfd` on Linux), `AlarmShared` holds its waker, the 200 ms steps gone
- [x] Shutdown: every alarm thread leaves its waker in a list the Ctrl+C handler rings once the flag is set
- [x] Tests: a change ends the timer's wait; the suite on the default and every reduced feature set; a live run on Stalwart (log entry)
- [x] ceichas pinned by git (661be05) through a patch
- [x] The 200 ms bound replaced in spec/daemon.md, log entry, CHANGELOG
- [ ] The git pin replaced by a ceichas release (user)
- [ ] The 200 ms shutdown polling of the watch threads (src/poll.rs) on ceichas wakers
