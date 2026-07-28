use pimalaya_cli::build::{features_env, target_envs};

fn main() {
    features_env(include_str!("./Cargo.toml"));
    target_envs();

    // NOTE: pimalaya-cli's git_envs() drives libgit2 describe, which panics on
    // a tagless repository. long_version! only needs GIT_DESCRIBE / GIT_REV, so
    // emit them directly (a tagged release build can forward the real values).
    println!(
        "cargo::rustc-env=GIT_DESCRIBE=v{}",
        env!("CARGO_PKG_VERSION")
    );
    println!("cargo::rustc-env=GIT_REV=unknown");
}
