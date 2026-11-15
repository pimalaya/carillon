---
cairn: change
id: collection-variable
status: landed
created: 2026-10-04
---

# Name the collection of every hook as `$collection`

## Why

A hook names its collection under its backend's word, `$mailbox`, `$calendar` or `$addressbook`, and a command reading the environment had to know which one to read. One command serving every domain (one per watched collection, a mail inbox beside a calendar and an addressbook) wants one name, as `$id` already is.

## What

Every hook also carries `$collection`, the collection as the account configured it, in its templates and in the command's environment. The backend's own word stays.
