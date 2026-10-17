//! The configuration of `cihub-server`: the YAML
//! file named by `--config`, under the `CIHUB_*` variables. The variable of a
//! key is `CIHUB_` followed by its path in capitals, with `_` as separator;
//! a variable wins over the file, the file over the default.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, bail};
use cihub_github::{GithubConfig, SignInClient};
use cihub_store::PoolSettings;
use cihub_telemetry::{LogFormat, Logs};
use secrecy::SecretString;
use serde_saphyr::{DefaultMessageFormatter, MessageFormatter};

const KEYS: [&str; 17] = [
    "database.url",
    "database.pool_size",
    "database.idle_timeout",
    "database.connection_timeout",
    "database.secret",
    "http.addr",
    "http.public_url",
    "internal.addr",
    "log.filter",
    "log.format",
    "github.url",
    "github.app_id",
    "github.private_key",
    "github.private_key_file",
    "github.webhook_secret",
    "github.client_id",
    "github.client_secret",
];

#[derive(Debug)]
pub struct Config {
    pub internal_addr: SocketAddr,
    pub http_addr: SocketAddr,
    /// The address where people reach the instance, such as
    /// `https://ci.example.com`; without it, nobody signs in to the dashboard.
    pub public_url: Option<String>,
    pub database: Database,
    pub logs: Logs,
    pub github: Option<GithubApp>,
}

#[derive(Debug)]
pub struct Database {
    pub url: SecretString,
    pub pool: PoolSettings,
    /// The key of the data that CIHub encrypts in Postgres.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the encryption of the data comes in its own change"
        )
    )]
    pub secret: Option<SecretString>,
}

#[derive(Debug)]
pub struct GithubApp {
    pub config: GithubConfig,
    pub webhook_secret: SecretString,
}

const DEFAULT_INTERNAL_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_HTTP_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_GITHUB_URL: &str = "https://github.com";
// The defaults of sqlx.
const DEFAULT_POOL_SIZE: u32 = 10;
const DEFAULT_IDLE_TIMEOUT_SECONDS: u32 = 600;
const DEFAULT_CONNECTION_TIMEOUT_SECONDS: u32 = 30;

impl Config {
    pub fn load(file: Option<&Path>) -> anyhow::Result<Self> {
        let file = match file {
            None => None,
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("{} cannot be read", path.display()))?;
                Some((path.display().to_string(), text))
            }
        };
        let file = file
            .as_ref()
            .map(|(path, text)| (path.as_str(), text.as_str()));
        Self::from_sources(file, |name| std::env::var(name).ok())
    }

    fn from_sources(
        file: Option<(&str, &str)>,
        get: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let sources = Sources {
            file: file.map(|(path, text)| read_file(path, text)).transpose()?,
            get,
        };
        let internal_addr = sources.addr("internal.addr", DEFAULT_INTERNAL_ADDR)?;
        let http_addr = sources.addr("http.addr", DEFAULT_HTTP_ADDR)?;
        let public_url = sources
            .non_empty("http.public_url")
            .map(|found| public_url(&found))
            .transpose()?;
        let database = database(&sources)?;

        let log_format = match sources.get("log.format") {
            None => LogFormat::Json,
            Some(format) => match format.value.as_str() {
                "json" => LogFormat::Json,
                "pretty" => LogFormat::Pretty,
                other => bail!("{} `{other}` must be `json` or `pretty`", format.origin),
            },
        };
        let logs = match sources.get("log.filter") {
            None => Logs::new("info", log_format)?,
            Some(filter) => Logs::new(&filter.value, log_format).with_context(|| {
                format!("{} `{}` is not a valid filter", filter.origin, filter.value)
            })?,
        };

        Ok(Self {
            internal_addr,
            http_addr,
            public_url,
            database,
            logs,
            github: github_app(&sources)?,
        })
    }
}

