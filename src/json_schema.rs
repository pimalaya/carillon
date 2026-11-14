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
//! [`JsonSchemaCommand`]: pimalaya_cli::clap::commands::json_schema::JsonSchemaCommand

use pimalaya_cli::{clap::commands::json_schema::JsonSchemaRegistry, error::ErrorReport};

use crate::error::ErrorCode;

/// Builds the command-to-schema registry consumed by `json-schema <DIR>`.
///
/// Each value describes the type the command hands to the printer, and
/// `carillon-error` the failure object every command prints instead, its
/// `code` typed by carillon's vocabulary and pimalaya-cli's own.
pub fn registry() -> JsonSchemaRegistry {
    let mut registry = JsonSchemaRegistry::new("carillon", env!("CARGO_PKG_VERSION"));

    registry.insert::<crate::check::CheckOutput>("carillon-check");
    registry.insert::<ErrorReport<ErrorCode>>("carillon-error");

    registry
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use pimalaya_cli::{clap::commands::json_schema::output_schema, error::ErrorReport};
    use serde_json::json;

    use super::registry;
    use crate::{check::CheckOutput, cli::Cli, error::ErrorCode};

    /// The runnable commands printing nothing, so described by no schema:
    /// the bare binary prints its help, and `watch` reports through its
    /// hooks.
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

    #[test]
    fn the_failure_code_is_typed_by_both_vocabularies() {
        let schema = output_schema::<ErrorReport<ErrorCode>>().to_string();

        for code in ["usage-invalid", "permission-denied", "store-version"] {
            assert!(schema.contains(&format!("\"{code}\"")), "{code}: {schema}");
        }
    }

    #[test]
    fn a_check_row_requires_its_code() {
        let schema = output_schema::<CheckOutput>();
        let row = &schema["$defs"]["BackendCheck"];

        assert_eq!(
            row["required"],
            json!(["backend", "ok", "code", "error"]),
            "{schema}"
        );
    }
}
