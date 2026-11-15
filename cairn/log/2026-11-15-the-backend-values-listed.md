---
cairn: log
change: the-backend-values-listed
landed: 2026-11-15
---

# The backend values listed

Line R9 of the 2026-11-15 CLI alignment round landed. Spec moved: daemon (added The backend is picked by `-b`).

- `-b` is a clap value enum, as himalaya's: its help and its refusal list `[possible values: auto, imap, jmap, maildir, caldav, carddav, msgraph, gmail, gcal, gpeople, pimdir]`, every backend in every build, and the order `auto` follows ends with pimdir.
