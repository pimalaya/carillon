---
cairn: tasks
change: prompt-shutdown
---

- [x] Make the io-imap watch shutdown-poll interval an option
- [x] Pump the resolver's coroutines with the shutdown flag, on a bounded read deadline
- [x] Hand back the not-ready failures on the JMAP connection
- [x] Build, clippy and fmt green on every feature combination
- [x] Fold the delta into the spec and log the change
- [ ] Measure the actual Ctrl+C latency against a live server
