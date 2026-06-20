//! Core graph model boundaries.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeId {
    Repo(RepoId),
    Checkout(CheckoutId),
    Workspace(WorkspaceId),
    AgentSession(AgentSessionId),
    MuxSession(MuxSessionId),
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
            Self::RuntimeProcess(id) => id.fmt(f),
            Self::Branch(id) => id.fmt(f),
            Self::Fork(id) => id.fmt(f),
            Self::ForgePr(id) => id.fmt(f),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNode {
    Repo(RepoNode),
    Checkout(CheckoutNode),
    Workspace(WorkspaceNode),
    AgentSession(AgentSessionNode),
    MuxSession(MuxSessionNode),
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
            Self::RuntimeProcess(node) => NodeId::RuntimeProcess(node.id.clone()),
            Self::Branch(node) => NodeId::Branch(node.id.clone()),
            Self::Fork(node) => NodeId::Fork(node.id.clone()),
            Self::ForgePr(node) => NodeId::ForgePr(node.id.clone()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub id: WorkspaceId,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BranchNode {
    pub id: BranchId,
    pub refname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

/// Harness-level classification for agent sessions. Harnesses that spawn
/// subordinate worker sessions (openCode `@explore` / `@general` subagents)
/// set `Subagent` so the TUI and resolver can distinguish human-driven work
/// from auxiliary traffic. Harnesses without subagent semantics leave this
/// `None` (sparse default).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Human,
    Subagent,
}

/// Best-effort role classification for ephemeral runtime process
/// observations. The role is diagnostic and resolver-supporting; it
/// is not durable identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProcessRole {
    HumanAgent,
    Subagent,
    Background,
    Shell,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkEndpoint {
    Node { id: NodeId },
    Unresolved { evidence: UnresolvedEndpoint },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub adapter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub fields: Metadata,
    /// Unix epoch (seconds) captured when the producing adapter ran
    /// against the live world. Feeds the SQL
    /// `candidate_links.discovery_freshness_epoch` column and the
    /// per-provider TTL comparison in `P7-003` / `P7-006`. `None`
    /// when the adapter did not record a timestamp (the loader then
    /// writes `0`, the schema default, so existing snapshots stay
    /// round-trippable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
}

/// Per-node producing-provider metadata (P7-002 / ADR 0037). The
/// sidecar lives on [`GraphSnapshot`] keyed by `NodeId` so node
/// structs themselves remain provider-agnostic and the producers
/// have a single place to record their origin alongside whatever
/// they were already going to push into the snapshot. The loader
/// fills the SQL `discovery_provider` / `discovery_freshness_epoch`
/// columns from this map; nodes without an entry land in SQL with
/// the schema's `'unknown'` / `0` defaults so legacy snapshots
/// (and any provider that has not yet been instrumented) keep
/// round-tripping.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
    /// SQL joins on `selected_link_id = candidate_links.link_id`
    /// naturally exclude these rows (NULL doesn't equal anything),
    /// which is the H-UI-008 tree-view invariant. Rust consumers
    /// pattern-match on `Option` before reading the slot as "the
    /// answer." See ADR 0077.
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
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PinLastSession {
    pub session_id: String,
    pub observed_epoch: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub node_provenance: BTreeMap<NodeId, NodeProvenance>,
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
}

/// Snapshot-resident pin record loaded from `[[pins.entries]]` TOML
/// per ADR 0057. Carries the declarative fields (ADR schema) plus the
/// loader-side provenance and store path. `binding` is populated by
/// the resolver pass ([`crate::resolve::pins`]) once mux lookup and
/// harness attribution have run; pre-resolve snapshots leave it as
/// `None`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
}
