//! The commands that a comment on a PR holds (`docs/GITHUB.md`). Lot 5 reads
//! `merge` only; `cancel` and `restart` come with their lots.

use cihub_domain::Priority;

/// `/cihub merge [--priority=urgent|high|medium|low|0-255] [--no-batch]
/// [--direct] [--queue=<name>]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Merge {
    /// `None` leaves the priority to the `priority_rules` (`AGENTS.md` D148).
    pub priority: Option<Priority>,
    pub no_batch: bool,
    pub direct: bool,
    /// The queue of the branch to enter, by its name (D148).
    pub queue: Option<String>,
}

/// The first line of `body` that is a `merge` command. A command with an
/// option that CIHub does not know is not a command.
pub(crate) fn parse_merge(body: &str) -> Option<Merge> {
    body.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        if !matches!(words.next(), Some("/cihub")) || words.next() != Some("merge") {
            return None;
        }
        let mut merge = Merge {
            priority: None,
            no_batch: false,
            direct: false,
            queue: None,
        };
        for word in words {
            match word {
                "--no-batch" => merge.no_batch = true,
                "--direct" => merge.direct = true,
                _ => {
                    if let Some(name) = word.strip_prefix("--queue=") {
                        if name.is_empty() {
                            return None;
                        }
                        merge.queue = Some(name.to_owned());
                        continue;
                    }
                    let value = word.strip_prefix("--priority=")?;
                    let priority =
                        Priority::from_name(value).or_else(|| value.parse().ok().map(Priority))?;
                    merge.priority = Some(priority);
                }
            }
        }
        Some(merge)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "each case of the table compares an `Option`"
    )]
    fn merge(priority: Option<Priority>, no_batch: bool, direct: bool) -> Option<Merge> {
        Some(Merge {
            priority,
            no_batch,
            direct,
            queue: None,
        })
    }

    #[test]
    fn merge_commands_and_their_options() {
        let cases = [
            ("/cihub merge", merge(None, false, false)),
            ("LGTM\n  /cihub merge --no-batch", merge(None, true, false)),
            (
                "/cihub merge --priority=urgent --direct",
                merge(Some(Priority::URGENT), false, true),
            ),
            (
                "/cihub merge --priority=42",
                merge(Some(Priority(42)), false, false),
            ),
            (
                "/cihub merge --queue=hotfix --no-batch",
                Some(Merge {
                    queue: Some("hotfix".into()),
                    ..merge(None, true, false).unwrap()
                }),
            ),
            ("/cihub merge --queue=", None),
            ("/cihub merge --priority=256", None),
            ("/cihub merge --priority=soon", None),
            ("/cihub merge --force", None),
            ("/cihub cancel", None),
            ("please /cihub merge", None),
            ("/cihubmerge", None),
            ("", None),
        ];
        for (body, want) in cases {
            assert_eq!(parse_merge(body), want, "{body:?}");
        }
    }
}
