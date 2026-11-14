---
cairn: log
change: unknown-keys-are-refused
landed: 2026-11-14
---

# Unknown keys are refused

Every configuration table refuses a key it does not read, after the family-wide owner decision of 2026-11-14. Spec moved: `daemon` (MODIFIED: A watch runs from a TOML file, which said the account block ignored unknown keys for himalaya's sake).

- src/config.rs: `Config` and `AccountConfig` gain `deny_unknown_fields`, every other configuration table already carrying it; no configuration type flattens another, so none was restructured. pimalaya-config's notification table refuses its own keys, and a secret is an enum. `accounts` and `imap.id.fields` stay free maps, their keys being names.
- The keys carillon moved under each backend before 0.1, which the tolerant account block read as nothing, are refused naming where they went and MIGRATION.md, through `Option<RemovedKeyConfig>` fields (an uninhabited type, as Calendula refuses its removed tables) whose `deserialize_with` fails: `folder` (mirador) and `mailbox` (both carillon's prototype), `collection`, `watch`, `hooks`, and the `dav` backend. An account-level `hooks` table used to load and never fire. The wizard rendered the current schema, so it wrote no key refused now.
- Kept: `hooks` as the alias of a backend's `hook` table (hook-shape-follows-siblings, owner 2026-11-05), and the SASL `username` and `password` spellings the sample writes.
- Cargo.toml: pimalaya-config's `sample` feature as a dev-dependency; the pimalaya-cli patch moved from git 0be1b1f to the local ../cli (79c0d9e, which only adds modules), a commit of its own.

Tests in src/config.rs's module: `an_unknown_key_is_refused_at_every_level` (root, account, `imap`, `imap.tls`, `imap.sasl.plain`, `imap.watch.idle`, `imap.hook`, a hook) failed on the root case before the change, the backend and deeper cases being guards that already passed; `a_key_carillon_no_longer_reads_is_refused_by_name` and `a_removed_key_is_refused_by_name_when_the_file_is_loaded` (through `Config::load` and pimalaya-config's parse) failed; `the_former_names_still_load` (`imap.hooks`, SASL `username` and `password`, a secret's `cmd`) and `the_sample_names_no_unknown_key` pass before and after. The last loads config.sample.toml as written through `Config::load`, then parses every pimalaya-config `sample::variants` of it and fails on any error naming an unknown key or variant; the sample has no sections, so most variants (one commented line of a commented backend) fail on a missing key and only the unknown-key check holds for them. A misspelt commented key made it fail, seen by hand.

Verified by `cargo fmt`, `cargo test --all-features` (90 unit tests, 5 in tests/exit.rs, 9 in tests/first_run.rs, the live suites built and ignored), `cargo test` on the default, `rustls-ring,imap` and empty feature sets, `cargo clippy --all-targets -- -D warnings` over the default, all, empty and per-backend feature sets (rustls-ring with each of imap, jmap, maildir, dav, msgraph, gmail, gcal, gpeople, pimdir), `cargo deny check` and `sh cairn/verify.sh`. No live suite was run.

Left: the family rule refuses a removed key while hook-shape-follows-siblings reads every former hook name until 1.0, for the owner to settle before that change lands; a backend the build leaves out is refused as an unknown key rather than naming its cargo feature; serde lists the refused keys among the expected ones when it refuses another key.
