//! The repository of `cihub queue`: the value of `--repo`, else the remote
//! `origin` of git in the working directory.

use std::process::Command;

use anyhow::{Context, bail};

pub(crate) fn from_git() -> anyhow::Result<(String, String)> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("cannot run git")?;
    if !output.status.success() {
        bail!("no remote `origin`: run cihub in a clone of the repository");
    }
    let url = String::from_utf8_lossy(&output.stdout);
    parse(url.trim()).with_context(|| format!("the remote `{}` names no repository", url.trim()))
}

pub(crate) fn from_arg(value: &str) -> Result<(String, String), String> {
    match value.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() && !name.contains('/') => {
            Ok((owner.to_owned(), name.to_owned()))
        }
        _ => Err(format!("`{value}` is not `owner/name`")),
    }
}

/// The last two parts of the path of a remote, without `.git`: the forms
/// `git@host:owner/name.git`, `ssh://git@host/owner/name` and
/// `https://host/owner/name.git`.
fn parse(url: &str) -> Option<(String, String)> {
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?.1,
        None => url.split_once(':')?.1,
    };
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = path.rsplit_once('/')?;
    let owner = owner.rsplit('/').next()?;
    (!owner.is_empty() && !name.is_empty()).then(|| (owner.to_owned(), name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_names_its_repository() {
        let acme = Some(("acme".to_owned(), "app".to_owned()));
        for url in [
            "git@github.com:acme/app.git",
            "git@github.com:acme/app",
            "ssh://git@github.example.com:2222/acme/app.git",
            "https://github.com/acme/app.git",
            "https://github.com/acme/app/",
            "https://gitlab.example.com/group/acme/app.git",
        ] {
            assert_eq!(parse(url), acme, "{url}");
        }
        assert_eq!(parse("https://github.com/app"), None);
        assert_eq!(parse("app"), None);
    }

    #[test]
    fn an_argument_names_its_repository() {
        assert_eq!(
            from_arg("acme/app"),
            Ok(("acme".to_owned(), "app".to_owned()))
        );
        for value in ["acme", "acme/", "/app", "acme/app/x"] {
            assert!(from_arg(value).is_err(), "{value}");
        }
    }
}
