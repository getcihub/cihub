use std::collections::BTreeSet;

use cihub_conditions::PathGlob;
use cihub_domain::AffectedTargets;

/// Where a queue in `parallel` mode reads the affected targets of a PR
/// (`DECISIONS.md` D226).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetsSource {
    /// CIHub computes them from the changed files of the PR.
    Files(Vec<Target>),
    /// The CI of the user uploads them.
    Upload,
}

/// A named target of `source: files` and its patterns, in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    pub patterns: Vec<TargetPattern>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetPattern {
    /// A pattern written `!<glob>` excludes.
    pub exclude: bool,
    pub glob: PathGlob,
}

impl Target {
    /// The last pattern that matches `path` decides, as in `.gitignore`
    /// (`DECISIONS.md` D227).
    pub fn keeps(&self, path: &str) -> bool {
        self.patterns
            .iter()
            .rev()
            .find(|pattern| pattern.glob.is_match(path))
            .is_some_and(|pattern| !pattern.exclude)
    }
}

/// The targets of a PR from its changed paths: the union of the targets
/// that keep each path, or `All` as soon as a path has none
/// (`DECISIONS.md` D227). The caller passes both paths of a renamed file and
/// gives `All` itself when the list of files is cut.
pub fn targets_of<'a>(
    targets: &[Target],
    paths: impl IntoIterator<Item = &'a str>,
) -> AffectedTargets {
    let mut affected = BTreeSet::new();
    for path in paths {
        let mut kept = false;
        for target in targets.iter().filter(|target| target.keeps(path)) {
            kept = true;
            affected.insert(target.name.clone());
        }
        if !kept {
            return AffectedTargets::All;
        }
    }
    AffectedTargets::Targets(affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(name: &str, patterns: &[&str]) -> Target {
        Target {
            name: name.into(),
            patterns: patterns
                .iter()
                .map(|p| {
                    let (exclude, glob) = p.strip_prefix('!').map_or((false, *p), |g| (true, g));
                    TargetPattern {
                        exclude,
                        glob: PathGlob::new(glob).unwrap(),
                    }
                })
                .collect(),
        }
    }

    fn set(names: &[&str]) -> AffectedTargets {
        AffectedTargets::Targets(names.iter().map(|n| (*n).to_owned()).collect())
    }

    #[test]
    fn the_last_matching_pattern_decides() {
        let web = target(
            "web",
            &["apps/web/**", "!apps/web/**/*.md", "apps/web/KEEP.md"],
        );
        let cases = [
            ("apps/web/a.ts", true),
            ("apps/web/docs/a.md", false),
            ("apps/web/KEEP.md", true),
            ("services/api/a.go", false),
        ];
        for (path, expected) in cases {
            assert_eq!(web.keeps(path), expected, "{path}");
        }
    }

    #[test]
    fn a_pr_takes_the_union_or_all() {
        let targets = [
            target("web", &["apps/web/**", "libs/auth/**", "!apps/web/**/*.md"]),
            target("api", &["services/api/**", "libs/auth/**"]),
            target("docs", &["docs/**", "**/*.md"]),
        ];
        let cases: [(&[&str], AffectedTargets); 6] = [
            (&["apps/web/a.ts"], set(&["web"])),
            (
                &["apps/web/a.ts", "services/api/b.go"],
                set(&["api", "web"]),
            ),
            (&["libs/auth/token.ts"], set(&["api", "web"])),
            (&["apps/web/README.md"], set(&["docs"])),
            (
                &["apps/web/a.ts", ".github/workflows/ci.yml"],
                AffectedTargets::All,
            ),
            (&[], set(&[])),
        ];
        for (paths, expected) in cases {
            assert_eq!(
                targets_of(&targets, paths.iter().copied()),
                expected,
                "{paths:?}"
            );
        }
    }
}
