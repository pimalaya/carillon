---
cairn: log
change: alarm-wait-on-ceichas
landed: 2026-11-05
---

# The alarm wait on ceichas

The alarm thread waits on a `CeichasTimer` (ceichas 661be05, a git pin until released) rather than in 200 ms wall-clock steps: `AlarmShared` holds its waker, rung when the book changes, and src/watch.rs leaves every alarm thread's waker in a list the Ctrl+C handler rings once the shutdown flag is set, so a thread blocked until a firing hours away still stops. A timer failing to wait is logged and tried again a second later. Spec moved: `daemon` (MODIFIED: Alarms are held in memory).

Verified on Stalwart with the `Timerfd` backend: alarms fired at T+30, T+60 and T+120 (an edit moving one), to the second; the idle alarm thread made 3 voluntary context switches in 10 seconds, one firing among them, where the 200 ms steps made about 50; the watch exited 219 ms after SIGINT with an alarm a day away, the remaining delay being the DAV thread's own 200 ms shutdown polling (src/poll.rs).

Left: the 200 ms shutdown polling of the watch threads; a test moving the wall clock mid-wait, which ceichas's VM suite covers on its side.
