---
cairn: log
change: api-listed-collections
landed: 2026-10-04
---

# Watched Google People and Graph collections by their listed ids

## What landed

`gpeople.addressbook = "contacts"` watches every contact with no membership filter, `Watched.group` becoming optional; its `check` probe reads one connections page, since no group lookup proves the token there. On Graph, `contacts` names the default contact folder before any folder so named, contact folders and calendars are listed a hundred to the page, and an id the page lacks is asked for directly (`contact_folder_get`, `calendar_get`), which reaches nested contact folders. Names keep matching as before.

## What is still true

`myContacts` and every other group still resolve by name or resource name. Google Calendar already took the listed calendar id and `primary`.

## Note for verification

Build, clippy and the suite are green; a test covers the unfiltered People picture. Not run against a live Google or Microsoft account.
