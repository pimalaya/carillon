---
cairn: log
change: rename-to-carillon
landed: 2026-08-22
---

# Renamed mirador to carillon

The crate, the binary, the config directory, the `CARILLON_CONFIG` variable, the systemd unit and every URL. The spec followed, being current truth; the log entries and the landed changes did not, being history that was true when it was written.

Nothing about behaviour moved. The migration guide gained the table that says so, and the one command anyone needs: the configuration file itself did not change, so renaming its directory is the whole migration.

The rename came last deliberately. Ahead of the merge it would have read as a rename that happened to absorb another tool; behind it, the history reads as what happened, which is that mirador absorbed the carillon daemon and then took its name.

## Left for a hand

The directory is still `mirador/` and the GitHub repository still answers to that name; both are the operator's to rename. And `carillon/cli`, the daemon this repository absorbed, now claims the same name with nothing left in it that is not here, so it is to be archived rather than kept in step.
