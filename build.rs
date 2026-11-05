use std::env;

use pimalaya_cli::build::{features_env, git_envs, target_envs};

/// The backends opening a socket, which share the TLS, proxy and
/// discovery machinery.
const NETWORK: &[&str] = &["IMAP", "JMAP", "DAV", "MSGRAPH", "GMAIL", "GCAL", "GPEOPLE"];
/// The vendor REST APIs, bearer-only and polled.
const API: &[&str] = &["MSGRAPH", "GMAIL", "GCAL", "GPEOPLE"];
/// The backends firing the alarms of a calendar.
const ALARM: &[&str] = &["DAV", "PIMDIR"];

fn main() {
    features_env(include_str!("./Cargo.toml"));
    target_envs();
    git_envs();

    // NOTE: `backend`, `network`, `api` and `alarm` collapse the repeated
    // backend feature lists: `backend` is set when any backend reporting
    // changes is enabled (pimdir reports none), which is what makes the
    // watch vocabulary and the hook runner reachable, `network` when any
    // backend opening a socket is, `api` when any vendor REST API is, and
    // `alarm` when any backend firing the alarms of a calendar is. Cargo
    // exports `CARGO_FEATURE_<NAME>` for every enabled feature.
    println!("cargo::rustc-check-cfg=cfg(backend)");
    println!("cargo::rustc-check-cfg=cfg(network)");
    println!("cargo::rustc-check-cfg=cfg(api)");
    println!("cargo::rustc-check-cfg=cfg(alarm)");

    let enabled = |feature: &&str| env::var_os(format!("CARGO_FEATURE_{feature}")).is_some();

    if API.iter().any(enabled) {
        println!("cargo::rustc-cfg=api");
    }

    if ALARM.iter().any(enabled) {
        println!("cargo::rustc-cfg=alarm");
    }

    if NETWORK.iter().any(enabled) {
        println!("cargo::rustc-cfg=network");
        println!("cargo::rustc-cfg=backend");
    } else if enabled(&"MAILDIR") {
        println!("cargo::rustc-cfg=backend");
    }
}
