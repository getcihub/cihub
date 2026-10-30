//! The domain model of CIHub: the identifiers, the queue and what it holds,
//! the events of its log, the reasons of its decisions, and the closed value
//! sets of its settings. The other crates share these types.
//!
//! The crate is pure: it does no I/O, reads no clock and draws no random
//! value. The caller gives every time and every identifier; the one
//! exception is `new` on a typed identifier, which makes a ULID from the
//! clock. It does not validate the settings (`cihub-config`), store anything
//! (`cihub-store`) or talk to an SCM (`cihub-scm`).
//!
//! # Components
//!
//! - Identifiers (`id`): one typed identifier per record, such as [`OrgId`]
//!   or [`ItemId`]. See [Identifiers](#identifiers).
//! - The queue (`queue`): a [`Queue`] per target branch of a repository,
//!   with its [`QueueState`] and the [`QueueControl`] of an operator.
//! - Items (`item`): an [`Item`] is one submission of a PR to a queue, with
//!   its [`ItemState`], [`Priority`] and [`BlockReason`]s.
//! - Groups (`group`): a [`TestGroup`] is the unit that the queue tests, one
//!   PR or a batch, with its [`GroupState`].
//! - Runs (`run`): a [`TestRun`] is one attempt to test a group on a test
//!   branch, with its [`RunStatus`].
//! - Checks (`check`): a [`CheckReport`] is what the SCM says of a check on a
//!   commit; a [`CheckObservation`] is the last report on the test commit of
//!   a run.
//! - PRs (`pr`): what CIHub reads of a PR, [`PrState`], [`Mergeability`] and
//!   [`ReviewDecision`].
//! - Events (`event`): an [`Event`] is one record of the log of a queue. Its
//!   [`EventKind`] is a fact (what CIHub observed) or a decision (what the
//!   queue did), as [`EventType::is_decision`] says.
//! - Reasons (`reason`): a [`Reason`] explains each change of state, from a
//!   closed vocabulary.
//! - Transitions (`transition`): the tables of the permitted changes of
//!   state, and the reasons that each one accepts ([`check_transition`]).
//! - Jobs (`job`): the [`JobClass`] of a call on the budget of a connection,
//!   the [`JobState`] of a job, and the [`JobError`] that a job reports.
//! - Settings (`settings`): the closed value sets of the settings of a
//!   queue, such as [`Strategy`] and [`MergeMethod`].
//! - Tenancy (`tenancy`): connections to an SCM, roles, permissions and the
//!   scopes of an API token.
//! - Tokens (`token`): the text forms of the secrets.
//! - Branches (`branch`): the names of the test branches.
//!
//! The log is the source of truth of a queue (principle I): the states of
//! the items, groups, runs and queues are projections that a replay of the
//! events rebuilds. Every change of state carries a [`Reason`] (principle
//! VI), which [`check_transition`] checks against the table of its
//! [`Lifecycle`].
//!
//! # Identifiers
//!
//! A typed identifier has the text form `<prefix>_<32 characters>`, for
//! example `org_0627ank54ajth78mw4353dqpw5dvt5kp`. The 32 characters encode
//! in base32 a ULID (16 bytes, ordered by time) and a CRC32 checksum of the
//! prefix and the ULID (4 bytes). The checksum catches a typo, a truncation
//! or a wrong prefix. Postgres stores the text form in a `text` column.
//!
//! | Type | Prefix | What it designates | Table |
//! | --- | --- | --- | --- |
//! | [`OrgId`] | `org` | An organization of CIHub: the boundary of a tenant | `orgs` |
//! | [`RepoId`] | `repo` | A repository that a connection reaches | `repos` |
//! | [`QueueId`] | `queue` | The queue of one target branch of a repository | `queues` |
//! | [`ItemId`] | `item` | One submission of a PR to a queue | `queue_items` |
//! | [`GroupId`] | `group` | A test group: one PR, a batch or a half of a bisection | `test_groups` |
//! | [`RunId`] | `run` | One attempt to test a group | `test_runs` |
//! | [`EventId`] | `event` | One event of the log of a queue | `events` |
//! | [`ConnectionId`] | `connection` | The access of CIHub to an SCM account | `scm_connections` |
//! | [`UserId`] | `user` | A person, from their SCM account; global | `users` |
//! | [`ApiTokenId`] | `apitoken` | The row of an API token, not the secret | `api_tokens` |
//! | [`AuditEntryId`] | `audit` | One entry of the audit log | `audit_log` |
//! | [`SessionId`] | `session` | A session of the dashboard or the command line; global | `sessions` |
//! | [`DeviceCodeId`] | `devicecode` | A sign-in of the command line in progress; global | `device_codes` |
//! | [`RequestId`] | `request` | One API request, in the logs; never stored | none |
//!
//! Other values identify a record without a typed identifier:
//!
//! - A PR is its number on its repository (`pr_number`). A queue has at most
//!   one current item per PR number.
//! - A commit is its SHA, as the SCM gives it (`head_sha`, `base_sha`,
//!   `test_sha`, `merged_sha`).
//! - A queue is also unique by its repository and its target branch.
//! - A test branch carries its run: [`test_branch`] names it
//!   `<prefix>-merge/pr-<n>/<run_id>`, where `n` is the number of the first PR
//!   of the group, and [`run_of_test_branch`] reads the [`RunId`] back, so
//!   that a check on the branch reaches its run. [`temp_branch`] gives the
//!   temporary branch of the assembly, `<prefix>-temp/pr-<n>/<run_id>`.
//! - A secret has a prefix that tells its kind ([`API_TOKEN_PREFIX`],
//!   [`SESSION_TOKEN_PREFIX`], [`CLI_SESSION_TOKEN_PREFIX`],
//!   [`DEVICE_CODE_PREFIX`]). The store keeps only its SHA-256; the row has
//!   its own typed identifier.
//! - The identifiers of the SCM (`external_id`: the account, the
//!   installation, the repository, the user) are opaque text. This crate does
//!   not carry them: `cihub-scm` does, and the tables keep them next to the
//!   typed identifier (`repos.external_id`, `users.external_id`).
//!
//! The states, the transitions and the reasons are described in
//! `specs/DOMAIN.md`.

