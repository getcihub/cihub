//! `cihub login`: keeps the address of the instance and an API token
//! (`DECISIONS.md` D161).

use std::io::Write;

use anyhow::Context;
use cihub_client::Client;
use clap::Args;
use secrecy::SecretString;

use crate::credentials::Credentials;

#[derive(Args)]
pub(crate) struct LoginArgs {
    /// The address of the instance, such as `https://cihub.example.com`.
    #[arg(long)]
    url: String,
    /// The API token; read from the standard input without it.
    #[arg(long)]
    token: Option<String>,
}

pub(crate) async fn run(out: &mut impl Write, args: LoginArgs) -> anyhow::Result<()> {
    let LoginArgs { url, token } = args;
    let token = if let Some(token) = token {
        token
    } else {
        writeln!(out, "API token:")?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim().to_owned()
    };
    let client = Client::new(&url, SecretString::from(token.clone()))?;
    let repos = client
        .repos()
        .await
        .context("the token does not open the API")?;
    let path = Credentials { url, token }.save()?;

    writeln!(
        out,
        "logged in: {} repositories, credentials in {}",
        repos.repos.len(),
        path.display()
    )?;

    Ok(())
}
