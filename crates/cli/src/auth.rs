//! `cihub auth`: the session of the command line on each host. `login`
//! signs in with a device code, which the person approves in the dashboard
//! (`specs/API.md`); `logout` ends the session; `status` checks each token;
//! `token` prints one, for a script.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use anyhow::{Context, bail};
use cihub_api_types::ErrorCode;
use cihub_client::{Client, ClientError};
use cihub_domain::is_api_token;
use clap::{Args, Subcommand};
use secrecy::{ExposeSecret, SecretString};

use crate::credentials::{
    Credentials, HOST_VARIABLE, HostSession, TOKEN_VARIABLE, TokenSource, address,
};

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub(crate) struct AuthArgs {
    #[command(subcommand)]
    command: AuthCommand,
}

#[derive(Args)]
struct Host {
    /// The host of the instance, such as `cihub.example.com`; `CIHUB_HOST`,
    /// else the only host that the command line knows, by default.
    #[arg(long)]
    hostname: Option<String>,
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Signs the command line in: the person approves a code in the
    /// dashboard.
    Login(Host),
    /// Ends the session of the host, then forgets it.
    Logout(Host),
    /// The hosts, who is signed in on each, and where the token comes from.
    Status {
        #[command(flatten)]
        host: Host,
        /// Shows each token.
        #[arg(long)]
        show_token: bool,
    },
    /// Prints the token of the host.
    Token(Host),
}

/// A command line asks again 5 seconds later after each `polling_too_fast`
/// (RFC 8628, section 3.5).
const SLOW_DOWN: Duration = Duration::from_secs(5);

pub(crate) async fn run(out: &mut impl Write, args: AuthArgs) -> anyhow::Result<()> {
    match args.command {
        AuthCommand::Login(host) => login(out, host.hostname.as_deref()).await,
        AuthCommand::Logout(host) => logout(out, host.hostname.as_deref()).await,
        AuthCommand::Status { host, show_token } => {
            status(out, host.hostname.as_deref(), show_token).await
        }
        AuthCommand::Token(host) => token(out, host.hostname.as_deref()),
    }
}

async fn login(out: &mut impl Write, hostname: Option<&str>) -> anyhow::Result<()> {
    let mut credentials = Credentials::load()?;
    let host = credentials.host(hostname)?;
    let client = Client::anonymous(&address(&host))?;

    let code = client.start_device_sign_in().await?;
    let address = format!("{}?code={}", code.verification_url, code.user_code);
    writeln!(
        out,
        "To sign in, open this address (it expires in {minutes} minutes):\n\n  {link}\n\n\
         Approve the code {user_code} there only if the page shows this code.",
        minutes = code.expires_in / 60,
        link = hyperlink(&address, std::io::stdout().is_terminal()),
        user_code = code.user_code,
    )?;
    let mut interval = Duration::from_secs(code.interval);
    let session = loop {
        tokio::time::sleep(interval).await;
        match client.device_session(&code.device_code).await {
            Ok(session) => break session,
            Err(ClientError::Api(problem)) => match problem.code {
                ErrorCode::DeviceCodePending => {}
                ErrorCode::PollingTooFast => interval += SLOW_DOWN,
                ErrorCode::DeviceCodeDenied => bail!("the code was denied: no session"),
                ErrorCode::DeviceCodeExpired => {
                    bail!("the code expired: run `cihub auth login` again")
                }
                _ => return Err(ClientError::Api(problem).into()),
            },
            Err(err) => return Err(err.into()),
        }
    };

    let login = session.login.clone();
    credentials.keep(
        &host,
        HostSession {
            token: session.token,
            login: session.login,
        },
    );
    let path = credentials.save()?;

    writeln!(
        out,
        "Signed in to {host} as {login}; the session is in {}.",
        path.display()
    )?;
    Ok(())
}

async fn logout(out: &mut impl Write, hostname: Option<&str>) -> anyhow::Result<()> {
    let mut credentials = Credentials::load()?;
    let host = credentials.host(hostname)?;
    let Some(session) = credentials.forget(&host) else {
        bail!("not signed in to {host}");
    };

    let client = Client::new(&address(&host), SecretString::from(session.token))?;
    match client.sign_out().await {
        Ok(()) => {}
        // The session already ended: there is nothing more to end.
        Err(ClientError::Api(problem)) if problem.code == ErrorCode::Unauthorized => {}
        Err(err) => return Err(err).context("the session did not end: it stays in the file"),
    }
    credentials.save()?;

    writeln!(out, "Signed out of {host} as {}.", session.login)?;
    Ok(())
}

async fn status(
    out: &mut impl Write,
    hostname: Option<&str>,
    show_token: bool,
) -> anyhow::Result<()> {
    let credentials = Credentials::load()?;
    let hosts: Vec<String> = if hostname.is_some() || std::env::var_os(HOST_VARIABLE).is_some() {
        vec![credentials.host(hostname)?]
    } else {
        credentials.hosts().map(str::to_owned).collect()
    };
    if hosts.is_empty() {
        bail!("not signed in to any host: run `cihub auth login --hostname <host>`");
    }

    let mut is_every_token_valid = true;
    for host in &hosts {
        let Some((token, source)) = credentials.token(host)? else {
            writeln!(out, "{host}: not signed in")?;
            is_every_token_valid = false;
            continue;
        };
        let from = match &source {
            TokenSource::Variable => TOKEN_VARIABLE.to_owned(),
            TokenSource::File(path) => path.display().to_string(),
        };
        let state = match who(host, &token).await {
            Ok(who) => who,
            Err(err) => {
                is_every_token_valid = false;
                format!("the token does not work: {err:#}")
            }
        };

        writeln!(out, "{host}: {state} (token from {from})")?;
        if show_token {
            writeln!(out, "  token: {}", token.expose_secret())?;
        }
    }

    if !is_every_token_valid {
        bail!("a token does not work");
    }

    Ok(())
}

/// Who the token of `host` stands for: a person, or an organization for an
/// API token, which stands for no person.
async fn who(host: &str, token: &SecretString) -> anyhow::Result<String> {
    let client = Client::new(&address(host), token.clone())?;

    if is_api_token(token.expose_secret()) {
        client.instance().await?;
        return Ok("an API token of an organization".to_owned());
    }

    let user = client.user().await?;
    Ok(format!("signed in as {}", user.login))
}

fn token(out: &mut impl Write, hostname: Option<&str>) -> anyhow::Result<()> {
    let credentials = Credentials::load()?;
    let host = credentials.host(hostname)?;
    let Some((token, _)) = credentials.token(&host)? else {
        bail!("not signed in to {host}: run `cihub auth login --hostname {host}`");
    };

    writeln!(out, "{}", token.expose_secret())?;
    Ok(())
}

/// `address` as a link that a terminal opens on a click (OSC 8); as it is
/// when the output is not a terminal, so that a script reads the address.
fn hyperlink(address: &str, is_terminal: bool) -> String {
    if is_terminal {
        format!("\x1b]8;;{address}\x1b\\{address}\x1b]8;;\x1b\\")
    } else {
        address.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_gets_a_link_and_a_script_the_address() {
        let address = "https://cihub.example.com/device?code=WDJB-MJHT";

        assert_eq!(hyperlink(address, false), address);
        assert_eq!(
            hyperlink(address, true),
            format!("\x1b]8;;{address}\x1b\\{address}\x1b]8;;\x1b\\")
        );
    }
}
