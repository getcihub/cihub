//! Domain model: identifiers, states, reasons, settings and events. Pure: no I/O.

/// Declares a closed vocabulary enum. The `snake_case` name is the same in
/// serde, `as_str`, `Display` and `FromStr`, because it is stored in Postgres enums and
/// exposed in the API, webhooks and analytics.
macro_rules! closed_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($(#[$vmeta:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash,
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

mod event;
mod id;
mod item;
mod queue;
mod reason;
mod run;
mod settings;
mod state;
mod transition;

pub use event::{Actor, Event, EventKind};
pub use id::{EventId, GroupId, IdParseError, ItemId, OrgId, QueueId, RepoId, RunId};
pub use item::{BlockReason, ImpactedTargets, Item, Priority};
pub use queue::Queue;
pub use reason::{CancelledReason, FailedReason, MergedReason, PendingReason, TestsPassedReason};
pub use run::{CheckObservation, ConflictCulprit, TestGroup, TestRun};
pub use settings::{
    BranchProtectionInjection, CheckConclusion, CheckKind, DirectMergeMode, GroupKind,
    MergeCommitTitle, MergeMethod, OptimizationMode, QueueMode, RunStatus, Strategy, SubmittedVia,
    Trigger, UpdateMethod,
};
pub use state::{ItemState, QueueState};
pub use transition::{Target, Transition, TransitionError, transition};
