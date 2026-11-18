---
cairn: log
change: graph-delta-page-size
landed: 2026-11-18
---

# A Graph delta page of a thousand

Finding CRL-5 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5). Spec moved: daemon (Microsoft Graph and Google accounts are watched through their own APIs, one clause).

- `Watched::delta_page` in src/msgraph.rs called `messages_delta`, `contacts_delta` and their `_from_link` twins, which send no page size, so Graph answered its default of ten rows a page: a 20,000-message Inbox was about 2,000 sequential requests before `session-opened`, and again after every lost session. It now calls io-msgraph's `messages_delta_with_params` (`max_page_size`), `contacts_delta_with_page_size` and the two `_from_link_with_page_size` calls with `DELTA_PAGE` (1000), as neverest's `PAGE_SIZE` and `CONTACTS_PAGE_SIZE` do. Graph keeps no memory of the `Prefer` header across pages, so it rides every page, the ones built from a `nextLink` or a `deltaLink` included. Graph caps message pages lower (512 by neverest's note of 2026-10-07); the header asks a ceiling, so a capped answer is read as before.
- src/scripted.rs keeps the head of every request it is sent (`heads`), beside the request lines (`seen`).
- Test: src/msgraph.rs `every_delta_page_asks_for_a_thousand_items`, a mail and a contacts round of two pages over the scripted stream, fails on the HEAD code (abc662c, no `odata.maxpagesize` in the first head) and passes after.
