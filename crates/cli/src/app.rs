//! `cihub app create`: registers the GitHub App of an instance from a
//! manifest (`specs/GITHUB.md`), before its first start. A local page posts the
//! manifest to GitHub, the person creates the App there, GitHub sends them back
//! with a code, and the keys of the App land in two files. It talks to GitHub
//! only: the instance does not run yet.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use cihub_github::{CreatedApp, app_manifest, new_app_url, new_secret, register_app};
use clap::{Args, Subcommand};
use secrecy::{ExposeSecret, SecretString};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The file of the private key, as the Docker installation names it.
const PRIVATE_KEY_FILE: &str = "github-app.private-key.pem";
/// The other keys, as the `CIHUB_GITHUB_*` variables of the server.
const ENV_FILE: &str = "github-app.env";
const GITHUB_COM: &str = "https://github.com";
const CALLBACK_PATH: &str = "/callback";
/// A request of the browser to the local page fits in this.
const MAX_REQUEST: usize = 16 * 1024;

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct AppArgs {
    #[command(subcommand)]
    command: AppCommand,
}

#[derive(Subcommand)]
enum AppCommand {
    /// Creates the GitHub App of an instance: a local page sends you to
    /// GitHub, then the keys of the App land in two files.
    Create(CreateArgs),
}

#[derive(Args)]
struct CreateArgs {
    /// The address where GitHub and the people reach the instance, such as
    /// `https://cihub.example.com`.
    #[arg(long)]
    public_url: String,
    /// The name of the App, unique on GitHub, such as `Acme CIHub`.
    #[arg(long)]
    name: String,
    /// The organization that owns the App; without it, your account.
    #[arg(long)]
    org: Option<String>,
    /// github.com, or the address of a GitHub Enterprise Server.
    #[arg(long, default_value = GITHUB_COM)]
    github_url: String,
    /// The port of the local page, on `localhost`.
    #[arg(long, default_value_t = 3999)]
    port: u16,
    /// The directory of the two files of the keys.
    #[arg(long, default_value = ".")]
    out: PathBuf,
}

/// A request of the browser to the local page.
#[derive(Debug, PartialEq, Eq)]
struct Request {
    path: String,
    query: Vec<(String, String)>,
}

impl Request {
    fn parameter(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) async fn run(out: &mut impl Write, args: AppArgs) -> anyhow::Result<()> {
    match args.command {
        AppCommand::Create(args) => create(out, args).await,
    }
}

async fn create(out: &mut impl Write, args: CreateArgs) -> anyhow::Result<()> {
    if !args.public_url.starts_with("https://") && !args.public_url.starts_with("http://") {
        bail!("--public-url is the address of the instance, such as https://cihub.example.com");
    }
    let key_path = args.out.join(PRIVATE_KEY_FILE);
    let env_path = args.out.join(ENV_FILE);
    for path in [&key_path, &env_path] {
        if path.exists() {
            bail!(
                "{} exists: move it away first, it may hold the keys of another App",
                path.display()
            );
        }
    }

    let listener = TcpListener::bind(("127.0.0.1", args.port))
        .await
        .with_context(|| format!("cannot listen on the port {}", args.port))?;
    let page = format!("http://localhost:{}", args.port);
    let state = new_secret()?;
    let manifest = app_manifest(
        &args.name,
        &args.public_url,
        &format!("{page}{CALLBACK_PATH}"),
    );
    let form_url = new_app_url(&args.github_url, args.org.as_deref(), state.expose_secret());
    let form = form_page(&form_url, &manifest.to_string());
    writeln!(
        out,
        "Open {page} in your browser: it sends you to GitHub, where you create the App {}.",
        args.name
    )?;

    let app = serve(&listener, &form, &state, &args.github_url).await?;
    write_secret(&key_path, app.pem.expose_secret())?;
    write_secret(&env_path, &env_file(&app, &args.github_url))?;
    writeln!(out, "{}", summary(&app, &key_path, &env_path))?;
    Ok(())
}

/// Answers the browser until GitHub sends it back with the code of the App.
async fn serve(
    listener: &TcpListener,
    form: &str,
    state: &SecretString,
    github_url: &str,
) -> anyhow::Result<CreatedApp> {
    loop {
        let (mut stream, _) = listener.accept().await.context("the local page stopped")?;
        let Some(request) = read_request(&mut stream).await else {
            continue;
        };

        if request.path == "/" {
            respond(&mut stream, "200 OK", form).await;
            continue;
        }
        if request.path != CALLBACK_PATH {
            respond(&mut stream, "404 Not Found", "Not found.").await;
            continue;
        }
        let (Some(code), Some(given)) = (request.parameter("code"), request.parameter("state"))
        else {
            respond(&mut stream, "400 Bad Request", "GitHub gave no code.").await;
            continue;
        };
        if given != state.expose_secret() {
            let text = "This link does not come from the page that created the App. \
                        Start again from the terminal.";
            respond(&mut stream, "400 Bad Request", text).await;
            continue;
        }

        let result = register_app(github_url, code, new_secret()?, now()).await;
        let text = if result.is_ok() {
            "The App exists, and its keys are in the files that the terminal names. \
             You can close this page."
        } else {
            "GitHub did not give the keys of the App: the terminal says why."
        };
        respond(&mut stream, "200 OK", text).await;
        return result.context("GitHub did not give the keys of the App");
    }
}

/// The request line of the browser, or `None` for a request that is not one.
async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 || buffer.len() > MAX_REQUEST {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&buffer);
    parse_request_line(head.lines().next()?)
}

