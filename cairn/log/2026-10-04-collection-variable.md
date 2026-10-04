---
cairn: log
change: collection-variable
landed: 2026-10-04
---

# Named every hook's collection as `$collection`

## What landed

`$collection` joined `$id` in every hook's vocabulary: templates may name it and every command finds it in its environment, holding the collection as the account configured it. `$mailbox`, `$calendar` and `$addressbook` stay.

## Note for verification

A hook test reads `collection`, `calendar` and `id` off a calendar arrival, with no `mailbox`; the Stalwart run of [dav-listed-collections](2026-10-04-dav-listed-collections.md) saw `collection=default` in the environment of both the event and the card command.
