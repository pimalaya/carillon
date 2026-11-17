---
cairn: delta
change: anonymous-dav-and-punycode-hosts
---

# Delta

## ADDED Requirements

### Requirement: A DAV block may send no credential (daemon)

`caldav.auth` and `carddav.auth` are optional, absent sending no `Authorization` header; a 401 to such a block says the account configures no `auth`, its code unchanged. One scenario.

### Requirement: Hosts are ASCII (packaging, new capability)

url's IDNA without a Unicode back end (`idna_adapter` 1.0.0); a non-ASCII host refused, a DAV `server` or `home` and a JMAP `server` naming the host and its Punycode form. One scenario.

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
