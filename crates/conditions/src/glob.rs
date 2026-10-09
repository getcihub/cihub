use std::fmt;

use globset::{GlobBuilder, GlobMatcher};

/// A glob on a path from the root of the repository: `*` stays inside one
/// segment, `**` crosses `/`. The `files *=` condition and the patterns of
/// `affected_targets` share it (`DECISIONS.md` D227).
#[derive(Clone, Debug)]
pub struct PathGlob(GlobMatcher);

impl PathGlob {
    /// # Errors
    ///
    /// The reason why `pattern` is not a valid glob.
    pub fn new(pattern: &str) -> Result<Self, String> {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob| Self(glob.compile_matcher()))
            .map_err(|e| e.kind().to_string())
    }

    pub fn is_match(&self, path: &str) -> bool {
        self.0.is_match(path)
    }

    pub fn as_str(&self) -> &str {
        self.0.glob().glob()
    }
}

impl PartialEq for PathGlob {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for PathGlob {}

impl fmt::Display for PathGlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_stays_in_a_segment_and_double_star_crosses() {
        let cases = [
            ("go.mod", "go.mod", true),
            ("go.mod", "a/go.mod", false),
            ("*.md", "README.md", true),
            ("*.md", "docs/a.md", false),
            ("**/*.md", "docs/a.md", true),
            ("**/*.md", "README.md", true),
            ("apps/web/**", "apps/web/a/b.ts", true),
            ("apps/web/**", "apps/webx/a.ts", false),
            ("apps/web/*", "apps/web/a/b.ts", false),
            ("services/{api,auth}/**", "services/auth/x", true),
        ];
        for (pattern, path, expected) in cases {
            let glob = PathGlob::new(pattern).unwrap();
            assert_eq!(glob.is_match(path), expected, "{pattern} on {path}");
            assert_eq!(glob.as_str(), pattern);
        }
        assert!(PathGlob::new("a[").is_err());
    }
}
