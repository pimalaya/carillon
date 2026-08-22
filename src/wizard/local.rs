//! Maildir wizard.
//!
//! Nothing is discovered here: the input named a folder, so the only
//! question left is whether it really is one. The root itself is what
//! the account watches, which `.` names in io-maildir's store, and a
//! subfolder is one edit away in the file the wizard just wrote.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::config::{ItemHook, MaildirConfig, MaildirHookConfig, NotifyConfig};

/// Configures Maildir from a folder path, checking it is there: a
/// watch on a directory that does not exist reports nothing, forever,
/// and that is the failure the wizard is meant to catch.
pub fn configure(root: PathBuf) -> Result<MaildirConfig> {
    if !root.is_dir() {
        bail!("No such folder `{}`", root.display());
    }

    Ok(MaildirConfig {
        mailbox: String::from("."),
        root,
        watch: None,
        hook: hook(),
    })
}

/// The hook a generated account fires: a desktop notification on
/// arrival. A Maildir listing knows a file name and no more, so the
/// notification names the item rather than its envelope.
fn hook() -> MaildirHookConfig {
    MaildirHookConfig {
        on_message_added: Some(ItemHook {
            notify: Some(NotifyConfig {
                summary: String::from("New mail in $mailbox"),
                body: String::from("$id"),
            }),
            cmd: None,
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_folder_is_refused() {
        assert!(configure(PathBuf::from("/nonexistent/carillon/maildir")).is_err());
    }

    #[test]
    fn a_generated_account_watches_the_root_and_notifies_on_arrival() {
        let root = std::env::temp_dir();
        let config = configure(root.clone()).expect("configure the maildir");

        assert_eq!(config.mailbox, ".");
        assert_eq!(config.root, root);
        assert!(config.watch.is_none());
        assert!(config.hook.on_message_added.is_some());
    }
}
