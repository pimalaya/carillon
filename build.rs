use std::env;

use pimalaya_cli::build::{features_env, git_envs, target_envs};

fn main() {
    features_env(include_str!("./Cargo.toml"));
    target_envs();
    git_envs();

    // NOTE: `backend` collapses the repeated backend feature list: it is
    // set when any backend is enabled, which is what makes the watch
    // vocabulary and the hook runner reachable. Cargo exports
    // `CARGO_FEATURE_<NAME>` for every enabled feature.
    println!("cargo::rustc-check-cfg=cfg(backend)");

    let backend = ["IMAP", "JMAP", "MAILDIR", "DAV"]
        .iter()
        .any(|feature| env::var_os(format!("CARGO_FEATURE_{feature}")).is_some());

    if backend {
        println!("cargo::rustc-cfg=backend");
    }
}
