---
cairn: tasks
change: hook-template-variables
---

- [x] Give each hook field the vocabulary it can fill, in one place both the loader and the runner read
- [x] Refuse a notification naming anything else when the configuration is read
- [x] Expand a legitimate but absent variable to nothing rather than dropping the notification
- [x] Cover the refusal, the default, and the absent-but-legitimate variable with tests
- [x] Build, clippy and fmt green on every feature combination
- [x] Verify against a live server that a removal notification fires
- [x] Fold the delta into the spec and log the change
