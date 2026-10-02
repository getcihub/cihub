//! The address of the instance and the API token of `cihub login`
//! (`AGENTS.md` D161). `CIHUB_URL` and `CIHUB_TOKEN` pass before the file.

use std::path::PathBuf;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
    pub url: String,
    pub token: String,
}

impl Credentials {
    /// The credentials of the environment, else of the file.
    pub(crate) fn load() -> anyhow::Result<Self> {
        let (url, token) = (
            std::env::var("CIHUB_URL").ok(),
            std::env::var("CIHUB_TOKEN").ok(),
        );
        if let (Some(url), Some(token)) = (&url, &token) {
            return Ok(Self {
                url: url.clone(),
                token: token.clone(),
            });
        }
        let path = path()?;
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no credentials in {}: run `cihub login`, or set CIHUB_URL and CIHUB_TOKEN",
                path.display()
            )
        })?;
        let saved: Self = serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid", path.display()))?;
        Ok(Self {
            url: url.unwrap_or(saved.url),
            token: token.unwrap_or(saved.token),
        })
    }

    /// Writes the file, readable by its owner only, and returns its path.
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
}

/// `$XDG_CONFIG_HOME/cihub/credentials.json`, else
/// `~/.config/cihub/credentials.json`.
fn path() -> anyhow::Result<PathBuf> {
    let config = match std::env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".config"),
            None => bail!("neither XDG_CONFIG_HOME nor HOME is set"),
        },
    };
    Ok(config.join("cihub").join("credentials.json"))
}