#![deny(missing_docs)]

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
            /// The `snake_case` name of the value, as Postgres, the API and
            /// the logs store it.
            pub fn as_str(self) -> &'static str {
                self.into()
            }
        }
    };
}

mod branch;
mod check;
mod event;
mod group;
mod id;
mod item;
mod job;
mod pr;
mod queue;
mod reason;
mod run;
mod settings;
mod tenancy;
mod token;
mod transition;

pub use branch::{run_of_test_branch, temp_branch, test_branch};
pub use check::{
    AwaitedCheckStatus, CheckConclusion, CheckKind, CheckObservation, CheckReport, CheckStatus,
};
pub use event::{Actor, ActorKind, Event, EventKind, EventType, Subjects};
pub use group::{GroupKind, GroupState, TestGroup};
pub use id::{
    ApiTokenId, AuditEntryId, ConnectionId, DeviceCodeId, EventId, GroupId, IdParseError, ItemId,
    OrgId, QueueId, RepoId, RequestId, RunId, SessionId, UserId,
};
pub use item::{BlockReason, ImpactedTargets, Item, ItemState, Priority, SubmittedVia};
pub use job::{JobClass, JobError, JobState};
pub use pr::{Mergeability, PrState, ReviewDecision};
pub use queue::{Queue, QueueControl, QueueState};
pub use reason::Reason;
pub use run::{ConflictCulprit, RequiredStatusesSource, RunStatus, TestRun};
pub use settings::{
    BranchProtectionInjection, DirectMergeMode, MergeCommitTitle, MergeMethod, OptimizationMode,
    QueueMode, Strategy, Trigger, UpdateMethod,
};
pub use tenancy::{
    ApiScope, ConnectionKind, ConnectionMode, DeviceCodeState, MembershipRole, Permission,
    RepoConfigState,
};
pub use token::{
    API_TOKEN_PREFIX, CLI_SESSION_TOKEN_PREFIX, DEVICE_CODE_PREFIX, SESSION_TOKEN_PREFIX,
    format_api_token, format_cli_session_token, format_device_code, format_session_token,
    format_user_code, is_api_token, is_cli_session_token, is_device_code, is_session_token,
    parse_user_code,
};
pub use transition::{Lifecycle, TransitionError, check_transition};
