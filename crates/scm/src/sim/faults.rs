use cihub_domain::JobClass;
use time::OffsetDateTime;

use super::{Sim, State};
use crate::ScmError;

/// A method of [`crate::Scm`] that makes a call, as the simulator records it
/// and as a [`Fault`] names it. The methods that make no call (`pr_url`,
/// `branch_rules_url`, `sign_in_url`) have none, and `pull_request` records as
/// [`ScmMethod::PullRequests`], the read that it makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScmMethod {
    /// [`crate::RepoCalls::pull_requests`].
    PullRequests,
    /// [`crate::RepoCalls::open_pull_requests`].
    OpenPullRequests,
    /// [`crate::RepoCalls::branch_head`].
    BranchHead,
    /// [`crate::RepoCalls::checks`].
    Checks,
    /// [`crate::RepoCalls::required_checks`].
    RequiredChecks,
    /// [`crate::RepoCalls::file`].
    File,
    /// [`crate::RepoCalls::permission`].
    Permission,
    /// [`crate::RepoCalls::is_ancestor`].
    IsAncestor,
    /// [`crate::RepoCalls::pr_files`].
    PrFiles,
    /// [`crate::RepoCalls::unresolved_threads`].
    UnresolvedThreads,
    /// [`crate::RepoCalls::set_branch`].
    SetBranch,
    /// [`crate::RepoCalls::delete_branch`].
    DeleteBranch,
    /// [`crate::RepoCalls::merge_into_branch`].
    MergeIntoBranch,
    /// [`crate::RepoCalls::merge_pr`].
    MergePr,
    /// [`crate::RepoCalls::set_status`].
    SetStatus,
    /// [`crate::RepoCalls::create_comment`].
    CreateComment,
    /// [`crate::RepoCalls::update_comment`].
    UpdateComment,
    /// [`crate::RepoCalls::acknowledge_comment`].
    AcknowledgeComment,
    /// [`crate::RepoCalls::add_label`].
    AddLabel,
    /// [`crate::RepoCalls::remove_label`].
    RemoveLabel,
    /// [`crate::RepoCalls::open_draft_pr`].
    OpenDraftPr,
    /// [`crate::RepoCalls::close_pr`].
    ClosePr,
    /// [`crate::RepoCalls::check_setup`].
    CheckSetup,
    /// [`crate::AppCalls::recent_deliveries`].
    RecentDeliveries,
    /// [`crate::AppCalls::redeliver`].
    Redeliver,
    /// [`crate::AppCalls::installation`].
    Installation,
    /// [`crate::AppCalls::installations`].
    Installations,
    /// [`crate::AppCalls::list_repos`].
    ListRepos,
    /// [`crate::AppCalls::members`].
    Members,
    /// [`crate::AppCalls::account_profile`].
    AccountProfile,
    /// [`crate::AppCalls::app_install_url`].
    AppInstallUrl,
    /// [`crate::PersonCalls::sign_in`].
    SignIn,
    /// [`crate::PersonCalls::refresh_person_token`].
    RefreshPersonToken,
    /// [`crate::PersonCalls::person_reach`].
    PersonReach,
    /// [`crate::PersonCalls::person_permission`].
    PersonPermission,
    /// [`crate::PersonCalls::person_role`].
    PersonRole,
}

/// Makes the next calls fail with `error`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The method to fail, or `None` for every method.
    pub call: Option<ScmMethod>,
    /// The error that each failed call returns.
    pub error: ScmError,
    /// The number of calls to fail.
    pub times: u32,
}

/// A call that the simulator received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimCall {
    /// The method that made the call.
    pub method: ScmMethod,
    /// The class that the call named.
    pub class: JobClass,
}

/// How [`Sim::take_deliveries_with`] damages the deliveries. The same seed
/// gives the same damage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookFaults {
    /// The seed of the draws.
    pub seed: u64,
    /// The share of deliveries lost, in percent.
    pub lose_percent: u8,
    /// The share of deliveries sent twice, in percent.
    pub duplicate_percent: u8,
    /// Shuffles the deliveries that are kept.
    pub reorder: bool,
}

impl WebhookFaults {
    pub(super) fn next(&mut self) -> u64 {
        splitmix(&mut self.seed)
    }

    pub(super) fn chance(&mut self, percent: u8) -> bool {
        self.next() % 100 < u64::from(percent)
    }
}

/// Makes calls fail at random, after the faults of [`Sim::fail`]. The same seed
/// gives the same failures for the same sequence of calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomFaults {
    /// The seed of the draws.
    pub seed: u64,
    /// The share of calls that fail with a server error, in percent.
    pub unavailable_percent: u8,
    /// The share of calls that fail with a spent rate limit, in percent.
    pub rate_limited_percent: u8,
    /// The end of the rate limit that such a failure reports.
    pub rate_limited_until: OffsetDateTime,
}

impl RandomFaults {
    fn draw(&mut self) -> Option<ScmError> {
        let roll = splitmix(&mut self.seed) % 100;
        let unavailable = u64::from(self.unavailable_percent);
        if roll < unavailable {
            Some(ScmError::Unavailable("random server error".into()))
        } else if roll < unavailable + u64::from(self.rate_limited_percent) {
            Some(ScmError::RateLimited {
                until: self.rate_limited_until,
            })
        } else {
            None
        }
    }
}

fn splitmix(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

    z ^ (z >> 31)
}

impl Sim {
    /// Makes the next calls fail, after the faults already waiting.
    pub fn fail(&self, fault: Fault) {
        self.with(|state| state.faults.push_back(fault));
    }

    /// Makes calls fail at random as `faults` says, or stops it with `None`.
    pub fn fail_at_random(&self, faults: Option<RandomFaults>) {
        self.with(|state| state.random = faults);
    }

    /// The calls received since the start.
    pub fn calls(&self) -> Vec<SimCall> {
        self.with(|state| state.calls.clone())
    }
}

impl State {
    pub(super) fn take_fault(&mut self, method: ScmMethod) -> Option<ScmError> {
        let Some(index) = self
            .faults
            .iter()
            .position(|fault| fault.call.is_none_or(|call| call == method))
        else {
            return self.random.as_mut().and_then(RandomFaults::draw);
        };

        let fault = &mut self.faults[index];
        let error = fault.error.clone();

        fault.times = fault.times.saturating_sub(1);
        if fault.times == 0 {
            self.faults.remove(index);
        }

        Some(error)
    }
}
