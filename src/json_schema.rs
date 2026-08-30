//! # JSON Schema registry
//!
//! Maps a CLI-invocation key, the command path joined with hyphens and
//! prefixed `carillon-`, to the JSON Schema of that command's `--json`
//! payload. [`JsonSchemaCommand`] writes one file per entry.
//!
//! Only the commands that hand data to the printer are in here: `watch`
//! reports its changes through the hooks and prints nothing, and
//! `completion` and `manual` write files.
//!
//! [`JsonSchemaCommand`]: pimalaya_cli::clap::commands::JsonSchemaCommand

use std::collections::BTreeMap;

use schemars::schema_for;
use serde_json::Value;

/// Builds the command-to-schema map consumed by `json-schema <DIR>`.
///
/// Each value describes the type the command hands to the printer.
pub fn schemas() -> BTreeMap<String, Value> {
    let mut schemas = BTreeMap::new();

    macro_rules! insert {
        ($key:expr, $ty:ty) => {
            schemas.insert(
                $key.to_string(),
                serde_json::to_value(schema_for!($ty)).unwrap(),
            );
        };
    }

    insert!("carillon-check", crate::check::CheckOutput);
    insert!(
        "carillon-configure",
        crate::wizard::configure::ConfigureOutput
    );

    schemas
}

#[cfg(test)]
mod tests {
    use clap::{Command, CommandFactory};

    use super::schemas;
    use crate::cli::Cli;

    /// Every command path the parser answers to, dash-joined under
    /// `prefix`, which is how a registry key is spelled.
    fn command_keys(command: &Command, prefix: &str) -> Vec<String> {
        let mut keys = Vec::new();

        for sub in command.get_subcommands() {
            let key = format!("{prefix}-{}", sub.get_name());
            keys.extend(command_keys(sub, &key));
            keys.push(key);
        }

        keys
    }

    /// A renamed or removed subcommand must not leave behind a schema
    /// nobody can ask for.
    #[test]
    fn every_registered_key_names_a_command() {
        let cli = Cli::command();
        let keys = command_keys(&cli, cli.get_name());

        for key in schemas().keys() {
            assert!(keys.contains(key), "`{key}` names no command");
        }
    }
}
