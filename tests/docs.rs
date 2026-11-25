//! # README tests
//!
//! The README's budgets of the guidelines: the Features bullets and the
//! section order (readme-008, readme-007), and its terminal block printing
//! what it shows (readme-006).

use std::fs;

/// The README, read from the repository root.
fn readme() -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")).unwrap()
}

/// The README's `## ` sections, in order.
fn readme_sections(readme: &str) -> Vec<&str> {
    readme
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect()
}

#[test]
fn the_readme_features_are_eight_short_bullets_at_most() {
    let readme = readme();
    let features: Vec<&str> = readme
        .split("\n## Features\n")
        .nth(1)
        .expect("a Features section")
        .split("\n## ")
        .next()
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();

    assert!(!features.is_empty() && features.len() <= 8, "{features:#?}");
    for feature in features {
        assert!(feature.chars().count() <= 120, "{feature}");
    }
}

#[test]
fn the_readme_sections_follow_the_cli_order() {
    assert_eq!(
        readme_sections(&readme()),
        [
            "Table of contents",
            "Features",
            "Installation",
            "Configuration",
            "Usage",
            "Alternatives",
            "License",
            "Social",
            "Sponsoring",
        ]
    );
}

/// The content of the first fenced block of `lang` in `text`, and the
/// rest of `text` after it.
#[cfg(all(unix, feature = "maildir"))]
fn fenced<'a>(text: &'a str, lang: &str) -> (&'a str, &'a str) {
    let open = format!("```{lang}\n");
    let start = text.find(&open).expect("a fenced block") + open.len();
    let end = start + text[start..].find("\n```").expect("a closing fence");
    (&text[start..end], &text[end + 4..])
}

/// The command line under the description, watching a Maildir a message
/// then reaches before SIGINT stops it, prints the text shown after it.
///
/// The message lands two seconds after the opening, once the first
/// listing took the baseline it is a change against.
#[cfg(all(unix, feature = "maildir"))]
#[test]
fn the_readme_header_block_prints_what_it_shows() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::Duration,
    };

    use tempfile::TempDir;

    let readme = readme();
    let (command, rest) = fenced(&readme, "sh");
    let (shown, _) = fenced(rest, "text");
    let args: Vec<&str> = command
        .strip_prefix("$ carillon ")
        .expect("a carillon command line")
        .split_whitespace()
        .collect();

    let home = TempDir::new().unwrap();
    let root = home.path().join("mail");
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    let config = home.path().join("config.toml");
    let toml = format!(
        "[accounts.personal]\nmaildir.root = {:?}\nmaildir.mailbox = \"INBOX\"\n\
         maildir.watch.poll.interval = 1\n",
        root.display().to_string(),
    );
    fs::write(&config, toml).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_carillon"))
        .arg("-c")
        .arg(&config)
        .args(&args)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    let patience = Duration::from_secs(20);
    let mut printed = vec![lines.recv_timeout(patience).expect("the opening")];

    thread::sleep(Duration::from_secs(2));
    let message = root.join("new").join("1795248000.1.laptop");
    fs::write(message, "Subject: hi\r\n\r\nhi\r\n").unwrap();
    printed.push(lines.recv_timeout(patience).expect("the change"));

    let pid = child.id().to_string();
    let killed = Command::new("kill").args(["-INT", &pid]).status().unwrap();
    assert!(killed.success());
    let status = child.wait().unwrap();
    printed.extend(lines.iter());

    assert!(status.success(), "{printed:?}");
    assert_eq!(printed.join("\n"), shown);
}
