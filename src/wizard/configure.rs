//! Command generating an account.
//!
//! The wizard generates, it never edits: it discovers an account from
//! one prompt (see [`super::discover`]), tests it, then hands the
//! resulting `[accounts.<name>]` table back as a file to create, a
//! block to append, or a document on stdout. Everything discovery does
//! not cover, meaning a second collection, the watch options and the
//! hooks beyond the arrival one, is written by hand against the
//! documented sample.
//!
//! It runs from `carillon configure`, and from the offer a bare
//! `carillon` or a command needing an account raises when it finds no
//! configuration. That offer is the only place the wizard introduces
//! itself, with a welcome naming the file that is missing: the command
//! asked for by name goes straight to the prompts.
//!
//! Appending is a plain text append rather than a re-serialization of
//! the whole file, so comments, ordering and hand-written formatting
//! come out untouched. Two rules guard it: the account name must be
//! free, since two `[accounts.<name>]` tables make the whole document
//! fail to parse, and the generated account claims the default only
//! when no other account does.

use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{IsTerminal, Write, stdin, stdout},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use pimalaya_cli::{printer::Printer, prompt};
use pimalaya_config::toml::TomlConfig;
use serde::Serialize;

use crate::{
    config::{CONFIG_SAMPLE_URL, Config},
    wizard::discover,
};

/// Configure an account interactively.
///
/// This command discovers a provider's settings from an email address
/// (or a server URL, or a local folder path), tests the connection,
/// then saves the resulting account to the configuration file, appends
/// it to the one already there, or prints it for you to place by hand.
/// Anything discovery does not cover is written by hand.
#[derive(Debug, Parser)]
pub struct ConfigureCommand;

impl ConfigureCommand {
    /// Runs the wizard, then saves, appends or prints the account.
    ///
    /// No welcome: whoever typed the command knows what it does. The
    /// banner belongs to the offer a missing configuration raises,
    /// which is where the wizard meets someone who did not ask for it.
    /// The account name is not asked either, since it is only the TOML
    /// table key, and renaming it is one edit in the file the wizard
    /// just wrote.
    ///
    /// A redirected stdout (`carillon configure > config.toml`) and the
    /// JSON output both stay non-interactive: the document goes to
    /// stdout and no file is touched. The prompts render on stderr, so
    /// they stay out of the redirected document.
    pub fn execute(self, printer: &mut impl Printer, config_paths: &[PathBuf]) -> Result<()> {
        if !stdin().is_terminal() {
            bail!(
                "Configuring needs a terminal to prompt on, \
                 write the configuration by hand instead: {CONFIG_SAMPLE_URL}"
            );
        }

        let path = Config::target_path(config_paths)?;
        let existing = ExistingConfig::read(&path)?;

        let (base_name, mut account) = discover::run()?;
        let name = account_name(&base_name, existing.as_ref());

        // NOTE: a second `default = true` would make the account every
        // command picks depend on map ordering, so the generated one
        // claims the default only when no other account does.
        let default = !existing.as_ref().is_some_and(|config| config.has_default);
        account.default = default;

        let config = GeneratedConfig {
            document: account.render(&name)?,
            name,
            default,
        };

        if printer.is_json() || !stdout().is_terminal() {
            return printer.out(config);
        }

        match existing {
            Some(_) => append_or_print(printer, &path, config),
            None => save_or_print(printer, &path, config),
        }
    }
}

/// What a configuration file already on disk constrains in the
/// generated account: the names it takes, and whether one of its
/// accounts already claims the default.
struct ExistingConfig {
    names: Vec<String>,
    has_default: bool,
}

impl ExistingConfig {
    /// Reads the configuration at the given path, or `None` when no
    /// file is there.
    ///
    /// A file that fails to parse is an error rather than a `None`:
    /// appending to a broken document would bury the actual problem
    /// under a second one.
    fn read(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }

