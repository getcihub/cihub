use globset::{GlobBuilder, GlobMatcher};

use crate::ConditionErrorKind;

/// A glob as `matches` reads it: `*` and `?` stay in one `/` segment, `**`
/// crosses them.
#[derive(Clone, Debug)]
pub struct Glob(GlobMatcher);

impl Glob {
    /// # Errors
    ///
    /// `pattern` is not a valid glob.
    pub fn new(pattern: &str) -> Result<Self, ConditionErrorKind> {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob| Self(glob.compile_matcher()))
            .map_err(|error| ConditionErrorKind::InvalidGlob(error.kind().to_string()))
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glob_stays_in_one_segment_unless_double() {
        let release = Glob::new("release/*").unwrap();

        assert!(release.is_match("release/2.0"));
        assert!(!release.is_match("release/2.0/fix"));
        assert!(!release.is_match("main"));
        assert!(Glob::new("team/**").unwrap().is_match("team/a/b"));
        assert!(matches!(
            Glob::new("release/[").unwrap_err(),
            ConditionErrorKind::InvalidGlob(_)
        ));
    }
}
