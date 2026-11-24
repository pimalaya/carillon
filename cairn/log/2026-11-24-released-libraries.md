---
cairn: log
change: released-libraries
landed: 2026-11-24
---

# Moved to the released libraries

## What landed

- Cargo.toml: pimalaya-cli 0.8.1, io-gpeople 0.9.1, io-jmap 0.11.1, io-msgraph 0.11.0, io-webdav 0.14.0, io-pimdir 0.20.0 from crates.io; the io-pimdir git pin (65a5026) and the `[patch.crates-io]` section are gone.
- src/dav.rs: io-webdav 0.14.0 hands back the member rows the server answered with an error (`WebdavSyncDelta::unreadable`, a 403, a 5xx), which 0.13 dropped, so a full enumeration read such a member as removed. As io-webdav's docs ask, a re-enumeration or a `PROPFIND` listing holding one now reads as incomplete: it reports no removal and logs a warning. A `sync-collection` delta reports nothing for such a row, as before. Pinned by `dav::tests::a_listing_with_an_unreadable_member_reports_no_removal`, red before the change (`ItemRemoved { domain: Card, id: "/b/b.vcf" }`).
- Capability moved: daemon (the WebDAV requirement reads an unreadable member as no removal).

## Behaviour from the other bumps

- pimalaya-cli 0.8.1: `--log-file` refuses a symbolic link to an existing file.
- io-gpeople 0.9.1: a sync-token page whose `totalItems` is negative parses.
- io-jmap 0.11.1 and io-msgraph 0.11.0 touch event creation only, which carillon does not do (the msgraph live suite builds its events with `..Default::default()`).
