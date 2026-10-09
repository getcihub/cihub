//! `cihub server bootstrap --installation <id>`: starts an organization from an
//! installation of the GitHub App, shows its API token once, and stops
//! (`DECISIONS.md` D154, D155).

use anyhow::Context;
use cihub_domain::ConnectionKind;
use cihub_platform::Bootstrapped;
use cihub_store::PgPool;
use secrecy::ExposeSecret;
use tracing::info;

use super::{Github, now};

pub(super) async fn run(pool: &PgPool, github: &Github, installation: &str) -> anyhow::Result<()> {
    let Bootstrapped {
        org_id,
        connection_id,
        created,
        token,
    } = cihub_platform::bootstrap(
        pool,
        github.app.as_ref(),
        ConnectionKind::GithubApp,
        installation,
        now(),
    )
    .await
    .with_context(|| format!("the installation {installation} cannot start an organization"))?;
    info!(%org_id, %connection_id, created, "organization bootstrapped");
    #[expect(
        clippy::print_stdout,
        reason = "the one place that shows the secret of the token, once"
    )]
    {
        println!("organization: {org_id}");
        println!("API token (shown once, keep it in a secret store):");
        println!("{}", token.expose_secret());
    }
    Ok(())
}
