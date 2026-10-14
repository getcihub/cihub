//! The hosts of `cihub auth`, and the session of each one. The host is
//! `--hostname`, else `CIHUB_HOST`, else the only host that the file knows;
//! `CIHUB_TOKEN` passes before the token of the file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, bail};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

pub(crate) const HOST_VARIABLE: &str = "CIHUB_HOST";
pub(crate) const TOKEN_VARIABLE: &str = "CIHUB_TOKEN";

/// The file of the command line: a session per host.
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Credentials {
    hosts: BTreeMap<String, HostSession>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct HostSession {
    pub token: String,
    pub login: String,
}

/// Where the token of a host comes from.
pub(crate) enum TokenSource {
    Variable,
    File(PathBuf),
}

impl Credentials {
    /// The file, or no host when it does not exist yet, or when the machine
    /// has no folder for it: a CI may give only `CIHUB_HOST` and `CIHUB_TOKEN`.
    pub(crate) fn load() -> anyhow::Result<Self> {
        let Ok(path) = path() else {
            return Ok(Self::default());
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
        };

        serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display()))
    }

    pub(crate) fn save(&self) -> anyhow::Result<PathBuf> {
        let path = path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options
            .open(&path)
            .with_context(|| format!("cannot write {}", path.display()))?;

        std::io::Write::write_all(&mut file, serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(path)
    }

    pub(crate) fn hosts(&self) -> impl Iterator<Item = &str> {
        self.hosts.keys().map(String::as_str)
    }

    pub(crate) fn session(&self, host: &str) -> Option<&HostSession> {
        self.hosts.get(host)
    }

    pub(crate) fn keep(&mut self, host: &str, session: HostSession) {
        self.hosts.insert(host.to_owned(), session);
    }

    pub(crate) fn forget(&mut self, host: &str) -> Option<HostSession> {
        self.hosts.remove(host)
    }

    /// The host of a command: `hostname`, else `CIHUB_HOST`, else the only
    /// host of the file. There is no default host.
    pub(crate) fn host(&self, hostname: Option<&str>) -> anyhow::Result<String> {
        let named = hostname.map(str::to_owned).or_else(|| {
            std::env::var(HOST_VARIABLE)
                .ok()
                .filter(|host| !host.is_empty())
        });
        if let Some(host) = named {
            return check_host(&host);
        }

        let mut hosts = self.hosts();
        match (hosts.next(), hosts.next()) {
            (Some(host), None) => Ok(host.to_owned()),
            (None, _) => {
                bail!("no host: run `cihub auth login --hostname <host>`, or set {HOST_VARIABLE}")
            }
            (Some(_), Some(_)) => bail!(
                "several hosts ({}): choose one with `--hostname` or {HOST_VARIABLE}",
                self.hosts().collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// The token of `host`: `CIHUB_TOKEN`, else the session of the file.
    pub(crate) fn token(&self, host: &str) -> anyhow::Result<Option<(SecretString, TokenSource)>> {
        if let Some(token) = std::env::var(TOKEN_VARIABLE)
            .ok()
            .filter(|token| !token.is_empty())
        {
            return Ok(Some((token.into(), TokenSource::Variable)));
        }

        let Some(session) = self.session(host) else {
            return Ok(None);
        };

        Ok(Some((
            session.token.clone().into(),
            TokenSource::File(path()?),
        )))
    }
}

/// The address of `host`: HTTPS, but for a loopback host, which serves a
/// development instance over HTTP.
pub(crate) fn address(host: &str) -> String {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    let scheme = if matches!(name, "localhost" | "127.0.0.1" | "::1") {
        "http"
    } else {
        "https"
    };

    format!("{scheme}://{host}")
}

/// A host is a name, with a port or not: `cihub.example.com`,
/// `localhost:8080`; never an address.
fn check_host(host: &str) -> anyhow::Result<String> {
    if host.is_empty() || host.contains('/') {
        bail!(
            "`{host}` is not a host: give a name such as `cihub.example.com`, without `https://`"
        );
    }

    Ok(host.to_lowercase())
}

/// `$XDG_CONFIG_HOME/cihub/credentials.json`, else
/// `~/.config/cihub/credentials.json`.
pub(crate) fn path() -> anyhow::Result<PathBuf> {
    let config = match std::env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".config"),
            None => bail!("neither XDG_CONFIG_HOME nor HOME is set"),
        },
    };
    Ok(config.join("cihub").join("credentials.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loopback_host_is_served_over_http() {
        for (host, address) in [
            ("cihub.example.com", "https://cihub.example.com"),
            ("cihub.example.com:8443", "https://cihub.example.com:8443"),
            ("localhost:8080", "http://localhost:8080"),
            ("127.0.0.1:8080", "http://127.0.0.1:8080"),
            ("[::1]:8080", "http://[::1]:8080"),
            ("localhost.example.com", "https://localhost.example.com"),
        ] {
            assert_eq!(super::address(host), address, "{host}");
        }
    }

    #[test]
    fn a_host_is_a_name_and_not_an_address() {
        assert_eq!(
            check_host("CIHub.example.com").unwrap(),
            "cihub.example.com"
        );
        assert!(check_host("https://cihub.example.com").is_err());
        assert!(check_host("").is_err());
    }

    #[test]
    fn the_only_host_of_the_file_is_the_default() {
        let mut credentials = Credentials::default();
        let session = || HostSession {
            token: "chc_a".into(),
            login: "ada".into(),
        };

        credentials.keep("a.example.com", session());
        if std::env::var_os(HOST_VARIABLE).is_none() {
            assert_eq!(credentials.host(None).unwrap(), "a.example.com");
            credentials.keep("b.example.com", session());
            assert!(credentials.host(None).is_err());
        }

        assert_eq!(
            credentials.host(Some("b.example.com")).unwrap(),
            "b.example.com"
        );
    }
}
