---
cairn: delta
change: birthday-alarms
---

# Delta: birthdays and anniversaries fire a hook

Capability file: cairn/spec/daemon.md.

## ADDED Requirements

### Requirement: An addressbook fires its birthdays and anniversaries
The `carddav` and `pimdir` backends SHALL offer `on-birthday-trigger`, firing once a year per `BDAY` and per `ANNIVERSARY` (RFC 6350 §6.2.5, §6.2.6) of every card of the watched addressbook, at the hook's `at` local time (default `09:00`), `days-before` days before the date (default 0), in the machine's local zone. It SHALL carry `$name`, `$kind` (`birthday` or `anniversary`), `$date`, `$age` (empty for a year-less date) and `$uid`. A year-less date SHALL fire with `$age` empty, and a 29 February SHALL fire on 28 February in a common year. A date given as text SHALL be ignored.

#### Scenario: A birthday at nine
- **GIVEN** a CardDAV account with `carddav.hooks.on-birthday-trigger.notify.summary = "$name turns $age"`, watching an addressbook holding a card "Jane Doe" with `BDAY:1990-11-27`
- **WHEN** the local clock reaches 2026-11-27 09:00
- **THEN** one notification titled "Jane Doe turns 36" is shown

#### Scenario: A year-less birthday
- **GIVEN** a card with `BDAY:--1127`
- **WHEN** the local clock reaches 09:00 on 27 November
- **THEN** the hook fires with `$age` empty

#### Scenario: A leap-day birthday
- **GIVEN** a card with `BDAY:2000-02-29`
- **WHEN** the local clock reaches 2027-02-28 09:00
- **THEN** the hook fires

#### Scenario: A birthday hook on a calendar
- **GIVEN** an account configuring `caldav.hooks.on-birthday-trigger`
- **WHEN** the configuration is read
- **THEN** it is refused, naming the hooks a CalDAV account has

### Requirement: An edited addressbook moves its dates
A card the watch reports added or changed SHALL have its body read again and its dates replaced, and a card reported removed SHALL have its dates forgotten, before the next firing is waited for. Dates SHALL be held in memory on the alarms' horizon, and a watch SHALL never fire one date twice, a restart included.

#### Scenario: A birthday corrected
- **GIVEN** a watch holding a card whose birthday fires tomorrow
- **WHEN** the card's `BDAY` is moved to the day after, and the watch polls
- **THEN** it fires the day after, and not tomorrow

### Requirement: A late birthday fires the same day only
A date reached late, the machine having slept, SHALL fire on waking when the local day is still its day, and SHALL otherwise be dropped with an info log.

#### Scenario: Asleep over nine
- **GIVEN** a running watch holding a birthday firing at 09:00, the machine asleep from 08:00 to 11:00
- **WHEN** it wakes at 11:00
- **THEN** the hook fires

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
