use std::collections::BTreeSet;
use std::net::SocketAddr;

use anyhow::{Context, bail};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

/// A role of the process (`AGENTS.md` D8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// HTTP server: API, webhooks, dashboard.
    Api,
    /// Reconciliation loop of the queues.
    Scheduler,
    /// Execution of the jobs of the modules.
    Worker,
    /// Delivery of the outbox.
    Notifier,
}

impl Role {
    const ALL: [Self; 4] = [Self::Api, Self::Scheduler, Self::Worker, Self::Notifier];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Scheduler => "scheduler",
            Self::Worker => "worker",
            Self::Notifier => "notifier",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

/// The configuration of the server, read from the `CIHUB_*` variables.
#[derive(Debug)]
pub struct Config {
    pub roles: BTreeSet<Role>,
    /// Address of `/healthz` and `/readyz`, separate from the public routes.
    pub internal_addr: SocketAddr,
    pub log_filter: EnvFilter,
    pub log_format: LogFormat,
}

const DEFAULT_INTERNAL_ADDR: &str = "127.0.0.1:8081";

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let roles = match get("CIHUB_ROLES") {
            None => Role::ALL.into_iter().collect(),
            Some(value) => parse_roles(&value)?,
        };
        let internal_addr =
            get("CIHUB_INTERNAL_ADDR").unwrap_or_else(|| DEFAULT_INTERNAL_ADDR.to_owned());
        let internal_addr = internal_addr
            .parse()
            .with_context(|| format!("CIHUB_INTERNAL_ADDR `{internal_addr}` is not an address"))?;
        let log = get("CIHUB_LOG").unwrap_or_else(|| "info".to_owned());
        let log_filter = EnvFilter::try_new(&log)
            .with_context(|| format!("CIHUB_LOG `{log}` is not a valid filter"))?;
        let log_format = match get("CIHUB_LOG_FORMAT").as_deref() {
            None | Some("json") => LogFormat::Json,
            Some("pretty") => LogFormat::Pretty,
            Some(other) => bail!("CIHUB_LOG_FORMAT `{other}` must be `json` or `pretty`"),
        };
        Ok(Self {
            roles,
            internal_addr,
            log_filter,
            log_format,
        })
    }
}

fn parse_roles(value: &str) -> anyhow::Result<BTreeSet<Role>> {
    let mut roles = BTreeSet::new();
    for name in value.split(',').map(str::trim) {
        let Some(role) = Role::parse(name) else {
            bail!(
                "CIHUB_ROLES: unknown role `{name}`, expected api, scheduler, worker or notifier"
            );
        };
        roles.insert(role);
    }
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
        let vars: HashMap<_, _> = vars.iter().copied().collect();
        Config::from_lookup(|name| vars.get(name).map(|v| (*v).to_owned()))
    }

    #[test]
    fn defaults_run_every_role() {
        let config = config(&[]).unwrap();
        assert_eq!(config.roles, Role::ALL.into_iter().collect());
        assert_eq!(config.internal_addr, DEFAULT_INTERNAL_ADDR.parse().unwrap());
        assert_eq!(config.log_format, LogFormat::Json);
    }

    #[test]
    fn roles_select_a_part() {
        let config = config(&[("CIHUB_ROLES", "worker, api,worker")]).unwrap();
        assert_eq!(config.roles, BTreeSet::from([Role::Api, Role::Worker]));
    }

    #[test]
    fn invalid_values_stop_the_start() {
        for vars in [
            [("CIHUB_ROLES", "api,cron")],
            [("CIHUB_ROLES", "")],
            [("CIHUB_INTERNAL_ADDR", "localhost")],
            [("CIHUB_LOG", "info,=[")],
            [("CIHUB_LOG_FORMAT", "text")],
        ] {
            let err = config(&vars).unwrap_err();
            assert!(err.to_string().contains(vars[0].0), "{err}");
        }
    }

    #[test]
    fn pretty_format() {
        let config = config(&[("CIHUB_LOG_FORMAT", "pretty")]).unwrap();
        assert_eq!(config.log_format, LogFormat::Pretty);
    }
}
