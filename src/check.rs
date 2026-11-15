//! # Check
//!
//! The `carillon check` command, opening the backends an account declares
//! so a credential or connectivity error surfaces before the first watch.
//!
//! Mirrors `himalaya account check`: each backend `--backend` allows is
//! tried in turn, and the result collected into a per-backend report.

#[cfg(feature = "dav")]
use std::sync::{Arc, atomic::AtomicBool};
use std::{fmt, path::PathBuf};

use anyhow::{Result, bail};
use clap::Parser;
use pimalaya_cli::{error::PartialOutcome, printer::Printer};
#[cfg(network)]
use pimalaya_config::secret::SecretResolver;
use pimalaya_config::toml::TomlConfig;
use schemars::JsonSchema;
use serde::Serialize;

#[cfg(feature = "maildir")]
use crate::config::MaildirConfig;
#[cfg(feature = "gcal")]
use crate::gcal;
#[cfg(feature = "gmail")]
use crate::gmail;
#[cfg(feature = "gpeople")]
use crate::gpeople;
#[cfg(feature = "msgraph")]
use crate::msgraph;
#[cfg(feature = "pimdir")]
use crate::pimdir;
use crate::{
    backend::Backend,
    config::Config,
    error::{self, ErrorCode},
};
#[cfg(feature = "dav")]
use crate::{
    config::DavServer,
    dav::{self, DavHome},
};
#[cfg(feature = "imap")]
use crate::{config::ImapConfig, imap};
#[cfg(feature = "jmap")]
use crate::{config::JmapConfig, jmap};

/// Validate the account configuration.
///
/// Every backend `--backend` allows on the account, `-a` naming it or the
/// default one, is opened the way a watch would, so a bad credential or
/// an unreachable server is reported here rather than at the first change.
/// When one failed, the command exits 3 once the report is printed.
#[derive(Debug, Parser)]
pub struct CheckCommand;

impl CheckCommand {
    /// Opens each allowed backend of the account and reports on it.
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;

        let (name, account_config) = config.take_account(account_name)?;

        let mut report = CheckOutput {
            account: name,
            backends: Vec::new(),
        };

        // NOTE: one resolver for the whole account, so two backends
        // naming one credential command unlock its store once.
        #[cfg(network)]
        let mut resolver = SecretResolver::new();

        #[cfg(feature = "imap")]
        if backend.allows_imap()
            && let Some(imap_config) = account_config.imap.clone()
        {
            report.backends.push(check_imap(imap_config, &mut resolver));
        }

        #[cfg(not(feature = "imap"))]
        if backend.allows_imap() && account_config.imap.is_some() {
            let err = error::backend_not_built("imap", "imap");
            report.backends.push(BackendCheck::from("imap", Err(err)));
        }

        #[cfg(feature = "jmap")]
        if backend.allows_jmap()
            && let Some(jmap_config) = account_config.jmap.clone()
        {
            report.backends.push(check_jmap(jmap_config, &mut resolver));
        }

        #[cfg(not(feature = "jmap"))]
        if backend.allows_jmap() && account_config.jmap.is_some() {
            let err = error::backend_not_built("jmap", "jmap");
            report.backends.push(BackendCheck::from("jmap", Err(err)));
        }

        #[cfg(feature = "maildir")]
        if backend.allows_maildir()
            && let Some(maildir_config) = account_config.maildir.clone()
        {
            report.backends.push(check_maildir(maildir_config));
        }

        #[cfg(not(feature = "maildir"))]
        if backend.allows_maildir() && account_config.maildir.is_some() {
            let err = error::backend_not_built("maildir", "maildir");
            report
                .backends
                .push(BackendCheck::from("maildir", Err(err)));
        }

        #[cfg(feature = "dav")]
        if backend.allows_caldav()
            && let Some(caldav_config) = account_config.caldav.clone()
        {
            report.backends.push(check_dav(
                "caldav",
                DavHome::Calendars,
                caldav_config.server(),
                &caldav_config.calendar,
                &mut resolver,
            ));
        }

        #[cfg(not(feature = "dav"))]
        if backend.allows_caldav() && account_config.caldav.is_some() {
            let err = error::backend_not_built("caldav", "dav");
            report.backends.push(BackendCheck::from("caldav", Err(err)));
        }

        #[cfg(feature = "dav")]
        if backend.allows_carddav()
            && let Some(carddav_config) = account_config.carddav.clone()
        {
            report.backends.push(check_dav(
                "carddav",
                DavHome::Addressbooks,
                carddav_config.server(),
                &carddav_config.addressbook,
                &mut resolver,
            ));
        }

        #[cfg(not(feature = "dav"))]
        if backend.allows_carddav() && account_config.carddav.is_some() {
            let err = error::backend_not_built("carddav", "dav");
            report
                .backends
                .push(BackendCheck::from("carddav", Err(err)));
        }

        #[cfg(feature = "msgraph")]
        if backend.allows_msgraph()
            && let Some(msgraph_config) = account_config.msgraph.clone()
        {
            let result = msgraph::probe(&msgraph_config, &mut resolver);
            report.backends.push(BackendCheck::from("msgraph", result));
        }

        #[cfg(not(feature = "msgraph"))]
        if backend.allows_msgraph() && account_config.msgraph.is_some() {
            let err = error::backend_not_built("msgraph", "msgraph");
            report
                .backends
                .push(BackendCheck::from("msgraph", Err(err)));
        }

