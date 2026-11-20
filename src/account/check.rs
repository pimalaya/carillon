//! # Account check
//!
//! The `carillon account check` command, opening the backends an account
//! declares so a credential or connectivity error surfaces before the
//! first watch. `carillon check` runs it too, a hidden alias.
//!
//! Mirrors `himalaya account check`: each backend `--backend` allows is
//! tried in turn, and the result collected into pimalaya-cli's
//! per-backend report.

use std::path::PathBuf;
#[cfg(dav)]
use std::sync::{Arc, atomic::AtomicBool};

use anyhow::{Result, bail};
use clap::Parser;
use pimalaya_cli::{
    account::{AccountCheck, AccountCheckOutput, BackendCheck},
    error::{PartialOutcome, with_code},
    printer::Printer,
};
#[cfg(network)]
use pimalaya_config::secret::SecretResolver;
use pimalaya_config::toml::TomlConfig;
#[cfg(dav)]
use url::Url;

#[cfg(feature = "maildir")]
use crate::config::maildir::MaildirConfig;
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
#[cfg(dav)]
use crate::{
    config::dav::DavServer,
    dav::{self, DavHome},
};
#[cfg(feature = "imap")]
use crate::{config::imap::ImapConfig, imap};
#[cfg(feature = "jmap")]
use crate::{config::jmap::JmapConfig, jmap};

/// Validate the account configuration.
///
/// Every backend `--backend` allows on the account, `-a` naming it or the
/// default one, is opened the way a watch would, so a bad credential or
/// an unreachable server is reported here rather than at the first change.
/// When one failed, the command exits 3 once the report is printed.
#[derive(Debug, Parser)]
pub struct AccountCheckCommand;

impl AccountCheckCommand {
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

        let mut backends = Vec::new();

        // NOTE: one resolver for the whole account, so two backends
        // naming one credential command unlock its store once.
        #[cfg(network)]
        let mut resolver = SecretResolver::new();

        #[cfg(feature = "imap")]
        if backend.allows_imap()
            && let Some(imap_config) = account_config.imap.clone()
        {
            backends.push(check_imap(imap_config, &mut resolver));
        }

        #[cfg(not(feature = "imap"))]
        if backend.allows_imap() && account_config.imap.is_some() {
            let err = error::backend_not_built("imap");
            backends.push(row("imap", Err(err)));
        }

        #[cfg(feature = "jmap")]
        if backend.allows_jmap()
            && let Some(jmap_config) = account_config.jmap.clone()
        {
            backends.push(check_jmap(jmap_config, &mut resolver));
        }

        #[cfg(not(feature = "jmap"))]
        if backend.allows_jmap() && account_config.jmap.is_some() {
            let err = error::backend_not_built("jmap");
            backends.push(row("jmap", Err(err)));
        }

        #[cfg(feature = "maildir")]
        if backend.allows_maildir()
            && let Some(maildir_config) = account_config.maildir.clone()
        {
            backends.push(check_maildir(maildir_config));
        }

        #[cfg(not(feature = "maildir"))]
        if backend.allows_maildir() && account_config.maildir.is_some() {
            let err = error::backend_not_built("maildir");
            backends.push(row("maildir", Err(err)));
        }

        #[cfg(feature = "caldav")]
        if backend.allows_caldav()
            && let Some(caldav_config) = account_config.caldav.clone()
        {
            backends.push(check_dav(
                "caldav",
                DavHome::Calendars,
                caldav_config.server(),
                &caldav_config.calendar,
                &mut resolver,
            ));
        }

        #[cfg(not(feature = "caldav"))]
        if backend.allows_caldav() && account_config.caldav.is_some() {
            let err = error::backend_not_built("caldav");
            backends.push(row("caldav", Err(err)));
        }

        #[cfg(feature = "carddav")]
        if backend.allows_carddav()
            && let Some(carddav_config) = account_config.carddav.clone()
        {
            backends.push(check_dav(
                "carddav",
                DavHome::Addressbooks,
                carddav_config.server(),
                &carddav_config.addressbook,
                &mut resolver,
            ));
        }

