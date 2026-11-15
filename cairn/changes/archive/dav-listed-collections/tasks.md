---
cairn: tasks
change: dav-listed-collections
---

- [x] Resolve a bare DAV collection under the principal's home set, absolute paths and other relative paths unchanged
- [x] Retry a failed round once on a fresh connection, keeping the picture and the token
- [x] List a collection whose server has no `sync-collection`, and read every round against the picture
- [x] Report what differs after a rejected sync token rather than nothing
- [x] Take a component-less member of a mixed calendar for the hooks' domain
- [x] Resolve and fall back the same way in `check`
- [x] Test the diff, the truncated listing, and discovery plus listing against a fake DAV server
- [x] Verify on Stalwart that `default` is found under both home sets and every event and card hook fires
- [x] Document the sample, fold the delta, log the change
