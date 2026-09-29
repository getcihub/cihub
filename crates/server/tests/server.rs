//! Starts the `cihub-server` binary, reads its probes and stops it with
//! `SIGTERM`.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

fn start(addr: &str, roles: Option<&str>) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cihub-server"));
    command
        .env_clear()
        .env("CIHUB_INTERNAL_ADDR", addr)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(roles) = roles {
        command.env("CIHUB_ROLES", roles);
    }
    command.spawn().unwrap()
}

/// Sends `GET path` and returns the status line and the body.
fn get(addr: &str, path: &str) -> Option<(String, String)> {
    let mut stream = TcpStream::connect(addr).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let (head, body) = response.split_once("\r\n\r\n")?;
    Some((head.lines().next()?.to_owned(), body.to_owned()))
}

fn wait_for(addr: &str, path: &str) -> (String, String) {
    for _ in 0..100 {
        if let Some(response) = get(addr, path) {
            return response;
        }
        sleep(Duration::from_millis(50));
    }
    panic!("{addr}{path} never answered");
}

fn stop(mut child: Child) {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
}

#[test]
fn all_in_one_answers_and_stops() {
    let addr = free_addr();
    let child = start(&addr, None);
    for path in ["/healthz", "/readyz"] {
        let (status, body) = wait_for(&addr, path);
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(
            body.contains(r#""roles":["api","scheduler","worker","notifier"]"#),
            "{body}"
        );
        assert!(body.contains(r#""name":"cihub-server""#), "{body}");
    }
    stop(child);
}

#[test]
fn one_role_per_process() {
    let addr = free_addr();
    let child = start(&addr, Some("worker"));
    let (status, body) = wait_for(&addr, "/readyz");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(body.contains(r#""roles":["worker"]"#), "{body}");
    stop(child);
}

#[test]
fn invalid_configuration_stops_the_start() {
    let status = Command::new(env!("CARGO_BIN_EXE_cihub-server"))
        .env_clear()
        .env("CIHUB_ROLES", "cron")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
}
