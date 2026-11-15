---
cairn: delta
change: alarm-wait-on-ceichas
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: Alarms are held in memory
A watch SHALL resolve the firings of the calendar it watches over a window ending a horizon after now (48 hours), and SHALL build it again when a member changes and when half the horizon has passed. Nothing SHALL be written between runs: a watch starting SHALL fire nothing whose trigger is before its start, and SHALL never fire one firing twice. A floating time SHALL resolve in the system's time zone, else as if on UTC, and zones SHALL come from the database bundled in the binary. Each account with an alarm hook SHALL wait for its next firing on a thread of its own, living as long as the account rather than its session, so the alarms a lost session read keep firing on time until the next session replaces them with what it reads. The thread SHALL wait on a ceichas timer, the wall clock counting a suspend and following a clock change, and SHALL not wake before its next firing, a move of its firings or a shutdown, which wakes every alarm thread once the shutdown is requested. The watch SHALL keep its own interval.

## REMOVED Requirements
