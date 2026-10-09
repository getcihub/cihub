//! The names of the test branches (lot 6, a frozen interface). The assembly
//! job creates `<prefix>-temp/pr-<n>/<run_id>`, merges the heads there, then
//! creates `<prefix>-merge/pr-<n>/<run_id>` at the result and deletes the
//! temporary branch. The name of a test branch leads back to its run: a check
//! reported on a test branch is routed to its queue by it.

use crate::RunId;

/// The test branch of the run `run_id`, for a group whose first PR is
/// `pr_number`. `prefix` holds no `/`.
pub fn test_branch(prefix: &str, pr_number: u64, run_id: RunId) -> String {
    format!("{prefix}-merge/pr-{pr_number}/{run_id}")
}

/// The temporary branch where the assembly merges the heads of a run, from
/// its test branch; `None` for a name that is not a test branch.
pub fn temp_branch(test_branch: &str) -> Option<String> {
    let (prefix, rest) = test_branch.split_once("-merge/")?;
    Some(format!("{prefix}-temp/{rest}"))
}

/// The run whose test branch is `branch`; `None` for any other branch.
pub fn run_of_test_branch(branch: &str) -> Option<RunId> {
    let (_, rest) = branch.split_once("-merge/pr-")?;
    let (pr_number, run_id) = rest.split_once('/')?;
    pr_number.parse::<u64>().ok()?;
    run_id.parse().ok()
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn a_test_branch_names_its_run_and_its_temporary_branch() {
        let run_id = RunId::from_ulid(Ulid(7));
        let branch = test_branch("cihub", 12, run_id);

        assert_eq!(branch, format!("cihub-merge/pr-12/{run_id}"));
        assert_eq!(run_of_test_branch(&branch), Some(run_id));
        assert_eq!(
            temp_branch(&branch).as_deref(),
            Some(format!("cihub-temp/pr-12/{run_id}").as_str())
        );
        let custom = test_branch("ship", 3, run_id);
        assert_eq!(run_of_test_branch(&custom), Some(run_id));
        assert_eq!(
            temp_branch(&custom).as_deref(),
            Some(format!("ship-temp/pr-3/{run_id}").as_str())
        );
    }

    #[test]
    fn another_branch_names_no_run() {
        for branch in [
            "main",
            "cihub-temp/pr-12/run_0000000000000000000000000000000000",
            "cihub-merge/pr-x/run_0000000000000000000000000000000000",
            "cihub-merge/pr-12/not-a-run",
            "cihub-merge/pr-12",
        ] {
            assert_eq!(run_of_test_branch(branch), None, "{branch}");
        }
        assert_eq!(temp_branch("main"), None);
    }
}
