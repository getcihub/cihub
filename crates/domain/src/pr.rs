//! What CIHub reads of a PR on the SCM. A PR is any change request: a
//! GitHub pull request, a GitLab merge request. Each SCM adapter translates
//! its own values into these ones.

closed_enum! {
    /// Whether the PR is open on the SCM.
    pub enum PrState {
        /// The PR can still merge.
        Open,
        /// The PR was closed without a merge.
        Closed,
        /// The PR merged, by the queue or outside it.
        Merged,
    }
}

closed_enum! {
    /// Whether the SCM accepts to merge the PR.
    pub enum Mergeability {
        /// Nothing on the SCM prevents the merge.
        Mergeable,
        /// The SCM requires the head to be up to date with the base.
        Behind,
        /// The PR conflicts with its base.
        Conflict,
        /// The SCM refuses the merge for another reason: a review, a
        /// required check, a draft.
        Blocked,
        /// The SCM has not computed it yet.
        Unknown,
    }
}

closed_enum! {
    /// The review decision of the PR.
    #[derive(Default)]
    pub enum ReviewDecision {
        /// The reviews that the rules require approve the PR.
        Approved,
        /// A reviewer asked for changes.
        ChangesRequested,
        /// The rules require a review that the PR does not have yet.
        ReviewRequired,
        /// No review rule applies.
        #[default]
        None,
    }
}
