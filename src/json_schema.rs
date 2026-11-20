//! # JSON Schema registry
//!
//! Maps a CLI-invocation key, the command path joined with hyphens and
//! prefixed `carillon-`, to the JSON Schema of that command's `--json`
//! payload. [`JsonSchemaCommand`] writes one file per entry. The
//! `carillon-error` key describes the failure object instead.
//!
//! Every runnable command appears, `watch` twice: its result under
//! `carillon-watch` and the events it prints before it under
//! `carillon-watch.events`. `check`, the hidden former name of `account
//! check`, keeps its `carillon-check` key beside `carillon-account-check`.
//! pimalaya-cli registers its own `completion`, `manual` and
//! `json-schema`.
//!
//! [`JsonSchemaCommand`]: pimalaya_cli::clap::commands::json_schema::JsonSchemaCommand

use pimalaya_cli::{
    account::{AccountCheckOutput, AccountListOutput},
    clap::commands::json_schema::JsonSchemaRegistry,
    error::ErrorReport,
};

use crate::error::ErrorCode;

/// Builds the command-to-schema registry consumed by `json-schema <DIR>`.
///
/// Each value describes the type the command hands to the printer, and
/// `carillon-error` the failure object every command prints instead, its
/// `code` typed by carillon's vocabulary and pimalaya-cli's own.
pub fn registry() -> JsonSchemaRegistry {
    let mut registry = JsonSchemaRegistry::new("carillon", env!("CARGO_PKG_VERSION"));

    registry.insert::<AccountListOutput>("carillon-account-list");
    registry.insert::<AccountCheckOutput<ErrorCode>>("carillon-account-check");
    registry.insert::<AccountCheckOutput<ErrorCode>>("carillon-check");
    registry.insert::<crate::stream::WatchOutput>("carillon-watch");
    registry.insert_events::<crate::stream::WatchStreamEvent>("carillon-watch");
    registry.insert::<ErrorReport<ErrorCode>>("carillon-error");

    registry
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use pimalaya_cli::{clap::commands::json_schema::output_schema, error::ErrorReport};
    use serde_json::json;

    use super::registry;
    use crate::{cli::Cli, error::ErrorCode};

    /// The runnable commands printing nothing, so described by no schema:
    /// the bare binary prints its help.
    const SILENT: &[&str] = &["carillon"];

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

        for code in [
            "usage-invalid",
            "request-origin",
            "permission-denied",
            "store-version",
            "account-missing",
            "account-default-missing",
            "account-default-several",
            "backend-not-built",
        ] {
            assert!(schema.contains(&format!("\"{code}\"")), "{code}: {schema}");
        }
    }

    /// Each event `watch` prints is described, tagged by `event`.
    #[test]
    fn the_watch_events_are_described() {
        let events = registry().events["carillon-watch"].to_string();

        for event in [
            "session-opened",
            "session-lost",
            "changed",
            "hook-failed",
            "alarm-fired",
            "item-added",
            "flag-removed",
        ] {
            assert!(
                events.contains(&format!("\"{event}\"")),
                "{event}: {events}"
            );
        }
    }

    /// `account check` prints the family's check, its row code typed by
    /// carillon's vocabulary, and `check` the same under its former key.
    #[test]
    fn a_check_row_requires_its_code() {
        let registry = registry();
        let schema = &registry.schemas["carillon-account-check"];
        let row = &schema["$defs"]["BackendCheck"];

        assert_eq!(schema["title"], "AccountCheckOutput", "{schema}");
        assert_eq!(
            row["required"],
            json!(["backend", "ok", "code", "error", "tls"]),
            "{schema}"
        );
        assert!(schema["$defs"].get("ErrorCode").is_some(), "{schema}");
        assert_eq!(schema, &registry.schemas["carillon-check"]);
    }

    /// `account list` prints the family's rows, `notBuilt` included.
    #[test]
    fn an_account_row_carries_what_the_build_leaves_out() {
        let schema = &registry().schemas["carillon-account-list"];
        let row = &schema["$defs"]["AccountRow"];

        assert_eq!(
            row["required"],
            json!(["name", "default", "backends", "notBuilt"]),
            "{schema}"
        );
    }
}