fn parse_request_line(line: &str) -> Option<Request> {
    let mut parts = line.split(' ');
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect();
    Some(Request {
        path: path.to_owned(),
        query,
    })
}

/// Decodes `%XX` and `+` of a query string.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (b'+', _) => {
                decoded.push(b' ');
                index += 1;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // The browser may close first: the page only informs, the terminal says
    // the outcome.
    let _ = stream.write_all(response.as_bytes()).await;
}

/// A page that posts the manifest to GitHub at once, with a button when the
/// browser runs no script.
fn form_page(form_url: &str, manifest: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Create the GitHub App of CIHub</title>\
         <form id=\"manifest\" method=\"post\" action=\"{}\">\
         <input type=\"hidden\" name=\"manifest\" value=\"{}\">\
         <button>Create the GitHub App on GitHub</button></form>\
         <script>document.getElementById(\"manifest\").submit()</script>",
        escape(form_url),
        escape(manifest)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The keys of the App as the variables of the server; the Docker
/// installation reads them in its `.env`.
fn env_file(app: &CreatedApp, github_url: &str) -> String {
    let mut lines = Vec::new();
    if github_url.trim_end_matches('/') != GITHUB_COM {
        lines.push(format!("CIHUB_GITHUB_URL={github_url}"));
    }
    lines.push(format!("CIHUB_GITHUB_APP_ID={}", app.id));
    lines.push(format!(
        "CIHUB_GITHUB_WEBHOOK_SECRET={}",
        app.webhook_secret.expose_secret()
    ));
    lines.push(format!("CIHUB_GITHUB_CLIENT_ID={}", app.client_id));
    lines.push(format!(
        "CIHUB_GITHUB_CLIENT_SECRET={}",
        app.client_secret.expose_secret()
    ));
    lines.join("\n") + "\n"
}

/// Writes a new file that only its owner reads.
fn write_secret(path: &Path, content: &str) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))
}

fn summary(app: &CreatedApp, key_path: &Path, env_path: &Path) -> String {
    format!(
        "The GitHub App {} exists: {}\n\
         \n  {}  the App ID, the Client ID, the webhook secret and the client secret\
         \n  {}  the private key\n\
         \nGive them to CIHub (https://cihub.io/docs/install), then install the App on your \
         repositories:\n  {}/installations/new",
        app.name,
        app.html_url,
        env_path.display(),
        key_path.display(),
        app.html_url
    )
}

fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place where the command line reads the clock: the date of the JWT of the new App"
    )]
    OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_callback_gives_its_code_and_its_state() {
        let request = parse_request_line("GET /callback?code=c0de&state=f%30od HTTP/1.1").unwrap();

        assert_eq!(request.path, "/callback");
        assert_eq!(request.parameter("code"), Some("c0de"));
        assert_eq!(request.parameter("state"), Some("f0od"));
        assert_eq!(request.parameter("other"), None);
    }

    #[test]
    fn only_a_get_is_a_request_of_the_page() {
        assert!(parse_request_line("POST / HTTP/1.1").is_none());
        assert!(parse_request_line("").is_none());
        assert_eq!(
            parse_request_line("GET / HTTP/1.1").map(|request| request.path),
            Some("/".to_owned())
        );
    }

    #[test]
    fn decoding_keeps_what_is_not_an_escape() {
        assert_eq!(decode("a+b%2Fc"), "a b/c");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }

    #[test]
    fn the_manifest_is_escaped_in_the_page() {
        let page = form_page("https://github.com/x?state=1&a=2", r#"{"name":"<A & B>"}"#);

        assert!(page.contains("action=\"https://github.com/x?state=1&amp;a=2\""));
        assert!(page.contains("value=\"{&quot;name&quot;:&quot;&lt;A &amp; B&gt;&quot;}\""));
    }
}