        let config = Config::from_paths(&[path.to_path_buf()])
            .with_context(|| format!("Read the configuration at {}", path.display()))?;

        Ok(Some(Self {
            names: config.accounts.keys().cloned().collect(),
            has_default: config.accounts.values().any(|account| account.default),
        }))
    }
}

/// The generated account, as the printer takes it.
#[derive(Serialize)]
pub struct GeneratedConfig {
    /// The account name, which is the `[accounts.<name>]` table key.
    name: String,
    /// Whether the account claims the default.
    default: bool,
    /// The rendered TOML document.
    document: String,
}

impl fmt::Display for GeneratedConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // NOTE: the trailing newline terminates the document, and it is
        // also what flushes the line-buffered stdout.
        writeln!(f, "{}", self.document.trim_end())
    }
}

/// Frames carillon, names the configuration file that is missing, and
/// points at the sample for everything the wizard does not cover.
///
/// Printed before the offer a bare `carillon` or a command needing an
/// account raises when it finds no configuration, so the wizard
/// introduces itself to someone who did not ask for it. `configure`
/// skips it, since it was asked for by name.
///
/// On stderr, so a redirected stdout holds the document alone.
pub fn print_welcome(path: &Path) {
    eprintln!();
    eprintln!("Welcome to carillon, the CLI to watch PIM collection changes.");
    eprintln!();
    eprintln!("carillon watches one collection per account, over IMAP, JMAP, CalDAV,");
    eprintln!("CardDAV or a local Maildir, and fires a desktop notification or a command");
    eprintln!("on every change. It needs one account to know what to watch, and no");
    eprintln!("configuration file was found at:");
    eprintln!();
    eprintln!("  {}", path.display());
    eprintln!();
    eprintln!("The wizard discovers a provider's settings from your email address, tests");
    eprintln!("the connection and generates a ready-to-use account watching your inbox,");
    eprintln!("calendar or addressbook. Everything it does not cover is written by hand,");
    eprintln!("and every field is documented at:");
    eprintln!();
    eprintln!("  {CONFIG_SAMPLE_URL}");
    eprintln!();
    eprintln!("At anytime, you can create a new account with the command:");
    eprintln!();
    eprintln!("  carillon configure");
    eprintln!();
}

/// The name discovery proposes, suffixed until the configuration does
/// not already hold it.
///
/// Not prompted: the name is only the TOML table key, and whoever wants
/// another one renames it in the file. It still has to be free, since a
/// second `[accounts.<name>]` table makes the whole document fail to
/// parse, taking the accounts that used to work down with it.
fn account_name(base: &str, existing: Option<&ExistingConfig>) -> String {
    let taken = existing
        .map(|config| config.names.as_slice())
        .unwrap_or(&[]);

    if !taken.iter().any(|name| name == base) {
        return base.to_string();
    }

    let mut suffix = 2;

    loop {
        let name = format!("{base}-{suffix}");

        if !taken.contains(&name) {
            return name;
        }

        suffix += 1;
    }
}

/// Offers to write the generated account to a configuration file that
/// does not exist yet, printing it instead when the offer is declined.
fn save_or_print(printer: &mut impl Printer, path: &Path, config: GeneratedConfig) -> Result<()> {
    let prompt = format!("Save this account to {}?", path.display());

    if !prompt::bool(prompt, true)? {
        return printer.out(config);
    }

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Create the config directory {}", parent.display()))?;
    }

    fs::write(path, config.to_string())
        .with_context(|| format!("Write the config file {}", path.display()))?;

    print_saved(path, &config);

    Ok(())
}

