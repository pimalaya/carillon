---
cairn: tasks
change: wizard
---

- [x] Render an account back as TOML: `skip_serializing_if` on the defaulted fields, `AccountConfig::render`
- [x] Add the wizard: one input prompt, discovery search, per-backend configuration and credentials
- [x] Test the connection, and read from it what discovery does not carry (DAV collection, calendar components, watch method)
- [x] Save, append or print, and welcome from a bare `carillon` or a command that finds no configuration
- [x] Add the shared `--help` footer
- [x] Update the sample configuration, the README and the changelog
- [x] Build, clippy, fmt and the tests green on every feature combination
- [x] Fold the delta into the spec and log the change
- [ ] Run the wizard against a live provider