        #[cfg(not(feature = "carddav"))]
        if backend.allows_carddav() && account_config.carddav.is_some() {
            let err = error::backend_not_built("carddav");
            backends.push(row("carddav", Err(err)));
        }

        #[cfg(feature = "msgraph")]
        if backend.allows_msgraph()
            && let Some(msgraph_config) = account_config.msgraph.clone()
        {
            let result = msgraph::probe(&msgraph_config, &mut resolver);
            let mut check = row("msgraph", result);
            check.tls = Some(msgraph_config.tls.check());
            backends.push(check);
        }

        #[cfg(not(feature = "msgraph"))]
        if backend.allows_msgraph() && account_config.msgraph.is_some() {
            let err = error::backend_not_built("msgraph");
            backends.push(row("msgraph", Err(err)));
        }

        #[cfg(feature = "gmail")]
        if backend.allows_gmail()
            && let Some(gmail_config) = account_config.gmail.clone()
        {
            let result = gmail::probe(&gmail_config, &mut resolver);
            let mut check = row("gmail", result);
            check.tls = Some(gmail_config.tls.check());
            backends.push(check);
        }

        #[cfg(not(feature = "gmail"))]
        if backend.allows_gmail() && account_config.gmail.is_some() {
            let err = error::backend_not_built("gmail");
            backends.push(row("gmail", Err(err)));
        }

        #[cfg(feature = "gcal")]
        if backend.allows_gcal()
            && let Some(gcal_config) = account_config.gcal.clone()
        {
            let result = gcal::probe(&gcal_config, &mut resolver);
            let mut check = row("gcal", result);
            check.tls = Some(gcal_config.tls.check());
            backends.push(check);
        }

        #[cfg(not(feature = "gcal"))]
        if backend.allows_gcal() && account_config.gcal.is_some() {
            let err = error::backend_not_built("gcal");
            backends.push(row("gcal", Err(err)));
        }

        #[cfg(feature = "gpeople")]
        if backend.allows_gpeople()
            && let Some(gpeople_config) = account_config.gpeople.clone()
        {
            let result = gpeople::probe(&gpeople_config, &mut resolver);
            let mut check = row("gpeople", result);
            check.tls = Some(gpeople_config.tls.check());
            backends.push(check);
        }

        #[cfg(not(feature = "gpeople"))]
        if backend.allows_gpeople() && account_config.gpeople.is_some() {
            let err = error::backend_not_built("gpeople");
            backends.push(row("gpeople", Err(err)));
        }

        #[cfg(feature = "pimdir")]
        if backend.allows_pimdir()
            && let Some(pimdir_config) = account_config.pimdir.clone()
        {
            let result = pimdir::probe(&pimdir_config);
            backends.push(row("pimdir", result));
        }

        #[cfg(not(feature = "pimdir"))]
        if backend.allows_pimdir() && account_config.pimdir.is_some() {
            let err = error::backend_not_built("pimdir");
            backends.push(row("pimdir", Err(err)));
        }

        if backends.is_empty() && backend == Backend::Auto {
            bail!("No backend matching `{backend}` is configured for this account");
        }

        if backends.is_empty() {
            let backend = backend.to_string();
            return Err(error::backend_missing(&name, &backend));
        }

        let report = AccountCheckOutput {
            accounts: vec![AccountCheck {
                account: name,
                backends,
            }],
        };

        let ok = report.ok();
        printer.out(report)?;

        // A failed backend exits 3 under its report, with no failure object.
        if !ok {
            return Err(PartialOutcome::default().into());
        }

        Ok(())
    }
}

/// Folds one backend's outcome into its row, coded as every carillon
/// failure is: a coded error in its chain, else a typed cause a backend's
/// reader knows.
fn row(backend: &'static str, result: Result<()>) -> BackendCheck<ErrorCode> {
    BackendCheck::new(backend, result.map_err(|err| with_code(err, error::code)))
}