        #[cfg(feature = "gmail")]
        if backend.allows_gmail()
            && let Some(gmail_config) = account_config.gmail.clone()
        {
            let result = gmail::probe(&gmail_config, &mut resolver);
            report.backends.push(BackendCheck::from("gmail", result));
        }

        #[cfg(not(feature = "gmail"))]
        if backend.allows_gmail() && account_config.gmail.is_some() {
            let err = error::backend_not_built("gmail", "gmail");
            report.backends.push(BackendCheck::from("gmail", Err(err)));
        }

        #[cfg(feature = "gcal")]
        if backend.allows_gcal()
            && let Some(gcal_config) = account_config.gcal.clone()
        {
            let result = gcal::probe(&gcal_config, &mut resolver);
            report.backends.push(BackendCheck::from("gcal", result));
        }

        #[cfg(not(feature = "gcal"))]
        if backend.allows_gcal() && account_config.gcal.is_some() {
            let err = error::backend_not_built("gcal", "gcal");
            report.backends.push(BackendCheck::from("gcal", Err(err)));
        }

        #[cfg(feature = "gpeople")]
        if backend.allows_gpeople()
            && let Some(gpeople_config) = account_config.gpeople.clone()
        {
            let result = gpeople::probe(&gpeople_config, &mut resolver);
            report.backends.push(BackendCheck::from("gpeople", result));
        }

        #[cfg(not(feature = "gpeople"))]
        if backend.allows_gpeople() && account_config.gpeople.is_some() {
            let err = error::backend_not_built("gpeople", "gpeople");
            report
                .backends
                .push(BackendCheck::from("gpeople", Err(err)));
        }

        #[cfg(feature = "pimdir")]
        if backend.allows_pimdir()
            && let Some(pimdir_config) = account_config.pimdir.clone()
        {
            let result = pimdir::probe(&pimdir_config);
            report.backends.push(BackendCheck::from("pimdir", result));
        }

        #[cfg(not(feature = "pimdir"))]
        if backend.allows_pimdir() && account_config.pimdir.is_some() {
            let err = error::backend_not_built("pimdir", "pimdir");
            report.backends.push(BackendCheck::from("pimdir", Err(err)));
        }

        if report.backends.is_empty() {
            bail!("No backend matching `{backend}` is configured for this account");
        }

        let failed = report.backends.iter().any(|check| !check.ok);
        printer.out(report)?;

        // A failed backend exits 3 under its report, with no failure object.
        if failed {
            return Err(PartialOutcome::default().into());
        }

        Ok(())
    }
}

#[cfg(feature = "imap")]
fn check_imap(imap_config: ImapConfig, resolver: &mut SecretResolver) -> BackendCheck {
    // NOTE: opening the session is the check: it runs the same
    // transport, greeting and authentication a watch would.
    let result = imap::open(&imap_config, resolver).map(|_| ());

    BackendCheck::from("imap", result)
}

#[cfg(feature = "jmap")]
fn check_jmap(jmap_config: JmapConfig, resolver: &mut SecretResolver) -> BackendCheck {
    let result = jmap::open(&jmap_config, resolver).map(|_| ());

    BackendCheck::from("jmap", result)
}

#[cfg(feature = "maildir")]
fn check_maildir(maildir_config: MaildirConfig) -> BackendCheck {
    let result = (|| -> Result<()> {
        if !maildir_config.root.is_dir() {
            bail!(
                "Maildir root `{}` does not exist or is not a directory",
                maildir_config.root.display()
            );
        }
        Ok(())
    })();

    BackendCheck::from("maildir", result)
}

#[cfg(feature = "dav")]
fn check_dav(
    backend: &'static str,
    home: DavHome,
    server: DavServer<'_>,
    collection: &str,
    resolver: &mut SecretResolver,
) -> BackendCheck {
    // NOTE: opening proves the transport, and one report the credential
    // and that the collection is there, as a first poll would.
    let shutdown = Arc::new(AtomicBool::new(false));
    let result = dav::probe(server, home, collection, &shutdown, resolver);

    BackendCheck::from(backend, result)
}

/// The `check` output: one account, and one line per backend tried.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutput {
    /// The account that was checked.
    pub account: String,
    /// One entry per backend `--backend` allowed on it.
    pub backends: Vec<BackendCheck>,
}

/// The outcome of opening one backend.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendCheck {
    /// The backend name, as the configuration spells it.
    pub backend: &'static str,
    /// Whether it opened.
    pub ok: bool,
    /// The stable code of the failure, `null` when it opened or the failure
    /// carries none.
    pub code: Option<ErrorCode>,
    /// Why it did not, rendered with its causes.
    pub error: Option<String>,
}

impl BackendCheck {
    /// Folds one backend's outcome into its report entry.
    fn from(backend: &'static str, result: Result<()>) -> Self {
        match result {
            Ok(()) => Self {
                backend,
                ok: true,
                code: None,
                error: None,
            },
            Err(err) => Self {
                backend,
                ok: false,
                code: error::code(&err),
                error: Some(format!("{err:#}")),
            },
        }
    }
}

impl fmt::Display for CheckOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Account: {}", self.account)?;
        for check in &self.backends {
            match &check.error {
                None => writeln!(f, "  {}: OK", check.backend)?,
                Some(err) => writeln!(f, "  {}: FAIL ({err})", check.backend)?,
            }
        }
        Ok(())
    }
}
