---
cairn: log
change: jmap-baseline-paged-after-its-state
landed: 2026-11-18
---

# A JMAP baseline paged after its state

Finding CRL-6 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5). Spec moved: daemon (A JMAP account watches every domain it configures, a sentence and a scenario).

- `Watched::arm` listed the collection, then read its state, so an item moving between the two was in neither and no round reported it. It now reads the state first, as `pimdir::load` reads its change cursor before the items. The price: an item arriving during the listing and listed is in the picture and named again by the first `/changes`, which reads as nothing for a message (same keywords) and as an edit for a card or an event.
- `baseline_mailbox`, `baseline_addressbook` and `baseline_calendar` were one query each, with no `position` nor `limit` and `total` never read, so a server capping its answers (RFC 8620 §5.5 lets it) left every item past the first page out of the picture, to be reported as an arrival when next touched. They now go through one `baseline` pager: pages of `min(maxObjectsInGet, 500)`, advancing by the ids a page named (`notFound` included), until a page comes back empty or `total` is reached. A listing whose count does not end at `total` (an item leaving between two pages shifts the next ones up) is read again from the start, and after three the session fails with "The watched collection kept moving while it was listed 3 times", retried with the usual backoff.
- Tests in src/jmap.rs, on a stateful loopback JMAP server (`Mock`, `serve`) that now also carries the CRL-1 tests: `a_baseline_reads_every_page_the_server_answers` (5 messages, 2 ids per query) and `an_arrival_while_the_baseline_is_listed_is_reported` (a message arriving once the listing reaches the end of the inbox) fail on the HEAD code (a8aab72: 2 messages held, no event) and pass after; `a_listing_short_of_its_total_is_read_again` drives the pager alone.
