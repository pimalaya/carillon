---
cairn: change
id: api-listed-collections
status: landed
created: 2026-10-04
---

# Watch a Google People and Graph collection by the id its listing gives

## Why

The same alignment as [dav-listed-collections](../dav-listed-collections/proposal.md), on the vendor backends. A listing of a Google People account gives one address book, `contacts`, every contact the account owns; carillon took `gpeople.addressbook` for a contact group and found no `contactGroups/contacts`. A Graph listing gives the default contact folder as `contacts`, nested folders by id, and every calendar by id; carillon matched `contacts` only after the top-level folders by name, nested folders not at all, and calendars on the first page of ten only.

## What

- `gpeople.addressbook = "contacts"` watches every contact, with no membership filter; a group is still matched by name or resource name.
- `msgraph.addressbook = "contacts"` names the default folder before any folder of that name; an id the top-level listing lacks is asked for directly, which reaches nested folders.
- `msgraph.calendar` and the contact folders are listed a hundred to the page, and a calendar id the page lacks is asked for directly.
- Google Calendar already takes the listed id (and `primary`); nothing changes there.
