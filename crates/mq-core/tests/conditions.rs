//! What keeps a PR out of its queue, submitted or not: the reasons that the
//! PR alone gives, which its conditions status shows (lot 9b).

use std::collections::BTreeSet;

use cihub_domain::{
    BlockReason, CheckConclusion, CheckKind, CheckReport, Mergeability, PrState, ReviewDecision,
};
use cihub_mq_core::{PrObservation, conditions_of};

fn pr(mergeability: Mergeability, unmet: &[&str]) -> PrObservation {
    PrObservation {
        state: PrState::Open,
        head_sha: "head".into(),
        base_branch: "main".into(),
        draft: false,
        labels: BTreeSet::new(),
        mergeability,
        review_decision: ReviewDecision::Approved,
        checks: vec![CheckReport {
            name: "ci".into(),
            kind: CheckKind::Check,
            conclusion: Some(CheckConclusion::Failure),
            details_url: None,
        }],
        unmet_conditions: unmet.iter().map(|&c| c.to_owned()).collect(),
        required_checks: vec!["ci".into()],
    }
}

#[test]
fn the_conditions_of_a_pr_are_its_mergeability_then_its_unmet_conditions() {
    let unmet = BlockReason::QueueConditionsUnmet {
        conditions: vec!["label = ready".into()],
    };
    let cases = [
        (pr(Mergeability::Mergeable, &[]), vec![]),
        (
            pr(Mergeability::Mergeable, &["label = ready"]),
            vec![unmet.clone()],
        ),
        (
            pr(Mergeability::Conflict, &["label = ready"]),
            vec![BlockReason::MergeConflict, unmet],
        ),
        (
            pr(Mergeability::Blocked, &[]),
            vec![BlockReason::FailingRequiredCheck { check: "ci".into() }],
        ),
        (
            pr(Mergeability::Unknown, &[]),
            vec![BlockReason::Undetermined],
        ),
    ];

    for (n, (pr, want)) in cases.into_iter().enumerate() {
        assert_eq!(conditions_of(&pr), want, "case {n}");
    }
}
