//! `cihub login`: keeps the address of the instance and an API token. Every
//! route of the API names its organization, which `cihub login` does not know:
//! the first `cihub queue` command checks the token.

use std::io::Write;

use clap::Args;

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

pub(crate) fn run(out: &mut impl Write, args: LoginArgs) -> anyhow::Result<()> {
    let LoginArgs { url, token } = args;
    let token = if let Some(token) = token {
        token
    } else {
        writeln!(out, "API token:")?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim().to_owned()
    };
    let path = Credentials { url, token }.save()?;

    writeln!(out, "credentials in {}", path.display())?;

    Ok(())
}
