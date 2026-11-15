//! # Account list
//!
//! The `account list` command, listing every account the configuration
//! declares.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use pimalaya_cli::{
    account::{AccountListOutput, AccountRow},
    printer::Printer,
};

use crate::config::Config;

/// List the accounts declared in the configuration.
///
/// Each row shows the account name, the backends it carries a block
/// for, and `default` on the default account. A backend this build
/// leaves out shows as `<name> (not built)`, its block loading unread.
/// Nothing is opened: `account check` does that.
#[derive(Debug, Parser)]
pub struct AccountListCommand;

impl AccountListCommand {
    /// Loads the configuration, taking no account, and prints its
    /// accounts sorted by name.
    pub fn execute(self, printer: &mut impl Printer, config_paths: &[PathBuf]) -> Result<()> {
        let config = Config::load(config_paths)?;

        let mut accounts: Vec<AccountRow> = config
            .accounts
            .iter()
            .map(|(name, account)| AccountRow {
                name: name.clone(),
                default: account.default,
                backends: account.backends(),
                not_built: account.not_built(),
            })
            .collect();
        accounts.sort_by(|a, b| a.name.cmp(&b.name));

        printer.out(AccountListOutput { accounts })
    }
}