fn database(sources: &Sources<impl Fn(&str) -> Option<String>>) -> anyhow::Result<Database> {
    let Some(url) = sources.non_empty("database.url") else {
        bail!("{} is required", names("database.url"));
    };
    let seconds = |key, default| {
        sources
            .positive(key, default)
            .map(|seconds| Duration::from_secs(seconds.into()))
    };

    Ok(Database {
        url: url.value.into(),
        pool: PoolSettings {
            size: sources.positive("database.pool_size", DEFAULT_POOL_SIZE)?,
            idle_timeout: seconds("database.idle_timeout", DEFAULT_IDLE_TIMEOUT_SECONDS)?,
            connection_timeout: seconds(
                "database.connection_timeout",
                DEFAULT_CONNECTION_TIMEOUT_SECONDS,
            )?,
        },
        secret: sources
            .non_empty("database.secret")
            .map(|secret| secret.value.into()),
    })
}

/// The root address of the instance, without its last `/`: the sign-in
/// builds its addresses from it.
fn public_url(found: &Found) -> anyhow::Result<String> {
    let url = found.value.trim_end_matches('/');
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match host {
        Some(host) if !host.is_empty() && !host.contains('/') => Ok(url.to_owned()),
        _ => bail!(
            "{} `{}` must be the root address of the instance, such as https://ci.example.com",
            found.origin,
            found.value
        ),
    }
}

type FileValues = BTreeMap<String, String>;

fn read_file(path: &str, text: &str) -> anyhow::Result<(String, FileValues)> {
    let sections: Option<BTreeMap<String, BTreeMap<String, String>>> = serde_saphyr::from_str(text)
        .map_err(|err| {
            let err = err.without_snippet();
            // The snippet could show a secret of the file.
            let at = err
                .location()
                .map(|l| format!(", line {} column {}", l.line(), l.column()))
                .unwrap_or_default();
            anyhow::anyhow!(
                "{path} is not valid{at}: {}",
                DefaultMessageFormatter.format_message(err)
            )
        })?;
    let mut values = FileValues::new();
    for (section, keys) in sections.unwrap_or_default() {
        for (key, value) in keys {
            let key = format!("{section}.{key}");
            if !KEYS.contains(&key.as_str()) {
                bail!(
                    "{path}: unknown key `{key}`, expected one of {}",
                    KEYS.join(", ")
                );
            }
            values.insert(key, value);
        }
    }
    Ok((path.to_owned(), values))
}

struct Sources<G> {
    file: Option<(String, FileValues)>,
    get: G,
}

struct Found {
    value: String,
    origin: String,
}

impl<G: Fn(&str) -> Option<String>> Sources<G> {
    fn get(&self, key: &str) -> Option<Found> {
        let variable = variable(key);
        if let Some(value) = (self.get)(&variable) {
            return Some(Found {
                value,
                origin: variable,
            });
        }
        let (path, values) = self.file.as_ref()?;
        values.get(key).map(|value| Found {
            value: value.clone(),
            origin: format!("`{key}` in {path}"),
        })
    }

    fn non_empty(&self, key: &str) -> Option<Found> {
        self.get(key).filter(|found| !found.value.is_empty())
    }

    fn positive(&self, key: &str, default: u32) -> anyhow::Result<u32> {
        let Some(found) = self.get(key) else {
            return Ok(default);
        };
        match found.value.parse() {
            Ok(value) if value > 0 => Ok(value),
            _ => bail!(
                "{} `{}` must be a positive integer",
                found.origin,
                found.value
            ),
        }
    }

    fn addr(&self, key: &str, default: &str) -> anyhow::Result<SocketAddr> {
        let Some(addr) = self.get(key) else {
            return Ok(default.parse()?);
        };
        addr.value
            .parse()
            .with_context(|| format!("{} `{}` is not an address", addr.origin, addr.value))
    }
}

fn names(key: &str) -> String {
    format!("{} or `{key}`", variable(key))
}

fn variable(key: &str) -> String {
    format!("CIHUB_{}", key.replace('.', "_").to_ascii_uppercase())
}

