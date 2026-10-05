use std::net::SocketAddr;

use anyhow::{Context, bail};
use cihub_github::GithubConfig;
use cihub_telemetry::{LogFormat, Logs};
use secrecy::SecretString;

/// The configuration of the server, read from the `CIHUB_*` variables.
#[derive(Debug)]
pub struct Config {
    /// Address of `/healthz` and `/readyz`, separate from the public routes.
    pub internal_addr: SocketAddr,
    /// Public address of the `web` role: webhooks, then API (`DECISIONS.md` D82).
    pub http_addr: SocketAddr,
    /// `postgres://user:password@host/database`.
    pub database_url: SecretString,
    pub logs: Logs,
    /// `None` when its variables are missing: `migrate` does not need it, the
    /// server stops without it (`DECISIONS.md` D92).
    pub github: Option<GithubApp>,
}

/// The GitHub App of the instance, created by hand (`DECISIONS.md` D79, D92).
#[derive(Debug)]
pub struct GithubApp {
    /// How to reach the App; its URL has no final `/`.
    pub config: GithubConfig,
    pub webhook_secret: SecretString,
}

const DEFAULT_INTERNAL_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_HTTP_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_GITHUB_URL: &str = "https://github.com";

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let internal_addr =
            get("CIHUB_INTERNAL_ADDR").unwrap_or_else(|| DEFAULT_INTERNAL_ADDR.to_owned());
        let internal_addr = internal_addr
            .parse()
            .with_context(|| format!("CIHUB_INTERNAL_ADDR `{internal_addr}` is not an address"))?;
        let http_addr = get("CIHUB_HTTP_ADDR").unwrap_or_else(|| DEFAULT_HTTP_ADDR.to_owned());
        let http_addr = http_addr
            .parse()
            .with_context(|| format!("CIHUB_HTTP_ADDR `{http_addr}` is not an address"))?;
        let Some(database_url) = get("CIHUB_DATABASE_URL").filter(|url| !url.is_empty()) else {
            bail!("CIHUB_DATABASE_URL is required");
        };
        let log_format = match get("CIHUB_LOG_FORMAT").as_deref() {
            None | Some("json") => LogFormat::Json,
            Some("pretty") => LogFormat::Pretty,
            Some(other) => bail!("CIHUB_LOG_FORMAT `{other}` must be `json` or `pretty`"),
        };
        let log = get("CIHUB_LOG").unwrap_or_else(|| "info".to_owned());
        let logs = Logs::new(&log, log_format)
            .with_context(|| format!("CIHUB_LOG `{log}` is not a valid filter"))?;
        Ok(Self {
            internal_addr,
            http_addr,
            database_url: database_url.into(),
            logs,
            github: github_app(&get)?,
        })
    }
}

/// The GitHub App when `CIHUB_GITHUB_APP_ID` is set; its key comes from
/// `CIHUB_GITHUB_PRIVATE_KEY` or from the file `CIHUB_GITHUB_PRIVATE_KEY_FILE`.
fn github_app(get: &impl Fn(&str) -> Option<String>) -> anyhow::Result<Option<GithubApp>> {
    let Some(app_id) = get("CIHUB_GITHUB_APP_ID").filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let private_key = match (
        get("CIHUB_GITHUB_PRIVATE_KEY"),
        get("CIHUB_GITHUB_PRIVATE_KEY_FILE"),
    ) {
        (Some(key), None) => key,
        (None, Some(path)) => std::fs::read_to_string(&path)
            .with_context(|| format!("CIHUB_GITHUB_PRIVATE_KEY_FILE `{path}` cannot be read"))?,
        _ => bail!(
            "CIHUB_GITHUB_APP_ID needs one of CIHUB_GITHUB_PRIVATE_KEY and CIHUB_GITHUB_PRIVATE_KEY_FILE"
        ),
    };
    let Some(webhook_secret) = get("CIHUB_GITHUB_WEBHOOK_SECRET").filter(|s| !s.is_empty()) else {
        bail!("CIHUB_GITHUB_APP_ID needs CIHUB_GITHUB_WEBHOOK_SECRET");
    };
    let url = get("CIHUB_GITHUB_URL").unwrap_or_else(|| DEFAULT_GITHUB_URL.to_owned());
    Ok(Some(GithubApp {
        config: GithubConfig {
            url: url.trim_end_matches('/').to_owned(),
            app_id,
            private_key: private_key.into(),
        },
        webhook_secret: webhook_secret.into(),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
        let mut vars: HashMap<_, _> = vars.iter().copied().collect();
        vars.entry("CIHUB_DATABASE_URL")
            .or_insert("postgres://cihub:cihub@127.0.0.1/cihub");
        Config::from_lookup(|name| vars.get(name).map(|v| (*v).to_owned()))
    }

    #[test]
    fn defaults() {
        let config = config(&[]).unwrap();
        assert_eq!(config.internal_addr, DEFAULT_INTERNAL_ADDR.parse().unwrap());
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR.parse().unwrap());
        assert_eq!(config.logs.format(), LogFormat::Json);
    }

    #[test]
    fn invalid_values_stop_the_start() {
        for vars in [
            [("CIHUB_INTERNAL_ADDR", "localhost")],
            [("CIHUB_HTTP_ADDR", "localhost")],
            [("CIHUB_DATABASE_URL", "")],
            [("CIHUB_LOG", "info,=[")],
            [("CIHUB_LOG_FORMAT", "text")],
        ] {
            let err = config(&vars).unwrap_err();
            assert!(err.to_string().contains(vars[0].0), "{err}");
        }
    }

    #[test]
    fn database_url_is_required() {
        let err = Config::from_lookup(|_| None).unwrap_err();
        assert_eq!(err.to_string(), "CIHUB_DATABASE_URL is required");
    }

    #[test]
    fn the_github_app_is_optional() {
        assert!(config(&[]).unwrap().github.is_none());
        let app = config(&[
            ("CIHUB_GITHUB_APP_ID", "123"),
            ("CIHUB_GITHUB_PRIVATE_KEY", "key"),
            ("CIHUB_GITHUB_WEBHOOK_SECRET", "secret"),
        ])
        .unwrap()
        .github
        .unwrap();
        assert_eq!(app.config.url, DEFAULT_GITHUB_URL);
        assert_eq!(app.config.app_id, "123");
    }

    #[test]
    fn the_github_app_needs_one_key_and_a_webhook_secret() {
        for vars in [
            &[
                ("CIHUB_GITHUB_APP_ID", "123"),
                ("CIHUB_GITHUB_WEBHOOK_SECRET", "s"),
            ][..],
            &[
                ("CIHUB_GITHUB_APP_ID", "123"),
                ("CIHUB_GITHUB_PRIVATE_KEY", "key"),
                ("CIHUB_GITHUB_PRIVATE_KEY_FILE", "/key.pem"),
                ("CIHUB_GITHUB_WEBHOOK_SECRET", "s"),
            ],
            &[
                ("CIHUB_GITHUB_APP_ID", "123"),
                ("CIHUB_GITHUB_PRIVATE_KEY", "key"),
            ],
        ] {
            let err = config(vars).unwrap_err();
            assert!(
                err.to_string().starts_with("CIHUB_GITHUB_APP_ID needs"),
                "{err}"
            );
        }
    }

    #[test]
    fn pretty_format() {
        let config = config(&[("CIHUB_LOG_FORMAT", "pretty")]).unwrap();
        assert_eq!(config.logs.format(), LogFormat::Pretty);
    }
}