/// Offers to append the generated account to the configuration file
/// already there, printing it instead when the offer is declined.
fn append_or_print(printer: &mut impl Printer, path: &Path, config: GeneratedConfig) -> Result<()> {
    let prompt = format!("Append account `{}` to {}?", config.name, path.display());

    if !prompt::bool(prompt, true)? {
        return printer.out(config);
    }

    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("Open the config file {}", path.display()))?;

    // NOTE: appending text keeps every comment and every hand-written
    // line of the file as they are, which parsing and re-serializing
    // the whole document would not. The leading newline separates the
    // two tables, and terminates the last line when the file ends
    // without one.
    write!(file, "\n{config}")
        .with_context(|| format!("Append to the config file {}", path.display()))?;

    print_saved(path, &config);

    Ok(())
}

/// Tells where the account landed, under which name, and what to run
/// next.
///
/// The name matters here because it was never asked for: an account
/// that did not claim the default is only reachable through `-a`.
fn print_saved(path: &Path, config: &GeneratedConfig) {
    let name = &config.name;

    eprintln!();
    eprintln!("Account `{name}` saved to {}.", path.display());

    if !config.default {
        eprintln!("Another account holds the default, so name this one with `-a {name}`.");
    }

    eprintln!("It notifies you when an item arrives; add the hooks you want beside it.");
    eprintln!("Run `carillon watch` to start watching.");
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    #[cfg(feature = "maildir")]
    use {crate::config::AccountConfig, std::fs};

    static NEXT_CONFIG: AtomicUsize = AtomicUsize::new(0);

    /// A path in the temporary directory no other test writes to.
    fn config_path() -> PathBuf {
        let id = NEXT_CONFIG.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("carillon-configure-{id}.toml"))
    }

    /// A minimal account watching a Maildir root, the one backend
    /// needing no network to describe.
    #[cfg(feature = "maildir")]
    #[cfg_attr(
        not(any(feature = "imap", feature = "jmap", feature = "dav")),
        allow(clippy::needless_update)
    )]
    fn account(default: bool) -> AccountConfig {
        AccountConfig {
            default,
            maildir: Some(crate::config::MaildirConfig {
                mailbox: String::from("."),
                root: PathBuf::from("/tmp/mail"),
                watch: None,
                hook: Default::default(),
            }),
            ..Default::default()
        }
    }

    /// An IMAP account the way the wizard builds one: an endpoint, a
    /// SASL credential read from a command, a hook.
    #[cfg(feature = "imap")]
    fn imap_account() -> crate::config::AccountConfig {
        use std::process::Command;

        use crate::config::{ImapConfig, ItemHook, NotifyConfig, SaslConfig, SaslPlainConfig};

        let mut passwd = Command::new("pass");
        passwd.args(["show", "carillon/posteo"]);

        let mut account = crate::config::AccountConfig {
            default: true,
            ..Default::default()
        };

        account.imap = Some(ImapConfig {
            mailbox: String::from("INBOX"),
            server: String::from("imaps://posteo.de:993"),
            tls: Default::default(),
            starttls: false,
            sasl: Some(SaslConfig::Plain(SaslPlainConfig {
                authzid: None,
                authcid: String::from("me@posteo.net"),
                passwd: pimalaya_config::secret::Secret::Command(passwd),
            })),
            sasl_ir: None,
            id: Default::default(),
            watch: None,
            hook: crate::config::ImapHookConfig {
                on_message_added: Some(ItemHook {
                    notify: Some(NotifyConfig {
                        summary: String::from("New mail in $mailbox from $sender"),
                        body: String::from("$subject"),
                    }),
                    cmd: None,
                }),
                ..Default::default()
            },
        });

        account
    }

    #[cfg(feature = "imap")]
    #[test]
    fn a_generated_imap_account_reads_top_down() {
        let document = imap_account().render("posteo").expect("render the account");
        let lines: Vec<&str> = document.lines().collect();

        // NOTE: what the account is, then what it watches, then where, then
        // the credential authenticating against it.
        assert_eq!(lines[0], "[accounts.posteo]");
        assert_eq!(lines[1], "default = true");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "imap.mailbox = \"INBOX\"");
        assert_eq!(lines[4], "imap.server = \"imaps://posteo.de:993\"");
        assert!(lines[5].starts_with("imap.sasl.plain."));

        // NOTE: the secret renders as the command it is read from, never as
        // a value, and the whole thing parses back.
        assert!(
            document.contains(
                r#"imap.sasl.plain.passwd.command = ["pass", "show", "carillon/posteo"]"#
            )
        );
        let config: Config = toml::from_str(&document).expect("parse the generated config");
        config.accounts["posteo"]
            .validate()
            .expect("the generated hooks name only what they carry");

        // NOTE: a default the config already holds is written, and the
        // untouched quirks are not.
        assert!(!document.contains("starttls"));
        assert!(!document.contains("imap.id"));
        assert!(!document.contains("imap.watch"));
        assert!(!document.contains("imap.tls"));
    }

    #[cfg(feature = "maildir")]
    #[test]
    fn a_generated_account_parses_back() {
        let document = account(true).render("perso").expect("render the account");
        let config: Config = toml::from_str(&document).expect("parse the generated config");
        let account = &config.accounts["perso"];

        assert_eq!(config.accounts.len(), 1);
        assert!(account.default);
        assert_eq!(
            account
                .maildir
                .as_ref()
                .map(|config| config.mailbox.as_str()),
            Some(".")
        );

        // NOTE: every other field is left at its default, so none of them is
        // written: a generated document holds what was configured.
        assert!(!document.contains("imap"));
        assert!(!document.contains("watch"));
        assert!(!document.contains("hook"));

        // NOTE: the account name heads the block, `default` reads before the
        // backend it qualifies, and the collection heads its own group.
        let lines: Vec<&str> = document.lines().collect();
        assert_eq!(lines[0], "[accounts.perso]");
        assert_eq!(lines[1], "default = true");
        assert_eq!(lines[3], "maildir.mailbox = \".\"");
    }

    #[cfg(feature = "maildir")]
    #[test]
    fn an_appended_account_keeps_the_existing_one() {
        let path = config_path();

        // NOTE: no trailing newline, the shape an appended block has to
        // survive without merging into the last line.
        fs::write(
            &path,
            "# my watches\n[accounts.work]\ndefault = true\nmaildir.mailbox = \".\"\nmaildir.root = \"/tmp/work\"",
        )
        .expect("write the existing config");

        let existing = ExistingConfig::read(&path)
            .expect("read the existing config")
            .expect("an existing config");

        assert_eq!(existing.names, ["work"]);
        assert!(existing.has_default);

        let document = account(!existing.has_default)
            .render("perso")
            .expect("render the account");
        let mut file = OpenOptions::new().append(true).open(&path).expect("open");
        write!(file, "\n{document}").expect("append the generated account");
        drop(file);

        let content = fs::read_to_string(&path).expect("read back");
        let config: Config = toml::from_str(&content).expect("parse the appended config");

        assert_eq!(config.accounts.len(), 2);

        // NOTE: exactly one default, and the comment is still there.
        let defaults = config
            .accounts
            .values()
            .filter(|account| account.default)
            .count();
        assert_eq!(defaults, 1);
        assert!(config.accounts["work"].default);
        assert!(content.starts_with("# my watches"));

        fs::remove_file(&path).expect("remove the config");
    }

    #[test]
    fn a_taken_name_gets_a_suffix() {
        let existing = ExistingConfig {
            names: vec!["perso".to_string(), "perso-2".to_string()],
            has_default: true,
        };

        assert_eq!(account_name("perso", None), "perso");
        assert_eq!(account_name("perso", Some(&existing)), "perso-3");
        assert_eq!(account_name("work", Some(&existing)), "work");
    }

    #[test]
    fn a_missing_configuration_constrains_nothing() {
        let existing = ExistingConfig::read(&config_path()).expect("read a missing config");

        assert!(existing.is_none());
    }
}