fn github_app(
    sources: &Sources<impl Fn(&str) -> Option<String>>,
) -> anyhow::Result<Option<GithubApp>> {
    let Some(app_id) = sources.non_empty("github.app_id") else {
        return Ok(None);
    };

    let app = names("github.app_id");
    let private_key = match (
        sources.get("github.private_key"),
        sources.get("github.private_key_file"),
    ) {
        (Some(key), None) => key.value,
        (None, Some(path)) => std::fs::read_to_string(&path.value)
            .with_context(|| format!("{} `{}` cannot be read", path.origin, path.value))?,
        _ => bail!(
            "{app} needs one of {} and {}",
            names("github.private_key"),
            names("github.private_key_file")
        ),
    };
    let Some(webhook_secret) = sources.non_empty("github.webhook_secret") else {
        bail!("{app} needs {}", names("github.webhook_secret"));
    };

    let url = sources
        .get("github.url")
        .map_or_else(|| DEFAULT_GITHUB_URL.to_owned(), |url| url.value);

    Ok(Some(GithubApp {
        config: GithubConfig {
            url: url.trim_end_matches('/').to_owned(),
            app_id: app_id.value,
            private_key: private_key.into(),
            sign_in: sign_in_client(sources)?,
        },
        webhook_secret: webhook_secret.value.into(),
    }))
}

