---
cairn: delta
change: a-draft-13-store
---

# Delta

The requirement is in cairn/spec/daemon.md.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: A pimdir calendar is watchable
The `pimdir` backend SHALL watch one collection of one pimdir store of the draft io-pimdir 0.14 reads (pimdir draft-13, the one neverest writes), given by `pimdir.root` and `pimdir.calendar` (the collection id a listing of the store gives), and optionally `pimdir.account`, the account of the store the calendar is grouped under (pimdir STORAGE §9.2), unset reading the calendar whichever account it is grouped under, as a reader (pimdir STORAGE §8, §14.1): it SHALL take no lock, SHALL write nothing to the store, and SHALL read the committed rows, the pending queue not folded in. It SHALL poll the store's change cursor (pimdir STORAGE §4.5) every `pimdir.watch.poll.interval` (30 seconds by default), read again the items stamped above it and forget the deleted or retained ones, and read the whole collection again when the store was reset, a purge count moved or the collection itself moved. An item SHALL be named by its public id (pimdir STORAGE §9.1), its object hash standing as its `$etag`, and an item whose body is not local SHALL be skipped until a change brings it. The backend SHALL declare `on-alarm-trigger` and no other hook, and an account configuring none SHALL be refused when read. A collection the store does not hold, or holds under another account than `pimdir.account` names, SHALL fail the watch and `account check` coded `calendar-not-found`, naming it; one holding no calendars, or a store of another draft, SHALL fail them naming it.

#### Scenario: A store the owner has not created
- **GIVEN** a `pimdir.root` holding no pimdir.db
- **WHEN** the watch starts
- **THEN** it fails naming the root, and creates nothing there

#### Scenario: A change synced in
- **GIVEN** a pimdir watch holding an event whose alarm fires at 08:45
- **WHEN** the store's owner applies an update moving the event an hour later, and the watch polls
- **THEN** the alarm fires at 09:45, and not at 08:45


## REMOVED Requirements

None.
