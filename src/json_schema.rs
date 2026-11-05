//! # JSON Schema registry
//!
//! Maps a CLI-invocation key, the command path joined with hyphens and
//! prefixed `carillon-`, to the JSON Schema of that command's `--json`
//! payload. [`JsonSchemaCommand`] writes one file per entry. The
//! `carillon-error` key describes the failure object instead.
//!
//! Every runnable command appears but `watch`, which reports its changes
//! through the hooks and prints nothing; pimalaya-cli registers its own
//! `completion`, `manual` and `json-schema`.
//!
//! [`JsonSchemaCommand`]: pimalaya_cli::clap::commands::JsonSchemaCommand

use pimalaya_cli::{clap::commands::JsonSchemaRegistry, error::ErrorReport};

/// Builds the command-to-schema registry consumed by `json-schema <DIR>`.
///
/// Each value describes the type the command hands to the printer, and
/// `carillon-error` the failure object every command prints instead.
pub fn registry() -> JsonSchemaRegistry {
    let mut registry = JsonSchemaRegistry::new("carillon", env!("CARGO_PKG_VERSION"));

    registry.insert::<crate::check::CheckOutput>("carillon-check");
    #[cfg(feature = "wizard")]
    registry.insert::<crate::wizard::configure::ConfigureOutput>("carillon-configure");
    registry.insert::<ErrorReport>("carillon-error");

    registry
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::registry;
    use crate::cli::Cli;

    /// The runnable commands printing nothing, so described by no schema:
    /// the bare binary prints its help or its welcome, and `watch` reports
    /// through its hooks.
    const SILENT: &[&str] = &["carillon", "carillon-watch"];

    /// Every command that prints is described, and no schema names a
    /// command that is gone.
    #[test]
    fn every_command_is_described() {
        assert_eq!(
            registry().check(&Cli::command(), SILENT),
            Vec::<String>::new()
        );
    }

    #[test]
    fn every_payload_is_an_object() {
        assert_eq!(registry().non_object_roots(), Vec::<String>::new());
    }
}
