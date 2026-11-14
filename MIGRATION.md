# Migration guide

This page lists the changes most likely to bite when upgrading. The full configuration schema lives in [config.sample.toml](./config.sample.toml), and the exhaustive list of changes in [CHANGELOG.md](./CHANGELOG.md).

## From 0.2 to the next release

- **The wizard is gone**: `carillon configure` (alias `wizard`) no longer exists, and nothing offers to write a configuration. An existing configuration keeps working; a new one is written by hand from [config.sample.toml](./config.sample.toml): copy it to $XDG_CONFIG_HOME/carillon/config.toml, keep one backend and the hooks you want, delete the rest, then run `carillon check`. Run at a terminal with no configuration, any command prints the commands doing the copy.
- **`-c` takes one path per flag**: a value is no longer split on `:`. Write `-c a.toml -c b.toml` where you wrote `-c a.toml:b.toml`; the first file is still the base and the next ones are merged on top.
- **`CARILLON_CONFIG` is no longer read**: pass the path with `-c` instead, for example in the systemd unit or the script starting the watch. Left in the environment, the variable is ignored and the default paths are read.
- **A pimdir watch reads pimdir draft-11 stores**, the ones neverest writes after v0.3.0 (io-pimdir 0.12). A store an earlier neverest wrote is refused (`store-version`): reinstall it with the current neverest, as [its migration guide](https://github.com/pimalaya/neverest/blob/master/MIGRATION.md) describes, before watching it. The `pimdir` backend is new since 0.2, so this concerns a git build only.
- **`-a default` names the account marked default** in `watch` as it already did in `check`, and several accounts marked default are refused: keep `default = true` on one.
