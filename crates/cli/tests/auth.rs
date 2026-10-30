//! `cihub auth` against fake hosts: `CIHUB_TOKEN` never reaches a host that
//! the command did not name, and the file of the sessions stays readable by
//! its owner only.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

/// A host that answers one request with `status`, and gives the head of that
/// request.
fn fake_host(status: &'static str) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let host = listener.local_addr().unwrap().to_string();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = vec![0u8; 16 * 1024];
        let read = stream.read(&mut buffer).unwrap();
        let response =
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        stream.write_all(response.as_bytes()).unwrap();
        sender
            .send(String::from_utf8_lossy(&buffer[..read]).into_owned())
            .unwrap();
    });
    (host, receiver)
}

/// A configuration directory whose file keeps a session `chc_<n>` for each
/// host, in this order.
fn config_directory(name: &str, hosts: &[&str]) -> PathBuf {
    let config = std::env::temp_dir().join(format!("cihub-auth-{name}-{}", std::process::id()));
    std::fs::create_dir_all(config.join("cihub")).unwrap();
    let sessions: serde_json::Map<_, _> = hosts
        .iter()
        .enumerate()
        .map(|(index, host)| {
            let session = serde_json::json!({ "token": format!("chc_{index}"), "login": "ada" });
            ((*host).to_owned(), session)
        })
        .collect();
    let file = serde_json::json!({ "hosts": sessions });
    std::fs::write(credentials_file(&config), file.to_string()).unwrap();
    config
}

fn credentials_file(config: &Path) -> PathBuf {
    config.join("cihub").join("credentials.json")
}

fn cihub(config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cihub"));
    command
        .env_clear()
        .env("XDG_CONFIG_HOME", config)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[test]
fn the_token_of_the_variable_stays_off_hosts_that_the_command_does_not_name() {
    let (first, first_request) = fake_host("401 Unauthorized");
    let (second, second_request) = fake_host("401 Unauthorized");
    let config = config_directory("status", &[&first, &second]);

    cihub(&config)
        .args(["auth", "status"])
        .env("CIHUB_TOKEN", "chc_variable")
        .status()
        .unwrap();
    let first_request = first_request.recv_timeout(Duration::from_secs(10)).unwrap();
    let second_request = second_request
        .recv_timeout(Duration::from_secs(10))
        .unwrap();

    assert!(first_request.contains("Bearer chc_0"), "{first_request}");
    assert!(second_request.contains("Bearer chc_1"), "{second_request}");
    std::fs::remove_dir_all(&config).unwrap();
}

#[test]
fn the_file_of_the_sessions_becomes_readable_by_its_owner_only() {
    let (host, request) = fake_host("204 No Content");
    let config = config_directory("logout", &[&host]);
    let file = credentials_file(&config);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    let status = cihub(&config).args(["auth", "logout"]).status().unwrap();
    let request = request.recv_timeout(Duration::from_secs(10)).unwrap();

    assert!(status.success());
    assert!(request.starts_with("DELETE /"), "{request}");
    assert!(!std::fs::read_to_string(&file).unwrap().contains(&host));
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(std::fs::read_dir(config.join("cihub")).unwrap().count(), 1);
    std::fs::remove_dir_all(&config).unwrap();
}
