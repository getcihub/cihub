//! `cihub app create` against a fake GitHub: the local page posts the
//! manifest, a callback with another state is refused, the code gives the keys,
//! the App takes a webhook secret of CIHub, and the keys land in two files that
//! only their owner reads.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The key that the fake GitHub gives: the App signs the change of its
/// webhook secret with it.
const TEST_KEY: &str = include_str!("../../github/tests/test-only-key.pem");

/// Answers the exchange of the code `c0de` and the change of the webhook
/// secret; the GitHub of the tests reads no body.
async fn fake_github() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 64 * 1024];
            let read = stream.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let (status, body) =
                if request.starts_with("POST /api/v3/app-manifests/c0de/conversions") {
                    let app = serde_json::json!({
                        "id": 42,
                        "name": "Acme CIHub",
                        "html_url": "https://github.com/apps/acme-cihub",
                        "client_id": "Iv23li",
                        "client_secret": "client-secret",
                        "webhook_secret": "the-secret-of-github",
                        "pem": TEST_KEY,
                    });
                    ("201 Created", app.to_string())
                } else if request.starts_with("PATCH /api/v3/app/hook/config")
                    && request.contains("Bearer ey")
                {
                    ("200 OK", "{}".to_owned())
                } else {
                    ("404 Not Found", "{}".to_owned())
                };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    url
}

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The status and the body of a GET, or `None` while nothing listens.
async fn get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    let status = answer.split(' ').nth(1)?.parse().ok()?;
    Some((status, answer))
}

fn out_directory(name: &str) -> PathBuf {
    let out = std::env::temp_dir().join(format!("cihub-app-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    out
}

#[tokio::test]
async fn the_code_of_github_gives_two_files_of_keys() {
    let github = fake_github().await;
    let port = free_port().await;
    let out = out_directory("created");
    let mut command = Command::new(env!("CARGO_BIN_EXE_cihub"))
        .args(["app", "create", "--public-url", "https://cihub.example.com"])
        .args([
            "--name",
            "Acme CIHub",
            "--org",
            "acme",
            "--github-url",
            &github,
        ])
        .args(["--port", &port.to_string()])
        .arg("--out")
        .arg(&out)
        .env_clear()
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    let mut page = None;
    for _ in 0..100 {
        page = get(port, "/").await;
        if page.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (_, page) = page.unwrap();
    let state = page
        .split("state=")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let (refused, _) = get(port, "/callback?code=c0de&state=other").await.unwrap();
    let (accepted, _) = get(port, &format!("/callback?code=c0de&state={state}"))
        .await
        .unwrap();
    let status = command.wait().unwrap();

    assert!(page.contains(&format!("{github}/organizations/acme/settings/apps/new")));
    assert!(page.contains("https://cihub.example.com/webhooks/github"));
    assert_eq!(refused, 400);
    assert_eq!(accepted, 200);
    assert!(status.success());
    let env = std::fs::read_to_string(out.join("github-app.env")).unwrap();
    let secret = env
        .lines()
        .find_map(|line| line.strip_prefix("CIHUB_GITHUB_WEBHOOK_SECRET="))
        .unwrap();
    assert_eq!(secret.len(), 64, "a secret of CIHub, not the one of GitHub");
    assert_eq!(
        env,
        format!(
            "CIHUB_GITHUB_URL={github}\nCIHUB_GITHUB_APP_ID=42\nCIHUB_GITHUB_WEBHOOK_SECRET={secret}\n\
             CIHUB_GITHUB_CLIENT_ID=Iv23li\nCIHUB_GITHUB_CLIENT_SECRET=client-secret\n"
        )
    );
    let key = out.join("github-app.private-key.pem");
    assert_eq!(std::fs::read_to_string(&key).unwrap(), TEST_KEY);
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir_all(&out).unwrap();
}

#[test]
fn existing_keys_are_never_overwritten() {
    let out = out_directory("kept");
    std::fs::write(out.join("github-app.env"), "kept").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cihub"))
        .args([
            "app",
            "create",
            "--public-url",
            "https://cihub.example.com",
            "--name",
            "x",
        ])
        .arg("--out")
        .arg(&out)
        .env_clear()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert!(!status.success());
    assert_eq!(
        std::fs::read_to_string(out.join("github-app.env")).unwrap(),
        "kept"
    );
    std::fs::remove_dir_all(&out).unwrap();
}
