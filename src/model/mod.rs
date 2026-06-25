//! Core graph model boundaries.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod rkyv_adapters;
use rkyv_adapters::MetadataAsJson;

pub type Metadata = BTreeMap<String, Value>;

/// Rank active candidate links by provenance precedence, then confidence,
/// then link id, and return the first. Non-active links are skipped, so the
/// helper is safe to call on either a list filtered ahead of time or a raw
/// `Vec<&GraphLink>`.
pub fn pick_preferred<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links
        .iter()
        .copied()
        .filter(|link| matches!(link.state, LinkState::Active))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

/// Component-wise prefix match so `/a/b` does **not** count as an ancestor
/// of `/a/barbecue`.
pub fn path_is_ancestor_of(ancestor: &Path, descendant: &Path) -> bool {
    let mut anc_iter = ancestor.components();
    let mut desc_iter = descendant.components();
    loop {
        match (anc_iter.next(), desc_iter.next()) {
            (Some(a), Some(d)) if a == d => continue,
            (Some(_), Some(_)) => return false,
            (Some(_), None) => return false,
            (None, _) => return true,
        }
    }
}

macro_rules! simple_id {
    ($name:ident, $kind:literal, $field:ident) => {
        #[derive(
            Clone,
            Debug,
            Eq,
            PartialEq,
            Ord,
            PartialOrd,
            Hash,
            Serialize,
            Deserialize,
            rkyv::Archive,
            rkyv::Serialize,
            rkyv::Deserialize,
        )]
        #[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
        pub struct $name {
            pub $field: String,
        }

        impl $name {
            pub fn new($field: impl Into<String>) -> Self {
                Self {
                    $field: $field.into(),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $kind, self.$field)
            }
        }
    };
}

simple_id!(RepoId, "repo", common_dir);
simple_id!(WorkspaceId, "workspace", root);
simple_id!(MuxSessionId, "mux_session", native_id);
simple_id!(ForkId, "fork", provider_source_key);
simple_id!(RuntimeProcessId, "runtime_process", observation_key);

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct CheckoutId {
    pub repo: RepoId,
    pub root: String,
}

impl CheckoutId {
    pub fn new(repo: RepoId, root: impl Into<String>) -> Self {
        Self {
            repo,
            root: root.into(),
        }
    }
}

impl fmt::Display for CheckoutId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checkout:{}@{}", self.repo, self.root)
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct AgentSessionId {
    pub harness_key: String,
    pub state_scope: String,
    pub session_key: String,
}

impl AgentSessionId {
    pub fn new(
        harness_key: impl Into<String>,
        state_scope: impl Into<String>,
        session_key: impl Into<String>,
    ) -> Self {
        Self {
            harness_key: harness_key.into(),
            state_scope: state_scope.into(),
            session_key: session_key.into(),
        }
    }
}

impl fmt::Display for AgentSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "agent_session:{}:{}:{}",
            self.harness_key, self.state_scope, self.session_key
        )
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct BranchId {
    pub repo: RepoId,
    pub refname: String,
}

impl BranchId {
    pub fn new(repo: RepoId, refname: impl Into<String>) -> Self {
        Self {
            repo,
            refname: refname.into(),
        }
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "branch:{}@{}", self.repo, self.refname)
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct ForgePrId {
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl ForgePrId {
    pub fn new(
        provider: impl Into<String>,
        host: impl Into<String>,
        owner: impl Into<String>,
        repo: impl Into<String>,
        number: u64,
    ) -> Self {
        Self {
            provider: provider.into(),
            host: host.into(),
            owner: owner.into(),
            repo: repo.into(),
            number,
        }
    }
}

impl fmt::Display for ForgePrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "forge_pr:{}:{}/{}/{}#{}",
            self.provider, self.host, self.owner, self.repo, self.number
        )
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct PinId {
    pub id: String,
}

impl PinId {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl fmt::Display for PinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pin:{}", self.id)
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "type", rename_all = "snake_case")]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum NodeId {
    Repo(RepoId),
    Checkout(CheckoutId),
    Workspace(WorkspaceId),
    AgentSession(AgentSessionId),
    MuxSession(MuxSessionId),
    Pin(PinId),
    RuntimeProcess(RuntimeProcessId),
    Branch(BranchId),
    Fork(ForkId),
    ForgePr(ForgePrId),
}

impl NodeId {
    pub fn checkout(repo: RepoId, root: impl Into<String>) -> Self {
        Self::Checkout(CheckoutId::new(repo, root))
    }

