# Migration guide

This page lists the changes most likely to bite when upgrading. The full configuration schema lives in [config.sample.toml](./config.sample.toml), and the exhaustive list of changes in [CHANGELOG.md](./CHANGELOG.md).

## From 0.2 to the next release

- **The wizard is gone**: `carillon configure` (alias `wizard`) no longer exists, and nothing offers to write a configuration. An existing configuration keeps working; a new one is written by hand from [config.sample.toml](./config.sample.toml): copy it to $XDG_CONFIG_HOME/carillon/config.toml, keep one backend and the hooks you want, delete the rest, then run `carillon check`. Run at a terminal with no configuration, any command prints the commands doing the copy.
- **An unknown key is refused at every level**, the root and the account included, naming the key and its line, where carillon ignored it there: fix a misspelt key, delete a key another tool reads (an account shared with himalaya goes in its own file), and build with the cargo feature of every backend the configuration declares. A key carillon moved under each backend before 0.1 is refused naming where it went, and was silently ignored until now:
  - `folder`, `mailbox` and `collection` on the account: name the collection under its backend, `imap.mailbox`, `maildir.mailbox`, `caldav.calendar`, `carddav.addressbook` and so on.
  - `watch` on the account: move it under its backend, `imap.watch.poll.interval` for instance.
  - `hooks` on the account: move each hook under its backend as `<backend>.hook`, `hooks.on-message-added` becoming `imap.hook.on-message-added`. Those hooks never fired while the table sat on the account.
  - `dav`: watch a calendar with a `caldav` block or an addressbook with a `carddav` block.
- **`-c` takes one path per flag**: a value is no longer split on `:`. Write `-c a.toml -c b.toml` where you wrote `-c a.toml:b.toml`; the first file is still the base and the next ones are merged on top.
- **`CARILLON_CONFIG` is no longer read**: pass the path with `-c` instead, for example in the systemd unit or the script starting the watch. Left in the environment, the variable is ignored and the default paths are read.
- **A pimdir watch reads pimdir draft-11 stores**, the ones neverest writes after v0.3.0 (io-pimdir 0.12). A store an earlier neverest wrote is refused (`store-version`): reinstall it with the current neverest, as [its migration guide](https://github.com/pimalaya/neverest/blob/master/MIGRATION.md) describes, before watching it. The `pimdir` backend is new since 0.2, so this concerns a git build only.
- **`-a default` names the account marked default** in `watch` as it already did in `check`, and several accounts marked default are refused: keep `default = true` on one.
- **`check` exits 3 when a backend failed**, after printing its report, where it exited 0: a script reading the exit status of `carillon check` now sees the failure. Exit 1 stays a failure of the command itself (no configuration, an unknown account), and exit 2 a command line carillon refused.
- **Failures carry a stable code**: under `--json`, the failure object and each `check` row carry `code`, `null` for a failure carrying none, a check row now reading `{backend, ok, code, error}`. Match on `code` (`permission-denied`, `quota-exceeded`, `rate-limited`, `store-version` and the other pimdir store codes, `usage-invalid`) rather than on the wording of `error`, which may change. A command line refused under `--json` prints its failure object, coded `usage-invalid`, on stdout instead of clap's message on stderr.
