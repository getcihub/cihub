//! Starts the `cihub-server` binary with a configuration it refuses.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::process::{Command, Stdio};

fn refuses(args: &[&str], vars: &[(&str, &str)]) {
    let status = Command::new(env!("CARGO_BIN_EXE_cihub-server"))
        .args(args)
        .env_clear()
        .envs(vars.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn invalid_configuration_stops_the_start() {
    refuses(&[], &[("CIHUB_ROLES", "cron")]);
    refuses(&[], &[]);
}

#[test]
fn unknown_command_stops_the_start() {
    refuses(
        &["serve"],
        &[("CIHUB_DATABASE_URL", "postgres://127.0.0.1/cihub")],
    );
}
