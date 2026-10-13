//! Domain model: identifiers, states, reasons, settings and events. Pure: no I/O.

/// Declares a closed vocabulary enum. The `snake_case` name is the same in
/// serde, `as_str`, `Display` and `FromStr`, because it is stored in Postgres enums and
/// exposed in the API, webhooks and analytics.
macro_rules! closed_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($(#[$vmeta:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
            serde::Serialize, serde::Deserialize,
            strum::Display, strum::EnumString, strum::IntoStaticStr, strum::EnumIter,
        )]
        #[serde(rename_all = "snake_case")]
        #[strum(serialize_all = "snake_case")]
        $vis enum $name { $($(#[$vmeta])* $variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                self.into()
            }
        }
    };
}

mod branch;
mod event;
mod id;
mod item;
mod job;
mod pr;
mod queue;
mod reason;
mod run;
mod settings;
mod state;
mod tenancy;
mod token;
mod transition;

pub use branch::{run_of_test_branch, temp_branch, test_branch};
pub use event::{Actor, ActorKind, Event, EventKind, EventType, Subjects};
pub use id::{
    ApiTokenId, AuditEntryId, ConnectionId, EventId, GroupId, IdParseError, ItemId, OrgId, QueueId,
    RepoId, RunId, SessionId, UserId,
};
pub use item::{BlockReason, ImpactedTargets, Item, Priority};
pub use job::{JobClass, JobError, JobState};
pub use pr::{CheckReport, Mergeability, Permission, PrState, ReviewDecision};
pub use queue::{Queue, QueueControl};
pub use reason::Reason;
pub use run::{CheckObservation, ConflictCulprit, RequiredStatusesSource, TestGroup, TestRun};
pub use settings::{
    BranchProtectionInjection, CheckConclusion, CheckKind, DirectMergeMode, GroupKind,
    MergeCommitTitle, MergeMethod, OptimizationMode, QueueMode, RunStatus, Strategy, SubmittedVia,
    Trigger, UpdateMethod,
};
pub use state::{GroupState, ItemState, QueueState};
pub use tenancy::{ApiScope, ConnectionKind, ConnectionMode, MembershipRole};
pub use token::{
    API_TOKEN_PREFIX, SESSION_TOKEN_PREFIX, format_api_token, format_session_token, is_api_token,
    is_session_token,
};
pub use transition::{Lifecycle, TransitionError, check_transition};
