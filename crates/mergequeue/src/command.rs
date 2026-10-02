//! The commands that a comment on a PR holds (`docs/GITHUB.md`). Lot 5 reads
//! `merge` only; `cancel` and `restart` come with their lots.

use cihub_domain::Priority;

/// `/cihub merge [--priority=urgent|high|medium|low|0-255] [--no-batch]
/// [--direct]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Merge {
    pub priority: Priority,
    pub no_batch: bool,
    pub direct: bool,
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
            priority: Priority::MEDIUM,
            no_batch: false,
            direct: false,
        };
        for word in words {
            match word {
                "--no-batch" => merge.no_batch = true,
                "--direct" => merge.direct = true,
                _ => {
                    let value = word.strip_prefix("--priority=")?;
                    merge.priority =
                        Priority::from_name(value).or_else(|| value.parse().ok().map(Priority))?;
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
    fn merge(priority: Priority, no_batch: bool, direct: bool) -> Option<Merge> {
        Some(Merge {
            priority,
            no_batch,
            direct,
        })
    }

    #[test]
    fn merge_commands_and_their_options() {
        let cases = [
            ("/cihub merge", merge(Priority::MEDIUM, false, false)),
            (
                "LGTM\n  /cihub merge --no-batch",
                merge(Priority::MEDIUM, true, false),
            ),
            (
                "/cihub merge --priority=urgent --direct",
                merge(Priority::URGENT, false, true),
            ),
            (
                "/cihub merge --priority=42",
                merge(Priority(42), false, false),
            ),
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