#[cfg(feature = "imap")]
fn check_imap(imap_config: ImapConfig, resolver: &mut SecretResolver) -> BackendCheck<ErrorCode> {
    // NOTE: opening the session is the check: it runs the same
    // transport, greeting and authentication a watch would.
    let result = imap::open(&imap_config, resolver).map(|_| ());

    let server = imap::parse_server(&imap_config.server);
    let plain = server.is_ok_and(|url| url.scheme() == "imap") && !imap_config.starttls;

    let mut check = row("imap", result);
    check.tls = (!plain).then(|| imap_config.tls.check());
    check
}

#[cfg(feature = "jmap")]
fn check_jmap(jmap_config: JmapConfig, resolver: &mut SecretResolver) -> BackendCheck<ErrorCode> {
    let result = jmap::open(&jmap_config, resolver).map(|_| ());

    let server = jmap::parse_server(&jmap_config.server);
    let plain = server.is_ok_and(|url| url.scheme() == "http");

    let mut check = row("jmap", result);
    check.tls = (!plain).then(|| jmap_config.tls.check());
    check
}

#[cfg(feature = "maildir")]
fn check_maildir(maildir_config: MaildirConfig) -> BackendCheck<ErrorCode> {
    let result = (|| -> Result<()> {
        if !maildir_config.root.is_dir() {
            bail!(
                "Maildir root `{}` does not exist or is not a directory",
                maildir_config.root.display()
            );
        }
        Ok(())
    })();

    row("maildir", result)
}

#[cfg(dav)]
fn check_dav(
    backend: &'static str,
    home: DavHome,
    server: DavServer<'_>,
    collection: &str,
    resolver: &mut SecretResolver,
) -> BackendCheck<ErrorCode> {
    // NOTE: opening proves the transport, and one report the credential
    // and that the collection is there, as a first poll would.
    let url = server.home.cloned();
    let url = url.or_else(|| server.server.and_then(|server| Url::parse(server).ok()));
    let plain = url.is_some_and(|url| url.scheme() == "http");
    let tls = (!plain).then(|| server.tls.check());

    let shutdown = Arc::new(AtomicBool::new(false));
    let result = dav::probe(server, home, collection, &shutdown, resolver);

    let mut check = row(backend, result);
    check.tls = tls;
    check
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use pimalaya_cli::account::{AccountCheck, AccountCheckOutput, BackendCheck};
    use pimalaya_config::toml::AccountError;

    use crate::{account::check::row, error::ErrorCode};

    /// The rows align past the longest label as every family `account
    /// check` prints them, a server's text unable to break its line.
    #[test]
    fn the_report_prints_on_the_family_layout() {
        let report = AccountCheckOutput {
            accounts: vec![AccountCheck {
                account: String::from("perso"),
                backends: vec![
                    BackendCheck::<ErrorCode>::new("maildir", Ok(())),
                    BackendCheck::new("imap", Err(anyhow!("NO \x1b[2Jrefused\nagain"))),
                ],
            }],
        };

        let expected = "\
Account:  perso
maildir:  OK
imap:     FAIL (NO \u{fffd}[2Jrefused\u{fffd}again)
";
        assert_eq!(expected, report.to_string());
    }

    /// A row takes the code carillon reads off a typed cause, which
    /// pimalaya-cli's chain walk alone would leave out, its text kept.
    #[test]
    fn a_row_is_coded_by_carillons_readers() {
        let err = anyhow::Error::new(AccountError::NoDefault).context("Cannot check");
        let check = row("imap", Err(err));

        assert!(!check.ok);
        assert_eq!(check.code, Some(ErrorCode::AccountDefaultMissing));
        let error = check.error.unwrap();
        assert!(
            error.starts_with("Cannot check: No default account"),
            "{error}"
        );

        let check = row("imap", Err(anyhow!("refused")));
        assert_eq!(check.code, None);
    }
}
