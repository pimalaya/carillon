---
cairn: log
change: config-sample-on-himalayas-shape
landed: 2026-11-25
---

# config.sample.toml on Himalaya's shape

## What landed

- config.sample.toml follows himalaya's: `[accounts.example]`, `default = true`, the account `proxy` and `tls` keys, then one section per backend, each key one `#key = value<TAB># note` line; 158 lines (159 before).
- `tls` takes flat keys at the account (`tls.provider`, `tls.rustls.crypto`, `tls.cert`, `tls.client.cert`, `tls.client.key`), where `imap.tls.client` was an inline table; the `imap.tls.provider` and `imap.tls.rustls.crypto` repeats went, the Network paragraph keeping `imap.alpn`, `imap.tls.cert` and `imap.proxy.url`.
- The Hooks heading is one line; its explanation moved into the notes of the hook keys, and the two IMAP hook lines left in the IMAP section joined the Hooks paragraph, which follows IMAP's Network paragraph as in himalaya.
- Alternative keys are separate paragraphs replacing the one above: `imap.watch.idle` or `poll`, `imap.sasl.plain`, `xoauth2` or `anonymous`, `jmap.watch.push` or `poll`, `jmap.auth.bearer`, `header` or `basic`, `caldav.auth.basic` or `bearer`. The `login`, `scram-sha-256` and `oauthbearer` lines, the same keys as `plain` or `xoauth2`, are named in those notes instead.
- Tests: himalaya's `the_sample_configuration_parses_with_each_variant` and its helpers ported to src/config.rs (under `notify`, which a sample notification needs), replacing `the_sample_names_no_unknown_key` and pimalaya-config's `sample` dev feature. A run of replacing paragraphs takes its paragraph of the rank at each rank, so a run of three tests each one. `the_sample_names_every_hook` checks the fifteen hook names stay in the sample.
- Capabilities moved: none (the sample's layout is not specified).
