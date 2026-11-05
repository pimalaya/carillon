---
cairn: change
id: alarm-wait-on-ceichas
status: active
created: 2026-11-05
---

# The alarm wait on ceichas

## Why

The alarm thread waits in 200 ms steps on the wall clock (src/alarm.rs `wait_until`), so a firing is up to a step late after a resume or a clock change and the thread wakes five times a second. ceichas (pimalaya/ceichas, change bootstrap) waits on the OS timer that fires at a wall time, with no polling.

## What

- `wait_until` becomes a `CeichasTimer::wait_until`, its waker taking the place of the `moved` flag; the shutdown handler wakes it.
- The requirement "Alarms are held in memory" drops its 200 ms bound for ceichas's guarantees.
- A test moving the wall clock past a trigger mid-wait, through ceichas's `Polling` backend or a clock it is handed.
- Later, the same for the shutdown polling of the other watch threads (src/poll.rs).
