//! Starts the `cihub` binary with arguments or a configuration it refuses.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::process::{Command, Stdio};

fn refuses(args: &[&str], vars: &[(&str, &str)]) {
    let status = Command::new(env!("CARGO_BIN_EXE_cihub"))
        .args(args)
        .env_clear()
        .envs(vars.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "{args:?}");
}

#[test]
fn invalid_configuration_stops_the_start() {
    refuses(&["server", "start"], &[]);
    refuses(
        &["server", "start", "--config", "/missing/server.yaml"],
        &[],
    );
}

#[test]
fn unknown_command_stops_the_start() {
    let database = [("CIHUB_DATABASE_URL", "postgres://127.0.0.1/cihub")];
    refuses(&[], &database);
    refuses(&["server"], &database);
    refuses(&["server", "start", "cron"], &database);
    refuses(&["server", "bootstrap"], &database);
    refuses(&["queue"], &database);
    refuses(&["queue", "drain", "--repo", "acme"], &database);
}