/// The client of the App for the sign-in to the dashboard: both keys, or
/// none, and then nobody signs in.
fn sign_in_client(
    sources: &Sources<impl Fn(&str) -> Option<String>>,
) -> anyhow::Result<Option<SignInClient>> {
    match (
        sources.non_empty("github.client_id"),
        sources.non_empty("github.client_secret"),
    ) {
        (Some(client_id), Some(client_secret)) => Ok(Some(SignInClient {
            client_id: client_id.value,
            client_secret: client_secret.value.into(),
        })),
        (None, None) => Ok(None),
        _ => bail!(
            "the sign-in needs both {} and {}",
            names("github.client_id"),
            names("github.client_secret")
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::ExposeSecret;

    use super::*;

    const PATH: &str = "/etc/cihub/server.yaml";
    const DATABASE: (&str, &str) = ("CIHUB_DATABASE_URL", "postgres://cihub@127.0.0.1/cihub");

    fn load(file: Option<&str>, vars: &[(&str, &str)]) -> anyhow::Result<Config> {
        let vars: HashMap<_, _> = vars.iter().copied().collect();
        Config::from_sources(file.map(|text| (PATH, text)), |name| {
            vars.get(name).map(|v| (*v).to_owned())
        })
    }

    #[test]
    fn every_key_has_its_variable() {
        let variables: Vec<String> = KEYS.iter().map(|key| variable(key)).collect();
        assert_eq!(
            variables,
            [
                "CIHUB_DATABASE_URL",
                "CIHUB_DATABASE_POOL_SIZE",
                "CIHUB_DATABASE_IDLE_TIMEOUT",
                "CIHUB_DATABASE_CONNECTION_TIMEOUT",
                "CIHUB_DATABASE_SECRET",
                "CIHUB_HTTP_ADDR",
                "CIHUB_HTTP_PUBLIC_URL",
                "CIHUB_INTERNAL_ADDR",
                "CIHUB_LOG_FILTER",
                "CIHUB_LOG_FORMAT",
                "CIHUB_GITHUB_URL",
                "CIHUB_GITHUB_APP_ID",
                "CIHUB_GITHUB_PRIVATE_KEY",
                "CIHUB_GITHUB_PRIVATE_KEY_FILE",
                "CIHUB_GITHUB_WEBHOOK_SECRET",
                "CIHUB_GITHUB_CLIENT_ID",
                "CIHUB_GITHUB_CLIENT_SECRET",
            ]
        );
    }

    #[test]
    fn variables_only_take_the_defaults() {
        let config = load(None, &[DATABASE]).unwrap();
        assert_eq!(config.internal_addr, DEFAULT_INTERNAL_ADDR.parse().unwrap());
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR.parse().unwrap());
        assert_eq!(config.logs.format(), LogFormat::Json);
        assert_eq!(
            config.database.pool,
            PoolSettings {
                size: 10,
                idle_timeout: Duration::from_secs(600),
                connection_timeout: Duration::from_secs(30),
            }
        );
        assert!(config.database.secret.is_none());
        assert!(config.github.is_none());
    }

    #[test]
    fn a_file_only_gives_every_key() {
        let file = "
database:
  url: postgres://cihub@db/cihub
  pool_size: 25
  idle_timeout: 120
  connection_timeout: 5
  secret: data-key
http:
  addr: 0.0.0.0:9090
internal:
  addr: 127.0.0.1:9091
log:
  filter: info,cihub_github=debug
  format: pretty
github:
  url: https://github.example.com/
  app_id: 123456
  private_key: key
  webhook_secret: secret
";
        let config = load(Some(file), &[]).unwrap();

        assert_eq!(
            config.database.url.expose_secret(),
            "postgres://cihub@db/cihub"
        );
        assert_eq!(
            config.database.pool,
            PoolSettings {
                size: 25,
                idle_timeout: Duration::from_secs(120),
                connection_timeout: Duration::from_secs(5),
            }
        );
        assert_eq!(config.database.secret.unwrap().expose_secret(), "data-key");
        assert_eq!(config.http_addr, "0.0.0.0:9090".parse().unwrap());
        assert_eq!(config.internal_addr, "127.0.0.1:9091".parse().unwrap());
        assert_eq!(config.logs.format(), LogFormat::Pretty);
        let app = config.github.unwrap();
        assert_eq!(app.config.url, "https://github.example.com");
        assert_eq!(app.config.app_id, "123456");
        assert_eq!(app.config.private_key.expose_secret(), "key");
        assert_eq!(app.webhook_secret.expose_secret(), "secret");
    }

    #[test]
    fn a_variable_wins_over_the_file() {
        let file = "database:\n  url: postgres://file/cihub\nhttp:\n  addr: 0.0.0.0:9090\n";
        let config = load(
            Some(file),
            &[("CIHUB_DATABASE_URL", "postgres://var/cihub")],
        )
        .unwrap();
        assert_eq!(config.database.url.expose_secret(), "postgres://var/cihub");
        assert_eq!(config.http_addr, "0.0.0.0:9090".parse().unwrap());
    }

    #[test]
    fn an_empty_file_gives_nothing() {
        let config = load(Some(""), &[DATABASE]).unwrap();
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR.parse().unwrap());
    }

    #[test]
    fn the_file_refuses_an_unknown_key_or_a_wrong_type() {
        for (file, expected) in [
            ("http:\n  adr: 0.0.0.0:80\n", "unknown key `http.adr`"),
            ("roles:\n  web: true\n", "unknown key `roles.web`"),
            ("http: 0.0.0.0:80\n", "is not valid"),
            ("http:\n  addr: [1, 2]\n", "is not valid"),
            (
                "http:\n  addr: 0.0.0.0:80\n  addr: 0.0.0.0:81\n",
                "is not valid",
            ),
        ] {
            let err = load(Some(file), &[DATABASE]).unwrap_err().to_string();
            assert!(err.starts_with(PATH), "{err}");
            assert!(err.contains(expected), "{file}: {err}");
        }
    }

    #[test]
    fn an_invalid_value_names_its_source() {
        for (file, vars, expected) in [
            (
                "",
                &[("CIHUB_HTTP_ADDR", "localhost")][..],
                "CIHUB_HTTP_ADDR",
            ),
            ("http:\n  addr: localhost\n", &[], "`http.addr` in /etc"),
            (
                "",
                &[("CIHUB_INTERNAL_ADDR", "localhost")],
                "CIHUB_INTERNAL_ADDR",
            ),
            ("", &[("CIHUB_LOG_FILTER", "info,=[")], "CIHUB_LOG_FILTER"),
            ("log:\n  filter: info,=[\n", &[], "`log.filter` in /etc"),
            ("", &[("CIHUB_LOG_FORMAT", "text")], "CIHUB_LOG_FORMAT"),
            ("log:\n  format: text\n", &[], "`log.format` in /etc"),
            (
                "",
                &[("CIHUB_DATABASE_POOL_SIZE", "0")],
                "CIHUB_DATABASE_POOL_SIZE",
            ),
            (
                "database:\n  idle_timeout: -1\n",
                &[],
                "`database.idle_timeout` in /etc",
            ),
            (
                "database:\n  connection_timeout: 5s\n",
                &[],
                "`database.connection_timeout` in /etc",
            ),
        ] {
            let mut vars = vars.to_vec();
            vars.push(DATABASE);
            let err = load(Some(file), &vars).unwrap_err().to_string();
            assert!(err.starts_with(expected), "{err}");
        }
    }

    #[test]
    fn the_database_url_is_required() {
        for (file, vars) in [
            (None, &[][..]),
            (Some("http:\n  addr: 0.0.0.0:80\n"), &[]),
            (None, &[("CIHUB_DATABASE_URL", "")]),
        ] {
            let err = load(file, vars).unwrap_err().to_string();
            assert_eq!(err, "CIHUB_DATABASE_URL or `database.url` is required");
        }
    }

    #[test]
    fn the_github_app_needs_one_key_and_a_webhook_secret() {
        let app = [("CIHUB_GITHUB_APP_ID", "123"), DATABASE];
        for (file, vars) in [
            ("", &[("CIHUB_GITHUB_WEBHOOK_SECRET", "s")][..]),
            (
                "github:\n  private_key_file: /key.pem\n",
                &[
                    ("CIHUB_GITHUB_PRIVATE_KEY", "key"),
                    ("CIHUB_GITHUB_WEBHOOK_SECRET", "s"),
                ],
            ),
            ("github:\n  private_key: key\n", &[]),
        ] {
            let mut vars = vars.to_vec();
            vars.extend(app);
            let err = load(Some(file), &vars).unwrap_err().to_string();
            assert!(
                err.starts_with("CIHUB_GITHUB_APP_ID or `github.app_id` needs"),
                "{err}"
            );
        }

        let app = load(
            Some("github:\n  private_key: key\n  webhook_secret: s\n"),
            &app,
        )
        .unwrap()
        .github
        .unwrap();

        assert_eq!(app.config.url, DEFAULT_GITHUB_URL);
        assert_eq!(app.config.app_id, "123");
        assert!(app.config.sign_in.is_none());
    }

    #[test]
    fn the_public_url_is_the_root_address_of_the_instance() {
        let url = |value| load(None, &[DATABASE, ("CIHUB_HTTP_PUBLIC_URL", value)]);

        assert_eq!(
            url("https://ci.example.com/")
                .unwrap()
                .public_url
                .as_deref(),
            Some("https://ci.example.com")
        );
        assert!(load(None, &[DATABASE]).unwrap().public_url.is_none());
        for wrong in ["ci.example.com", "https://ci.example.com/cihub", "https://"] {
            let err = url(wrong).unwrap_err().to_string();
            assert!(err.contains("must be the root address"), "{wrong}: {err}");
        }
    }

    #[test]
    fn the_sign_in_needs_the_client_id_and_its_secret() {
        let app = "github:\n  app_id: 123\n  private_key: key\n  webhook_secret: s\n";
        let client_id = ("CIHUB_GITHUB_CLIENT_ID", "Iv1.client");
        let client_secret = ("CIHUB_GITHUB_CLIENT_SECRET", "secret");

        let both = load(Some(app), &[DATABASE, client_id, client_secret]).unwrap();
        let alone = load(Some(app), &[DATABASE, client_id]).unwrap_err();

        let sign_in = both.github.unwrap().config.sign_in.unwrap();
        assert_eq!(sign_in.client_id, "Iv1.client");
        assert_eq!(sign_in.client_secret.expose_secret(), "secret");
        assert!(
            alone.to_string().starts_with("the sign-in needs both"),
            "{alone}"
        );
    }
}
