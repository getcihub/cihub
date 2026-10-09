//! `cihub server token create --org <slug>`: creates an API token with every
//! scope for an organization, shows it once, and stops (`DECISIONS.md` D248).

use anyhow::Context;
use cihub_store::PgPool;
use secrecy::ExposeSecret;

use super::now;

pub(super) async fn create(pool: &PgPool, slug: &str) -> anyhow::Result<()> {
    let (_, token) = cihub_platform::create_token(pool, slug, now())
        .await
        .with_context(|| format!("no token created for the organization {slug}"))?;

    #[expect(
        clippy::print_stdout,
        reason = "the one place that shows the secret of the token, once"
    )]
    {
        println!("{}", token.expose_secret());
    }
    Ok(())
}
