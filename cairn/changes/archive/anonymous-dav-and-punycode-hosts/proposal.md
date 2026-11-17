---
cairn: change
id: anonymous-dav-and-punycode-hosts
status: landed
created: 2026-11-17
---

# Anonymous DAV and Punycode hosts

## Why

Audit line COH-6 of the CLI family analysis (2026-11-15) and two owner rulings of 2026-11-17, ahead of carillon 1.0 freezing its configuration. carillon already loaded a DAV block with no `auth`, sending no `Authorization` header, but a server answering it 401 failed with the bare `WebDAV server returned HTTP 401`. It also resolved idna_adapter 1.2, the ICU4X back end, so it turned a non-ASCII host into Punycode silently where Himalaya, Cardamum, Calendula and Ortie refuse it. The rulings: `auth` is optional everywhere, a 401 then saying the account configures no `auth`; a non-ASCII host is refused everywhere, the message pointing at its Punycode form.

## What

- `dav::watch` and `dav::probe` put "DAV server answered 401: it asks for credentials, and the account configures no `auth`" above a 401 when the block sends no credential, the code left `null`.
- Cargo.toml pins `idna_adapter = "=1.0.0"` with no default features, as Himalaya does; Cargo.lock drops the ICU4X crates.
- A DAV `server` or `home` and a JMAP `server` whose host is not ASCII are refused with "Host `<host>` is not ASCII: write it in its Punycode form (xn--...)".
- A new capability spec, packaging, states the IDNA rule as Himalaya's packaging spec does.