    pub fn as_checkout(&self) -> Option<&CheckoutId> {
        match self {
            Self::Checkout(id) => Some(id),
            _ => None,
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repo(id) => id.fmt(f),
            Self::Checkout(id) => id.fmt(f),
            Self::Workspace(id) => id.fmt(f),
            Self::AgentSession(id) => id.fmt(f),
            Self::MuxSession(id) => id.fmt(f),
            Self::Pin(id) => id.fmt(f),
            Self::RuntimeProcess(id) => id.fmt(f),
            Self::Branch(id) => id.fmt(f),
            Self::Fork(id) => id.fmt(f),
            Self::ForgePr(id) => id.fmt(f),
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNode {
    Repo(RepoNode),
    Checkout(CheckoutNode),
    Workspace(WorkspaceNode),
    AgentSession(AgentSessionNode),
    MuxSession(MuxSessionNode),
    Pin(PinNode),
    RuntimeProcess(RuntimeProcessNode),
    Branch(BranchNode),
    Fork(ForkNode),
    ForgePr(ForgePrNode),
}

impl GraphNode {
    pub fn checkout(id: CheckoutId, root: impl Into<String>) -> Self {
        Self::Checkout(CheckoutNode::new(id, root))
    }

    pub fn id(&self) -> NodeId {
        match self {
            Self::Repo(node) => NodeId::Repo(node.id.clone()),
            Self::Checkout(node) => NodeId::Checkout(node.id.clone()),
            Self::Workspace(node) => NodeId::Workspace(node.id.clone()),
            Self::AgentSession(node) => NodeId::AgentSession(node.id.clone()),
            Self::MuxSession(node) => NodeId::MuxSession(node.id.clone()),
            Self::Pin(node) => NodeId::Pin(node.id.clone()),
            Self::RuntimeProcess(node) => NodeId::RuntimeProcess(node.id.clone()),
            Self::Branch(node) => NodeId::Branch(node.id.clone()),
            Self::Fork(node) => NodeId::Fork(node.id.clone()),
            Self::ForgePr(node) => NodeId::ForgePr(node.id.clone()),
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RepoNode {
    pub id: RepoId,
    pub common_dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remotes: Vec<String>,
}

impl RepoNode {
    pub fn new(id: RepoId) -> Self {
        Self {
            common_dir: id.common_dir.clone(),
            id,
            source_paths: Vec::new(),
            remotes: Vec::new(),
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct CheckoutNode {
    pub id: CheckoutId,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<BranchId>,
}

impl CheckoutNode {
    pub fn new(id: CheckoutId, root: impl Into<String>) -> Self {
        Self {
            id,
            root: root.into(),
            git_dir: None,
            current_branch: None,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct WorkspaceNode {
    pub id: WorkspaceId,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct AgentSessionNode {
    pub id: AgentSessionId,
    pub harness_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Single-line preview of the session's most recent
    /// user/assistant text message, populated best-effort by the
    /// harness adapter. Capped at 200 chars with a trailing `…`
    /// when the source is longer. See ADR 0023.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message_preview: Option<String>,
    /// Best-effort timestamp for the session's latest observed activity
    /// (Unix epoch seconds). Harness adapters populate this from transcript
    /// file mtimes or provider state when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_epoch: Option<i64>,
    /// Harness-level session classification. Harnesses that spawn
    /// subordinate worker sessions (openCode subagents) set `Subagent` so
    /// the TUI and resolver can distinguish human-driven work from
    /// auxiliary traffic. Harnesses without subagent semantics leave this
    /// `None` (sparse default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_kind: Option<SessionKind>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct MuxSessionNode {
    pub id: MuxSessionId,
    pub backend: String,
    /// The post-backend portion of the mux's identifier. For
    /// default-socket tmux sessions this is just the bare session
    /// name (e.g. `editor`). For non-default-socket sessions
    /// (when discovery for them lands per H-PIN-F-001) it would
    /// be `<socket>:<name>` (e.g. `scratch:editor`). The fully-
    /// prefixed form `<backend>:<native_id>` lives on
    /// [`MuxSessionId.native_id`] — same field name on the id
    /// type, but the id holds the prefixed form while this field
    /// drops the backend prefix.
    ///
    /// Resolver consumers (`resolve::pins::mux_index`, the pin
    /// lookup path) reconstruct the prefixed key by formatting
    /// `<backend>:<native_id>` so the bookkeeping stays
    /// consistent with `PinMux::native_id()`'s output.
    pub native_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_pane_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_pane_pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_pane_current_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_pane_start_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_attached: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_epoch: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_epoch: Option<i64>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RuntimeProcessNode {
    pub id: RuntimeProcessId,
    pub observation_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_pane_pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RuntimeProcessRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_epoch: Option<i64>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct BranchNode {
    pub id: BranchId,
    pub refname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ForkNode {
    pub id: ForkId,
    pub provider: String,
    pub provider_source_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ForgePrNode {
    pub id: ForgePrId,
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_epoch: Option<i64>,
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub is_draft: bool,
}

/// First-class graph representation of a session pin declaration.
///
/// The persisted TOML entry remains the source of truth; this node is
/// rebuilt from [`PinCandidate`] so graph consumers can address pins by
/// stable [`NodeId`] and inspect their store lineage, target mux, and
/// current binding without consulting a private sidecar.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinNode {
    pub id: PinId,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux: PinMuxRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_argv: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub provenance: Provenance,
    pub store_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<PinBinding>,
}

impl From<&PinCandidate> for PinNode {
    fn from(pin: &PinCandidate) -> Self {
        Self {
            id: PinId::new(pin.id.clone()),
            display_name: pin.display_name.clone(),
            harness: pin.harness.clone(),
            cwd: pin.cwd.clone(),
            mux: pin.mux.clone(),
            launch_argv: pin.launch_argv.clone(),
            reason: pin.reason.clone(),
            provenance: pin.provenance,
            store_path: pin.store_path.clone(),
            binding: pin.binding.clone(),
        }
    }
}

/// Harness-level classification for agent sessions. Harnesses that spawn
/// subordinate worker sessions (openCode `@explore` / `@general` subagents)
/// set `Subagent` so the TUI and resolver can distinguish human-driven work
/// from auxiliary traffic. Harnesses without subagent semantics leave this
/// `None` (sparse default).
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Human,
    Subagent,
}

/// Best-effort role classification for ephemeral runtime process
/// observations. The role is diagnostic and resolver-supporting; it
/// is not durable identity.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProcessRole {
    HumanAgent,
    Subagent,
    Background,
    Shell,
    Unknown,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    AssociatedWith,
    BelongsToRepo,
    CheckedOutBranch,
    WorkspaceContainsRepo,
    BranchHasForgePr,
    LinkedToMux,
    RootedIn,
    ForksWorkspace,
    ForksRepo,
    CreatedCheckout,
    ReferencedCheckout,
    ParentSession,
    ChildSession,
    CreatedBranch,
    AssociatedBranch,
    ParentFork,
    RootedAtPath,
    MuxContainsProcess,
    ProcessIdentifiesSession,
    ProcessCandidatesSession,
    PinTargetsMux,
    PinRealizedBySession,
}

impl RelationKind {
    /// Stable snake_case label used across CLI, TUI, and snapshot
    /// outputs. Matches the `serde(rename_all = "snake_case")` form.
    pub fn snake_case(&self) -> &'static str {
        match self {
            Self::AssociatedWith => "associated_with",
            Self::BelongsToRepo => "belongs_to_repo",
            Self::CheckedOutBranch => "checked_out_branch",
            Self::WorkspaceContainsRepo => "workspace_contains_repo",
            Self::BranchHasForgePr => "branch_has_forge_pr",
            Self::LinkedToMux => "linked_to_mux",
            Self::RootedIn => "rooted_in",
            Self::ForksWorkspace => "forks_workspace",
            Self::ForksRepo => "forks_repo",
            Self::CreatedCheckout => "created_checkout",
            Self::ReferencedCheckout => "referenced_checkout",
            Self::ParentSession => "parent_session",
            Self::ChildSession => "child_session",
            Self::CreatedBranch => "created_branch",
            Self::AssociatedBranch => "associated_branch",
            Self::ParentFork => "parent_fork",
            Self::RootedAtPath => "rooted_at_path",
            Self::MuxContainsProcess => "mux_contains_process",
            Self::ProcessIdentifiesSession => "process_identifies_session",
            Self::ProcessCandidatesSession => "process_candidates_session",
            Self::PinTargetsMux => "pin_targets_mux",
            Self::PinRealizedBySession => "pin_realized_by_session",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    LocalDeclared,
    LocalPin,
    GlobalDeclared,
    GlobalPin,
    StrongDiscovered,
    Discovered,
    Convention,
    Cached,
}

impl Provenance {
    /// Numeric precedence used for ranking competing candidate links.
    /// Higher wins.
    ///
    /// The local/global axis dominates the explicit/auto axis: a
    /// project-local pin beats a global declared link because project
    /// context is more specific than user-wide intent. Within a
    /// scope, explicit declared links (operator typed `conspectus
    /// declared create` or `pin bind`) beat pin auto-attribution, so
    /// `pin bind --to <session>` lands as `LocalDeclared` and
    /// authoritatively overrides the pin's `LocalPin` evidence per
    /// ADR 0057 §Resolver Binding Semantics step 5.
    pub fn precedence(self) -> u8 {
        match self {
            Self::LocalDeclared => 7,
            Self::LocalPin => 6,
            Self::GlobalDeclared => 5,
            Self::GlobalPin => 4,
            Self::StrongDiscovered => 3,
            Self::Discovered | Self::Convention => 2,
            Self::Cached => 1,
        }
    }

    /// Stable snake_case label used in operator-facing surfaces.
    pub fn snake_case(self) -> &'static str {
        match self {
            Self::LocalDeclared => "local_declared",
            Self::LocalPin => "local_pin",
            Self::GlobalDeclared => "global_declared",
            Self::GlobalPin => "global_pin",
            Self::StrongDiscovered => "strong_discovered",
            Self::Discovered => "discovered",
            Self::Convention => "convention",
            Self::Cached => "cached",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    /// Stable snake_case label used in operator-facing surfaces.
    pub fn snake_case(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkEndpoint {
    Node { id: NodeId },
    Unresolved { evidence: UnresolvedEndpoint },
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct UnresolvedEndpoint {
    pub node_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    #[rkyv(with = MetadataAsJson)]
    pub metadata: Metadata,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LinkState {
    Active,
    Ignored {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Overridden {
        by: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl LinkState {
    pub fn is_ignored(&self) -> bool {
        matches!(self, Self::Ignored { .. })
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct GraphLink {
    pub id: String,
    pub source: NodeId,
    pub target: LinkEndpoint,
    pub relation: RelationKind,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub freshness: Freshness,
    pub source_metadata: SourceMetadata,
    pub state: LinkState,
}

impl GraphLink {
    pub fn new(
        id: impl Into<String>,
        source: NodeId,
        target: LinkEndpoint,
        relation: RelationKind,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            target,
            relation,
            provenance,
            confidence: Confidence::Medium,
            freshness: Freshness::Unknown,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    pub fn target_node_id(&self) -> Option<&NodeId> {
        match &self.target {
            LinkEndpoint::Node { id } => Some(id),
            LinkEndpoint::Unresolved { .. } => None,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct SourceMetadata {
    pub adapter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    #[rkyv(with = MetadataAsJson)]
    pub fields: Metadata,
    /// Unix epoch (seconds) captured when the producing adapter ran
    /// against the live world. Feeds the per-provider TTL comparison
    /// in `P7-003` / `P7-006`. `None` when the adapter did not record
    /// a timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
}

/// Per-node producing-provider metadata (P7-002 / ADR 0037). The
/// sidecar lives on [`GraphSnapshot`] keyed by `NodeId` so node
/// structs themselves remain provider-agnostic and the producers
/// have a single place to record their origin alongside whatever
/// they were already going to push into the snapshot.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct NodeProvenance {
    /// Stable identifier of the producing discovery provider, e.g.
    /// `git`, `harness::claude_code`, `tmux`, `forge::github`,
    /// `atelier`, `pins`, `aliases`, `declared`, `cross_link`,
    /// `codex_log`, `hook_sidecar`, `agent_deck`, `workspace`.
    /// Convention: lowercase, `::`-separated when an area hosts
    /// multiple adapters; mirrors `SourceMetadata.adapter` on the
    /// link side. The canonical constants live in
    /// `crate::discovery::providers`.
    pub provider: String,
    /// Unix epoch (seconds) captured when the provider produced
    /// this node. `None` when the producer did not record a
    /// timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ResolvedRelationship {
    pub source: NodeId,
    /// Placeholder target that carries the would-have-been winner's
    /// neighbor for "no-winner" slots (ADR 0077). When
    /// `selected_link_id` is `Some`, this is the resolved target.
    /// When `selected_link_id` is `None`, the resolver could not
    /// pick — consumers MUST gate downstream use of `target` on
    /// `selected_link_id.is_some()` and treat it as a placeholder
    /// otherwise.
    pub target: NodeId,
    pub relation: RelationKind,
    /// The link id of the resolver's chosen winner, when there is
    /// one. `None` when the resolver explicitly cannot pick — at
    /// time of writing the only producer is
    /// `suppress_ambiguous_cwd_mux_links`, which fires when a mux
    /// is claimed by multiple sessions via cwd evidence alone.
    /// Rust consumers pattern-match on `Option` before reading the slot
    /// as "the answer." See ADR 0077.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_link_id: Option<String>,
    /// Candidates that competed for this slot. When
    /// `selected_link_id` is `Some`, this is the rejected losers.
    /// When `selected_link_id` is `None`, this is *every*
    /// candidate that was considered — including what would have
    /// been the arbitrary tiebreak winner — so the Other-zone
    /// renderer and the H-UI-007 ambiguity signal can both walk
    /// the full candidate set without a parallel inference path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competing_link_ids: Vec<String>,
    /// Optional resolver score breakdown. Populated by
    /// `resolve::explain_resolved_relationships` for explainer
    /// surfaces; omitted from default graph JSON so the baseline
    /// machine-readable shape remains compact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ResolutionExplanation>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ResolutionExplanation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<CandidateScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competing: Vec<CandidateScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisive_axis: Option<String>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct CandidateScore {
    pub link_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<ScoreAxis>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ScoreAxis {
    pub name: String,
    pub value: String,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnostic {
    UnresolvedEndpoint {
        link_id: String,
        relation: RelationKind,
    },
    Config {
        path: String,
        message: String,
    },
    Conflict {
        source: NodeId,
        relation: RelationKind,
        selected_link_id: String,
        competing_link_ids: Vec<String>,
    },
    /// ADR 0057. No live mux matches the pin's `mux.native_id()`.
    /// Per ADR 0058 Q5, `last_session` carries the most recent
    /// recorded binding when the pin-bindings sidecar (ADR 0058) has
    /// one — so the launch path can advertise "Enter to resume X"
    /// instead of a generic "Enter to launch" when continuity is
    /// available. `None` means the sidecar was absent, unreadable,
    /// or the resolver ran without consulting one (the default in
    /// scenario TUIs).
    PinUnbound {
        pin_id: String,
        expected_mux_native_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_session: Option<PinLastSession>,
    },
    /// ADR 0057. Mux exists but no harness session matching
    /// `pin.harness` is attributed to it.
    PinStaleMux {
        pin_id: String,
        mux: MuxSessionId,
    },
    /// ADR 0057. Multiple harness sessions are attributed to the
    /// bound mux for this pin's harness. The resolver picks
    /// `chosen` per its existing ranking; `competing` lists the
    /// runners-up so the operator can override with
    /// `conspectus pin bind --to <session-id>`.
    PinAmbiguous {
        pin_id: String,
        chosen: AgentSessionId,
        competing: Vec<AgentSessionId>,
    },
    /// ADR 0057. Bound session's first-observed cwd diverges from
    /// the pin's declared cwd. Binding still holds; this is advisory.
    PinDrift {
        pin_id: String,
        declared_cwd: String,
        observed_cwd: String,
    },
}

/// ADR 0058 §Q5: the recorded last-bound session a `PinUnbound`
/// diagnostic optionally carries. Drives the launch path's
/// "Enter to resume X" hint surface.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinLastSession {
    pub session_id: String,
    pub observed_epoch: i64,
}

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub candidate_links: Vec<GraphLink>,
    pub resolved_relationships: Vec<ResolvedRelationship>,
    pub diagnostics: Vec<Diagnostic>,
    /// Operator-chosen display names per ADR 0029. Carried alongside
    /// the graph but kept out of the serialized snapshot so the JSON
    /// shape stays a pure view of discovered state. Empty for callers
    /// that don't load aliases.
    #[serde(skip, default)]
    pub aliases: crate::aliases::AliasOverlay,
    /// Session pin declarations per ADR 0057. Loaded from
    /// `[[pins.entries]]` TOML by `discovery::pins`; the resolver
    /// (H-PIN-004) consumes this to synthesize bound-state
    /// `LinkedToMux` candidates with `LocalPin`/`GlobalPin`
    /// provenance. Pins remain in this sidecar even after binding so
    /// row builders can render unbound pins as first-class rows
    /// without walking `candidate_links`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<PinCandidate>,
    /// Per-node producing-provider metadata (P7-002 / ADR 0037).
    /// Empty for snapshots whose producers have not been
    /// instrumented yet; the loader falls back to the schema's
    /// `'unknown'` / `0` defaults for nodes without an entry. See
    /// [`NodeProvenance`].
    ///
    /// Serializes as a JSON array of `{node_id, provider,
    /// freshness_epoch}` entries (via [`node_provenance_serde`])
    /// because JSON object keys must be strings and `NodeId` is a
    /// structured type. The in-memory shape stays a `BTreeMap` so
    /// loader lookups are O(log n) and the per-node iteration order
    /// is deterministic.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "node_provenance_serde"
    )]
    pub node_provenance: BTreeMap<NodeId, NodeProvenance>,
}

/// Round-trip helper for the `GraphSnapshot::node_provenance`
/// sidecar. The map serializes as an ordered list of
/// `NodeProvenanceEntry` records so the JSON payload remains
/// hand-inspectable and tooling-friendly; the in-memory shape stays
/// `BTreeMap<NodeId, NodeProvenance>` for fast lookups.
mod node_provenance_serde {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{NodeId, NodeProvenance};

    #[derive(Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    struct Entry {
        node_id: NodeId,
        #[serde(flatten)]
        provenance: NodeProvenance,
    }

    pub fn serialize<S>(
        map: &BTreeMap<NodeId, NodeProvenance>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<Entry> = map
            .iter()
            .map(|(node_id, provenance)| Entry {
                node_id: node_id.clone(),
                provenance: provenance.clone(),
            })
            .collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<NodeId, NodeProvenance>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: Vec<Entry> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.node_id, entry.provenance))
            .collect())
    }
}

impl GraphSnapshot {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn canonicalize(&mut self) {
        self.nodes.sort_by_key(GraphNode::id);
        self.candidate_links.sort_by_key(stable_json_key);
        self.resolved_relationships.sort();
        self.diagnostics.sort();
        self.pins.sort_by(|a, b| {
            a.provenance
                .cmp(&b.provenance)
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.store_path.cmp(&b.store_path))
        });
    }

    /// Rebuild the first-class pin-node projection from the current
    /// pin sidecar. The sidecar remains the TOML-backed source of
    /// truth; graph nodes are derived so consumers can select and link
    /// pins like every other node kind.
    pub fn sync_pin_nodes(&mut self) {
        self.nodes.retain(|node| !matches!(node, GraphNode::Pin(_)));
        self.node_provenance
            .retain(|id, _| !matches!(id, NodeId::Pin(_)));
        for pin in &self.pins {
            let node = PinNode::from(pin);
            let id = NodeId::Pin(node.id.clone());
            self.node_provenance.insert(
                id,
                NodeProvenance {
                    provider: "pins".to_string(),
                    freshness_epoch: None,
                },
            );
            self.nodes.push(GraphNode::Pin(node));
        }
    }

    /// Remove every node, candidate link, and provenance entry
    /// belonging to `provider`. Used by the warm-start path
    /// (P7-003 phase 3) to evict a stale provider's slice before
    /// re-running it, and by the eventual `conspectus serve` tick
    /// to swap a single provider's contribution without rebuilding
    /// the rest of the graph (P7-005).
    ///
    /// Semantics:
    ///
    /// * Nodes are dropped iff their `node_provenance` entry's
    ///   provider equals `provider`. Nodes without a provenance
    ///   entry are kept (they pre-date instrumentation; eviction
    ///   stays conservative).
    /// * Candidate links are dropped iff their
    ///   `source_metadata.adapter` equals `provider`. Declared
    ///   links, cross-link inferences, and other providers' links
    ///   survive untouched.
    /// * The matching `node_provenance` entries are removed.
    /// * `resolved_relationships` are cleared because the resolver
    ///   runs against `candidate_links` and must re-derive after
    ///   the candidate set changes.
    /// * `aliases`, `pins`, and `diagnostics` are independent of
    ///   provider identity and survive intact.
    pub fn evict_provider(&mut self, provider: &str) {
        let mut evicted_ids: BTreeSet<NodeId> = BTreeSet::new();
        for (id, prov) in &self.node_provenance {
            if prov.provider == provider {
                evicted_ids.insert(id.clone());
            }
        }
        self.nodes.retain(|node| !evicted_ids.contains(&node.id()));
        self.candidate_links
            .retain(|link| link.source_metadata.adapter != provider);
        self.node_provenance
            .retain(|_, prov| prov.provider != provider);
        self.resolved_relationships.clear();
    }
}

/// Snapshot-resident pin record loaded from `[[pins.entries]]` TOML
/// per ADR 0057. Carries the declarative fields (ADR schema) plus the
/// loader-side provenance and store path. `binding` is populated by
/// the resolver pass ([`crate::resolve::pins`]) once mux lookup and
/// harness attribution have run; pre-resolve snapshots leave it as
/// `None`.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinCandidate {
    /// Pin id from the TOML entry; unique within its store.
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux: PinMuxRef,
    /// Per-pin launch argv override. `None` means the harness
    /// adapter's `launch_argv` default applies at launch time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_argv: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `Provenance::LocalPin` or `Provenance::GlobalPin`.
    pub provenance: Provenance,
    /// Absolute path of the TOML file the pin was loaded from.
    pub store_path: String,
    /// Resolver-populated binding state per ADR 0057. `None` after
    /// loader-only discovery; populated by the resolver pass.
    /// `Some(PinBinding::Unbound)` means the resolver ran and found
    /// no live mux matching `mux.native_id()`; `Some(PinBinding::Bound)`
    /// means the resolver found a live mux and attributed a harness
    /// session to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<PinBinding>,
}

/// Resolver-determined binding state for a [`PinCandidate`].
/// Ambiguous-binding cases resolve to `Bound` for the resolver's
/// preferred candidate and emit a parallel `Diagnostic::PinAmbiguous`
/// listing the competing candidates so the operator can override.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinBinding {
    /// The mux exists and a `pin.harness` session was attributed to
    /// it. The resolver synthesizes a `LinkedToMux` candidate carrying
    /// the pin's provenance (`LocalPin` / `GlobalPin`).
    Bound {
        mux: MuxSessionId,
        session: AgentSessionId,
    },
    /// The mux exists but no live `pin.harness` session is attributed
    /// to it. Operator action: relaunch the harness inside the
    /// existing mux.
    StaleMux { mux: MuxSessionId },
    /// No mux matching `pin.mux.native_id()` exists in the current
    /// snapshot. Operator action: launch the pin to create the mux.
    Unbound,
}

/// Mux backend coordinates carried inside [`PinCandidate`]. Mirrors
/// the schema in [`crate::pins::PinMux`] but lives in the model
/// crate so consumers can read it without depending on the
/// pin-schema module.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinMuxRef {
    pub backend: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_name: Option<String>,
}

impl PinMuxRef {
    /// Mux native id encoding per ADR 0057 (mirrors
    /// [`crate::pins::PinMux::native_id`] without taking a
    /// schema-module dependency).
    pub fn native_id(&self) -> String {
        match self.effective_socket() {
            None => format!("{}:{}", self.backend, self.name),
            Some(socket) => format!("{}:{}:{}", self.backend, socket, self.name),
        }
    }

    /// Collapses `None` and the `"default"` sentinel to `None`.
    pub fn effective_socket(&self) -> Option<&str> {
        match self.socket_name.as_deref() {
            None | Some("default") => None,
            Some(name) => Some(name),
        }
    }
}

fn stable_json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("graph model values serialize")
}

/// Maximum [`AgentSessionNode::last_message_preview`] length (chars,
/// not bytes) per ADR 0023. Adapters cap their extracted previews
/// at this length.
pub const LAST_MESSAGE_PREVIEW_CAP: usize = 200;

/// Normalize an extracted message into the shape adapters store in
/// [`AgentSessionNode::last_message_preview`] (ADR 0023): collapse
/// every run of whitespace to a single space, trim the ends, return
/// `None` for empty results, otherwise cap to
/// [`LAST_MESSAGE_PREVIEW_CAP`] chars with a trailing `…`.
pub fn normalize_last_message_preview(raw: &str) -> Option<String> {
    let mut collapsed = String::with_capacity(raw.len().min(LAST_MESSAGE_PREVIEW_CAP * 4));
    let mut last_was_space = true; // leading-trim
    for ch in raw.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                collapsed.push(' ');
                last_was_space = true;
            }
        } else {
            collapsed.push(ch);
            last_was_space = false;
        }
    }
    // Trim trailing space that the collapse may have left behind.
    if collapsed.ends_with(' ') {
        collapsed.pop();
    }
    if collapsed.is_empty() {
        return None;
    }
    let char_count = collapsed.chars().count();
    if char_count <= LAST_MESSAGE_PREVIEW_CAP {
        return Some(collapsed);
    }
    // Take the first cap-1 chars, append the ellipsis. Char-based
    // iteration so we don't slice mid-grapheme on non-ASCII content.
    let truncated: String = collapsed
        .chars()
        .take(LAST_MESSAGE_PREVIEW_CAP - 1)
        .collect();
    Some(format!("{truncated}…"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_with_stable_prefixes() {
        assert_eq!(
            RepoId::new("/tmp/repo/.git").to_string(),
            "repo:/tmp/repo/.git"
        );
        assert_eq!(
            AgentSessionId::new("codex", "/home/me/.codex", "session-1").to_string(),
            "agent_session:codex:/home/me/.codex:session-1"
        );
        assert_eq!(
            ForgePrId::new("github", "github.com", "openai", "conspectus", 42).to_string(),
            "forge_pr:github:github.com/openai/conspectus#42"
        );
        assert_eq!(
            RuntimeProcessId::new("tmux:0:12345").to_string(),
            "runtime_process:tmux:0:12345"
        );
    }

    #[test]
    fn node_id_round_trips_through_json() {
        let id = NodeId::Branch(BranchId::new(RepoId::new("/repo/.git"), "refs/heads/main"));

        let encoded = serde_json::to_string(&id).expect("serialize node id");
        let decoded: NodeId = serde_json::from_str(&encoded).expect("deserialize node id");

        assert_eq!(decoded, id);
    }

    #[test]
    fn checkout_helpers_serialize_checkout_wire_names() {
        let repo = RepoId::new("/repo/.git");
        let id = CheckoutId::new(repo.clone(), "/repo");
        let node_id = NodeId::checkout(repo, "/repo");
        let node = GraphNode::checkout(id.clone(), "/repo");

        assert_eq!(node_id.as_checkout(), Some(&id));
        assert_eq!(node.id(), NodeId::Checkout(id));

        let encoded_id = serde_json::to_value(&node_id).expect("serialize node id");
        let encoded_node = serde_json::to_value(&node).expect("serialize graph node");

        assert_eq!(encoded_id["type"], "checkout");
        assert_eq!(encoded_node["type"], "checkout");
    }

    #[test]
    fn relation_kind_serializes_as_snake_case() {
        let encoded =
            serde_json::to_string(&RelationKind::CreatedCheckout).expect("serialize relation kind");

        assert_eq!(encoded, r#""created_checkout""#);

        let encoded = serde_json::to_string(&RelationKind::MuxContainsProcess)
            .expect("serialize relation kind");
        assert_eq!(encoded, r#""mux_contains_process""#);
    }

    #[test]
    fn sparse_node_skips_empty_optional_fields() {
        let repo = GraphNode::Repo(RepoNode::new(RepoId::new("/workspace/repo/.git")));
        let encoded = serde_json::to_value(repo).expect("serialize repo node");

        assert!(encoded.get("source_paths").is_none());
        assert!(encoded.get("remotes").is_none());
    }

    #[test]
    fn graph_link_preserves_unresolved_endpoint_evidence() {
        let link = GraphLink::new(
            "lineage-1",
            NodeId::Fork(ForkId::new("atelier/fork-1")),
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some("codex".to_string()),
                    native_id: Some("pending-child".to_string()),
                    state_scope: None,
                    path: Some("/workspace/fork".to_string()),
                    metadata: Metadata::new(),
                },
            },
            RelationKind::ChildSession,
            Provenance::StrongDiscovered,
        );

        let encoded = serde_json::to_string(&link).expect("serialize graph link");
        let decoded: GraphLink = serde_json::from_str(&encoded).expect("deserialize graph link");

        assert_eq!(decoded, link);
        assert!(decoded.target_node_id().is_none());
    }

    #[test]
    fn graph_link_preserves_ignored_and_overridden_states() {
        let mut ignored = GraphLink::new(
            "ignored",
            NodeId::AgentSession(AgentSessionId::new("codex", "global", "a")),
            LinkEndpoint::Node {
                id: NodeId::MuxSession(MuxSessionId::new("tmux:1")),
            },
            RelationKind::LinkedToMux,
            Provenance::Convention,
        );
        ignored.state = LinkState::Ignored {
            reason: Some("user rejected match".to_string()),
        };

        let mut overridden = ignored.clone();
        overridden.id = "overridden".to_string();
        overridden.state = LinkState::Overridden {
            by: "local-declared-link".to_string(),
            reason: None,
        };

        assert!(ignored.state.is_ignored());
        assert!(
            serde_json::to_string(&overridden)
                .expect("serialize overridden link")
                .contains("overridden")
        );
    }

    #[test]
    fn normalize_last_message_preview_collapses_whitespace_and_trims() {
        assert_eq!(
            normalize_last_message_preview("  hello\n\tworld  "),
            Some("hello world".to_string())
        );
        assert_eq!(
            normalize_last_message_preview("multiple    spaces"),
            Some("multiple spaces".to_string())
        );
    }

    #[test]
    fn normalize_last_message_preview_returns_none_for_empty_inputs() {
        assert_eq!(normalize_last_message_preview(""), None);
        assert_eq!(normalize_last_message_preview("   "), None);
        assert_eq!(normalize_last_message_preview("\n\t \r"), None);
    }

    #[test]
    fn normalize_last_message_preview_caps_long_inputs_with_ellipsis() {
        let body = "a".repeat(LAST_MESSAGE_PREVIEW_CAP + 50);
        let preview = normalize_last_message_preview(&body).expect("non-empty");
        let chars: Vec<char> = preview.chars().collect();
        assert_eq!(chars.len(), LAST_MESSAGE_PREVIEW_CAP);
        assert_eq!(*chars.last().unwrap(), '…');
        // The body before the ellipsis is the first cap-1 chars of
        // the input (all `a`s here).
        assert!(chars[..chars.len() - 1].iter().all(|c| *c == 'a'));
    }

    #[test]
    fn normalize_last_message_preview_preserves_short_unicode() {
        let preview = normalize_last_message_preview("hi 👋 there").expect("non-empty");
        assert_eq!(preview, "hi 👋 there");
    }

    #[test]
    fn normalize_last_message_preview_caps_on_grapheme_boundary_not_byte() {
        // String of CJK characters (each is 3 bytes in UTF-8). Capping
        // by chars must not split bytes mid-codepoint.
        let body = "東".repeat(LAST_MESSAGE_PREVIEW_CAP + 5);
        let preview = normalize_last_message_preview(&body).expect("non-empty");
        // All chars are valid (no partial codepoints would mean
        // Rust would refuse to construct the String at all).
        assert_eq!(preview.chars().count(), LAST_MESSAGE_PREVIEW_CAP);
        assert!(preview.ends_with('…'));
    }

    /// Helper: build a candidate link tagged with `provider` so
    /// eviction-by-source_metadata tests stay readable.
    fn provider_link(id: &str, provider: &str) -> GraphLink {
        let source = NodeId::Repo(RepoId::new(format!("/{id}-src/.git")));
        let target = NodeId::Repo(RepoId::new(format!("/{id}-tgt/.git")));
        let mut link = GraphLink::new(
            id,
            source,
            LinkEndpoint::Node { id: target },
            RelationKind::BelongsToRepo,
            Provenance::StrongDiscovered,
        );
        link.source_metadata.adapter = provider.to_string();
        link
    }

    #[test]
    fn evict_provider_drops_only_matching_nodes_and_links() {
        // Two providers contribute nodes + links into one snapshot.
        // Evicting one leaves the other untouched.
        let git_repo = RepoNode::new(RepoId::new("/git-only/.git"));
        let git_id = NodeId::Repo(git_repo.id.clone());
        let workspace_id = WorkspaceId::new("/tmux-only");
        let tmux_workspace = WorkspaceNode {
            id: workspace_id.clone(),
            root: "/tmux-only".to_string(),
            provider: Some("atelier".to_string()),
            name: None,
        };
        let workspace_node_id = NodeId::Workspace(workspace_id.clone());

        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::Repo(git_repo));
        snap.nodes.push(GraphNode::Workspace(tmux_workspace));
        snap.candidate_links.push(provider_link("git-link", "git"));
        snap.candidate_links
            .push(provider_link("tmux-link", "tmux"));
        snap.node_provenance.insert(
            git_id.clone(),
            NodeProvenance {
                provider: "git".to_string(),
                freshness_epoch: Some(100),
            },
        );
        snap.node_provenance.insert(
            workspace_node_id.clone(),
            NodeProvenance {
                provider: "tmux".to_string(),
                freshness_epoch: Some(200),
            },
        );

        snap.evict_provider("git");

        assert_eq!(snap.nodes.len(), 1, "tmux node should survive");
        assert!(matches!(&snap.nodes[0], GraphNode::Workspace(w) if w.id == workspace_id));
        assert_eq!(snap.candidate_links.len(), 1);
        assert_eq!(snap.candidate_links[0].id, "tmux-link");
        assert!(!snap.node_provenance.contains_key(&git_id));
        assert!(snap.node_provenance.contains_key(&workspace_node_id));
    }

    #[test]
    fn evict_provider_is_a_noop_when_provider_has_no_slice() {
        // Eviction must be idempotent and gracefully handle keys
        // that simply don't appear in this snapshot (e.g. a forge
        // provider on a snapshot built with no GitHub repos).
        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))));
        snap.node_provenance.insert(
            NodeId::Repo(RepoId::new("/r/.git")),
            NodeProvenance {
                provider: "git".to_string(),
                freshness_epoch: Some(100),
            },
        );
        let before = snap.clone();
        snap.evict_provider("never-existed");
        // The only legitimate difference is `resolved_relationships`
        // being cleared; the before snapshot has none either so the
        // shapes match.
        assert_eq!(snap.nodes, before.nodes);
        assert_eq!(snap.candidate_links, before.candidate_links);
        assert_eq!(snap.node_provenance, before.node_provenance);
    }

    #[test]
    fn evict_provider_keeps_nodes_without_provenance_entries() {
        // Pre-instrumentation snapshots may contain nodes the
        // provenance sidecar doesn't know about. Eviction skips
        // those rather than dropping them — the conservative
        // default avoids losing data we can't attribute.
        let mut snap = GraphSnapshot::empty();
        let orphan_id = NodeId::Repo(RepoId::new("/orphan/.git"));
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/orphan/.git"))));
        // No node_provenance entry for the orphan.

        snap.evict_provider("git");

        assert!(
            snap.nodes.iter().any(|n| n.id() == orphan_id),
            "orphan node missing from provenance must survive eviction"
        );
    }

    #[test]
    fn evict_provider_evicts_candidate_links_even_when_no_node_matches() {
        // The candidate-links sweep stands on its own: a mutator
        // that only emitted links (e.g. cross_link) must have its
        // links pulled even though it emitted no nodes.
        let mut snap = GraphSnapshot::empty();
        snap.candidate_links
            .push(provider_link("xl-1", "cross_link"));
        snap.candidate_links
            .push(provider_link("xl-2", "cross_link"));
        snap.candidate_links.push(provider_link("keep", "git"));

        snap.evict_provider("cross_link");

        assert_eq!(snap.candidate_links.len(), 1);
        assert_eq!(snap.candidate_links[0].id, "keep");
    }

    #[test]
    fn evict_provider_clears_resolved_relationships_to_force_re_resolve() {
        let mut snap = GraphSnapshot::empty();
        let repo_id = RepoId::new("/r/.git");
        let checkout_id = CheckoutId::new(repo_id.clone(), "/r");
        snap.resolved_relationships.push(ResolvedRelationship {
            source: NodeId::Repo(repo_id),
            target: NodeId::Checkout(checkout_id),
            relation: RelationKind::CreatedCheckout,
            selected_link_id: Some("git-link".to_string()),
            competing_link_ids: Vec::new(),
            explanation: None,
        });

        snap.evict_provider("git");

        assert!(
            snap.resolved_relationships.is_empty(),
            "evict_provider must clear resolved relationships so the resolver re-runs"
        );
    }

    // ---- ADR 0083 rkyv archive round-trip coverage (P11-003) ----

    /// Construct a `GraphSnapshot` populated with one of every
    /// `NodeKind` plus link, resolved-relationship, diagnostic,
    /// alias, pin, and node-provenance entries. The fixture is the
    /// shared input for the rkyv archive round-trip tests below;
    /// putting it here (rather than building a fresh one per test)
    /// keeps the variant coverage in lockstep with what the format
    /// is expected to preserve.
    fn populated_snapshot_for_archive_tests() -> GraphSnapshot {
        let repo_id = RepoId::new("/r/.git");
        let checkout_id = CheckoutId::new(repo_id.clone(), "/r");
        let workspace_id = WorkspaceId::new("/ws");
        let agent_id = AgentSessionId::new("codex", "/h", "sess-1");
        let mux_id = MuxSessionId::new("tmux:editor");
        let runtime_id = RuntimeProcessId::new("proc-1");
        let branch_id = BranchId::new(repo_id.clone(), "refs/heads/main");
        let fork_id = ForkId::new("github:owner:repo");
        let pr_id = ForgePrId::new("github", "github.com", "owner", "repo", 1);

        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode::new(
            checkout_id.clone(),
            "/r",
        )));
        snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: "/ws".to_string(),
            provider: Some("agent-deck".to_string()),
            name: Some("ws".to_string()),
        }));
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: agent_id.clone(),
            harness_key: "codex".to_string(),
            cwd: Some("/r".to_string()),
            title: Some("hello".to_string()),
            last_message_preview: Some("hi".to_string()),
            last_active_epoch: Some(1),
            session_kind: Some(SessionKind::Human),
        }));
        snap.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            backend: "tmux".to_string(),
            native_id: "editor".to_string(),
            cwd: Some("/r".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: Some(true),
            activity_epoch: Some(2),
            created_epoch: Some(3),
        }));
        snap.nodes
            .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: runtime_id.clone(),
                observation_key: "proc-1".to_string(),
                pid: Some(42),
                parent_pid: None,
                root_pane_pid: None,
                command: Some("claude-code".to_string()),
                cwd: Some("/r".to_string()),
                harness_key: Some("codex".to_string()),
                role: Some(RuntimeProcessRole::HumanAgent),
                depth: Some(1),
                observed_epoch: Some(4),
            }));
        snap.nodes.push(GraphNode::Branch(BranchNode {
            id: branch_id.clone(),
            refname: "refs/heads/main".to_string(),
            current_commit: Some("abc".to_string()),
            upstream: Some("origin/main".to_string()),
        }));
        snap.nodes.push(GraphNode::Fork(ForkNode {
            id: fork_id.clone(),
            provider: "github".to_string(),
            provider_source_key: "github:owner:repo".to_string(),
            name: Some("repo".to_string()),
            scope: None,
            capabilities: vec!["read".to_string()],
        }));
        snap.nodes.push(GraphNode::ForgePr(ForgePrNode {
            id: pr_id.clone(),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
            state: Some("open".to_string()),
            url: Some("https://example".to_string()),
            updated_epoch: Some(5),
            is_draft: false,
        }));

        let mut fields = Metadata::new();
        fields.insert("s".to_string(), Value::String("hi".to_string()));
        fields.insert("n".to_string(), Value::from(42i64));
        fields.insert("b".to_string(), Value::Bool(true));
        fields.insert("arr".to_string(), serde_json::json!([1, 2, "three"]));
        fields.insert("obj".to_string(), serde_json::json!({"k": "v"}));
        fields.insert("z".to_string(), Value::Null);

        let mut link = GraphLink::new(
            "link-1",
            NodeId::AgentSession(agent_id.clone()),
            LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id.clone()),
            },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );
        link.source_metadata.adapter = "tmux".to_string();
        link.source_metadata.fields = fields.clone();
        link.source_metadata.freshness_epoch = Some(6);
        snap.candidate_links.push(link);

        let unresolved_link = GraphLink::new(
            "link-2",
            NodeId::AgentSession(agent_id.clone()),
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "mux".to_string(),
                    harness_key: Some("codex".to_string()),
                    native_id: Some("ghost".to_string()),
                    state_scope: None,
                    path: None,
                    metadata: fields.clone(),
                },
            },
            RelationKind::LinkedToMux,
            Provenance::Discovered,
        );
        snap.candidate_links.push(unresolved_link);

        snap.resolved_relationships.push(ResolvedRelationship {
            source: NodeId::AgentSession(agent_id.clone()),
            target: NodeId::MuxSession(mux_id.clone()),
            relation: RelationKind::LinkedToMux,
            selected_link_id: Some("link-1".to_string()),
            competing_link_ids: vec!["link-2".to_string()],
            explanation: None,
        });

        snap.diagnostics.push(Diagnostic::UnresolvedEndpoint {
            link_id: "link-2".to_string(),
            relation: RelationKind::LinkedToMux,
        });

        snap.aliases
            .insert(NodeId::AgentSession(agent_id.clone()), "Alpha".to_string());

        snap.pins.push(PinCandidate {
            id: "pin-1".to_string(),
            display_name: "Pin Alpha".to_string(),
            harness: "codex".to_string(),
            cwd: "/r".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "editor".to_string(),
                socket_name: None,
            },
            launch_argv: Some(vec!["codex".to_string()]),
            reason: Some("primary".to_string()),
            provenance: Provenance::LocalPin,
            store_path: "/r/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Bound {
                mux: mux_id.clone(),
                session: agent_id.clone(),
            }),
        });

        snap.node_provenance.insert(
            NodeId::Repo(repo_id),
            NodeProvenance {
                provider: "git".to_string(),
                freshness_epoch: Some(7),
            },
        );
        snap.node_provenance.insert(
            NodeId::AgentSession(agent_id),
            NodeProvenance {
                provider: "harness::codex".to_string(),
                freshness_epoch: Some(8),
            },
        );

        snap.sync_pin_nodes();
        snap.canonicalize();
        snap
    }

    #[test]
    fn graph_snapshot_rkyv_round_trip_preserves_every_field() {
        let snap = populated_snapshot_for_archive_tests();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snap).expect("archive snapshot");
        let mut decoded: GraphSnapshot =
            rkyv::from_bytes::<GraphSnapshot, rkyv::rancor::Error>(&bytes)
                .expect("deserialize snapshot");
        // `canonicalize` is idempotent on a canonical snapshot, so
        // calling it again is harmless and provides defense against
        // any future deserializer that returns an unsorted shape.
        decoded.canonicalize();
        assert_eq!(decoded, snap);
    }

    /// Regression net for the `ValueAsJson` adapter in
    /// `src/model/rkyv_adapters.rs`. Covers every `serde_json::Value`
    /// variant (String, Number, Bool, Array, Object, Null) through a
    /// full archive → deserialize cycle so a future adapter regression
    /// surfaces in CI rather than at a consumer site.
    #[test]
    fn metadata_with_every_value_variant_round_trips() {
        let mut fields = Metadata::new();
        fields.insert("s".to_string(), Value::String("hi".to_string()));
        fields.insert("i".to_string(), Value::from(42i64));
        fields.insert("f".to_string(), Value::from(2.5f64));
        fields.insert("b".to_string(), Value::Bool(true));
        fields.insert("z".to_string(), Value::Null);
        fields.insert("arr".to_string(), serde_json::json!([1, "two", false]));
        fields.insert(
            "obj".to_string(),
            serde_json::json!({"nested": {"k": [1, 2]}}),
        );

        let original = SourceMetadata {
            adapter: "test".to_string(),
            evidence: Some("ev".to_string()),
            fields,
            freshness_epoch: Some(99),
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&original).expect("archive metadata");
        let decoded: SourceMetadata =
            rkyv::from_bytes::<SourceMetadata, rkyv::rancor::Error>(&bytes)
                .expect("deserialize metadata");

        assert_eq!(decoded, original);
    }

    /// Every `NodeId` variant must round-trip through the archive
    /// since `NodeId` is the key of `GraphSnapshot::node_provenance`
    /// and `AliasOverlay::entries`. If a future variant adds a
    /// payload type whose archive impl is missing, this test catches
    /// it before any consumer hits the failure.
    #[test]
    fn every_node_id_variant_archives_and_round_trips() {
        let repo = RepoId::new("/r/.git");
        let variants = [
            NodeId::Repo(repo.clone()),
            NodeId::Checkout(CheckoutId::new(repo.clone(), "/r")),
            NodeId::Workspace(WorkspaceId::new("/ws")),
            NodeId::AgentSession(AgentSessionId::new("codex", "/h", "sess")),
            NodeId::MuxSession(MuxSessionId::new("tmux:editor")),
            NodeId::Pin(PinId::new("pin-1")),
            NodeId::RuntimeProcess(RuntimeProcessId::new("proc-1")),
            NodeId::Branch(BranchId::new(repo.clone(), "refs/heads/main")),
            NodeId::Fork(ForkId::new("github:owner:repo")),
            NodeId::ForgePr(ForgePrId::new("github", "github.com", "o", "r", 1)),
        ];
        for id in &variants {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(id)
                .unwrap_or_else(|e| panic!("archive {id:?}: {e}"));
            let decoded: NodeId = rkyv::from_bytes::<NodeId, rkyv::rancor::Error>(&bytes)
                .unwrap_or_else(|e| panic!("deserialize {id:?}: {e}"));
            assert_eq!(&decoded, id);
        }
    }
}
