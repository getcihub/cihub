//! Starts the `cihub` binary with arguments it refuses.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::process::{Command, Stdio};

fn refuses(args: &[&str]) {
    let status = Command::new(env!("CARGO_BIN_EXE_cihub"))
        .args(args)
        .env_clear()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "{args:?}");
}

#[test]
fn unknown_command_stops() {
    refuses(&[]);
    refuses(&["queue"]);
    refuses(&["queue", "drain", "--repo", "acme"]);
    refuses(&["server", "start"]);
    refuses(&["login"]);
    refuses(&["auth"]);
    refuses(&["auth", "login", "--hostname", "https://cihub.example.com"]);
}
