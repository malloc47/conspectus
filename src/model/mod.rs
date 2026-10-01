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
    /// Whether this node has `id`, without building an owned [`NodeId`]
    /// the way [`Self::id`] does.
    pub fn has_id(&self, id: &NodeId) -> bool {
        match (self, id) {
            (Self::Repo(node), NodeId::Repo(id)) => node.id == *id,
            (Self::Checkout(node), NodeId::Checkout(id)) => node.id == *id,
            (Self::Workspace(node), NodeId::Workspace(id)) => node.id == *id,
            (Self::AgentSession(node), NodeId::AgentSession(id)) => node.id == *id,
            (Self::MuxSession(node), NodeId::MuxSession(id)) => node.id == *id,
            (Self::Pin(node), NodeId::Pin(id)) => node.id == *id,
            (Self::RuntimeProcess(node), NodeId::RuntimeProcess(id)) => node.id == *id,
            (Self::Branch(node), NodeId::Branch(id)) => node.id == *id,
            (Self::Fork(node), NodeId::Fork(id)) => node.id == *id,
            (Self::ForgePr(node), NodeId::ForgePr(id)) => node.id == *id,
            _ => false,
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
    /// Git-worktree facts for this checkout. `None` means
    /// the checkout was not produced by worktree enumeration (e.g. an
    /// Atelier-declared checkout, or a graph built before worktree
    /// discovery); `Some` carries the linked-vs-primary kind and any
    /// lock / prune status from `git worktree list --porcelain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeMeta>,
}

impl CheckoutNode {
    pub fn new(id: CheckoutId, root: impl Into<String>) -> Self {
        Self {
            id,
            root: root.into(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }
    }

    /// Attach git-worktree metadata.
    pub fn with_worktree(mut self, meta: WorktreeMeta) -> Self {
        self.worktree = Some(meta);
        self
    }
}

/// Whether a checkout is a repo's primary working tree or a linked
/// worktree sharing its `.git`. The primary worktree is the
/// one whose `git_dir` equals the repo common dir; linked worktrees
/// live under `.git/worktrees/<name>`.
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
pub enum WorktreeKind {
    /// The repo's primary working tree.
    Primary,
    /// A linked worktree (`git worktree add`) sharing the repo's `.git`.
    Linked,
}

/// Git-worktree facts for a [`CheckoutNode`], sourced from
/// `git worktree list --porcelain`. `locked` / `prunable` are `Some`
/// when git reports that status; the inner string is git's reason,
/// which may be empty when git gives none.
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
pub struct WorktreeMeta {
    pub kind: WorktreeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prunable: Option<String>,
}

impl WorktreeMeta {
    /// A linked worktree with no lock / prune status.
    pub fn linked() -> Self {
        Self {
            kind: WorktreeKind::Linked,
            locked: None,
            prunable: None,
        }
    }

    /// A repo's primary working tree.
    pub fn primary() -> Self {
        Self {
            kind: WorktreeKind::Primary,
            locked: None,
            prunable: None,
        }
    }

    pub fn is_linked(&self) -> bool {
        matches!(self.kind, WorktreeKind::Linked)
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

// Builder helpers on `AgentSessionNode` mirror
// `RepoNode::new` / `with_*`. Struct literals elsewhere move to
// them as files are touched.
impl AgentSessionNode {
    /// Minimal ctor. All optional fields default to `None`.
    pub fn new(id: AgentSessionId, harness_key: impl Into<String>) -> Self {
        Self {
            id,
            harness_key: harness_key.into(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_last_message_preview(mut self, preview: impl Into<String>) -> Self {
        self.last_message_preview = Some(preview.into());
        self
    }

    pub fn with_last_active_epoch(mut self, epoch: i64) -> Self {
        self.last_active_epoch = Some(epoch);
        self
    }

    pub fn with_session_kind(mut self, kind: SessionKind) -> Self {
        self.session_kind = Some(kind);
        self
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
pub struct MuxSessionNode {
    pub id: MuxSessionId,
    pub backend: String,
    /// The post-backend portion of the mux's identifier. For
    /// default-socket tmux sessions this is just the bare session
    /// name (e.g. `editor`). For non-default-socket sessions
    /// (once discovery covers them) it would be `<socket>:<name>`
    /// (e.g. `scratch:editor`). The fully-
    /// prefixed form `<backend>:<native_id>` lives on
    /// `MuxSessionId.native_id` — same field name on the id
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
    /// Epoch of the most recent client attach to this mux session
    /// (`tmux #{session_last_attached}`). Distinct from
    /// `activity_epoch` (any pane activity) and `created_epoch`
    /// (session birth): this tracks when the operator last *looked
    /// at* the session, which drives the "last attached" mux recency
    /// sort basis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attached_epoch: Option<i64>,
}

// Builder helpers on `MuxSessionNode` mirror
// `RepoNode::new` / `with_*`.
impl MuxSessionNode {
    /// Minimal ctor. All optional fields default to `None`.
    pub fn new(id: MuxSessionId, backend: impl Into<String>, native_id: impl Into<String>) -> Self {
        Self {
            id,
            backend: backend.into(),
            native_id: native_id.into(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
            last_attached_epoch: None,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_active_pane_command(mut self, command: impl Into<String>) -> Self {
        self.active_pane_command = Some(command.into());
        self
    }

    pub fn with_active_pane_pid(mut self, pid: i64) -> Self {
        self.active_pane_pid = Some(pid);
        self
    }

    pub fn with_active_pane_current_path(mut self, path: impl Into<String>) -> Self {
        self.active_pane_current_path = Some(path.into());
        self
    }

    pub fn with_active_pane_start_command(mut self, command: impl Into<String>) -> Self {
        self.active_pane_start_command = Some(command.into());
        self
    }

    pub fn with_client_attached(mut self, attached: bool) -> Self {
        self.client_attached = Some(attached);
        self
    }

    pub fn with_activity_epoch(mut self, epoch: i64) -> Self {
        self.activity_epoch = Some(epoch);
        self
    }

    pub fn with_created_epoch(mut self, epoch: i64) -> Self {
        self.created_epoch = Some(epoch);
        self
    }

    pub fn with_last_attached_epoch(mut self, epoch: i64) -> Self {
        self.last_attached_epoch = Some(epoch);
        self
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

    /// Inverse of [`Self::snake_case`]. Parses the
    /// stable snake_case label back to its `RelationKind`.
    /// Returns `Err` with an operator-friendly message for
    /// unknown labels. Callers (CLI, declared parser, table
    /// renderer) all consult this so an added variant is a
    /// single change instead of parallel updates in three
    /// files.
    pub fn from_snake_case(raw: &str) -> Result<Self, String> {
        match raw {
            "associated_with" => Ok(Self::AssociatedWith),
            "belongs_to_repo" => Ok(Self::BelongsToRepo),
            "checked_out_branch" => Ok(Self::CheckedOutBranch),
            "workspace_contains_repo" => Ok(Self::WorkspaceContainsRepo),
            "branch_has_forge_pr" => Ok(Self::BranchHasForgePr),
            "linked_to_mux" => Ok(Self::LinkedToMux),
            "rooted_in" => Ok(Self::RootedIn),
            "forks_workspace" => Ok(Self::ForksWorkspace),
            "forks_repo" => Ok(Self::ForksRepo),
            "created_checkout" => Ok(Self::CreatedCheckout),
            "referenced_checkout" => Ok(Self::ReferencedCheckout),
            "parent_session" => Ok(Self::ParentSession),
            "child_session" => Ok(Self::ChildSession),
            "created_branch" => Ok(Self::CreatedBranch),
            "associated_branch" => Ok(Self::AssociatedBranch),
            "parent_fork" => Ok(Self::ParentFork),
            "rooted_at_path" => Ok(Self::RootedAtPath),
            "mux_contains_process" => Ok(Self::MuxContainsProcess),
            "process_identifies_session" => Ok(Self::ProcessIdentifiesSession),
            "process_candidates_session" => Ok(Self::ProcessCandidatesSession),
            "pin_targets_mux" => Ok(Self::PinTargetsMux),
            "pin_realized_by_session" => Ok(Self::PinRealizedBySession),
            _ => Err(format!(
                "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
            )),
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
    /// Keys populated in this map come from
    /// [`source_field`] constants. Producers stamp
    /// `Metadata::insert(source_field::MATCH_KIND, …)`;
    /// consumers read via
    /// `metadata.get(source_field::MATCH_KIND)`. New keys
    /// gain a constant so producer/consumer sides can't
    /// silently drift.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    #[rkyv(with = MetadataAsJson)]
    pub fields: Metadata,
    /// Unix epoch (seconds) captured when the producing adapter ran
    /// against the live world. Feeds the per-provider TTL comparison
    /// in the warm-start gate. `None` when the adapter did not record
    /// a timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
}

impl SourceMetadata {
    /// The link's [`MatchKind`], read from the
    /// [`source_field::MATCH_KIND`] field or, failing that, from
    /// `evidence`.
    pub fn match_kind(&self) -> Option<MatchKind> {
        self.fields
            .get(source_field::MATCH_KIND)
            .and_then(Value::as_str)
            .or(self.evidence.as_deref())
            .and_then(MatchKind::from_snake_case)
    }
}

/// Canonical field-name constants for
/// [`SourceMetadata::fields`]. Every producer that stamps a
/// value into the map and every consumer that reads one back
/// should reference the constants here so a typo or rename
/// surfaces at compile time instead of silently dropping the
/// evidence.
///
/// Missing constants can be added in the same commit that
/// introduces the field — the compile-time drift check is
/// the `every_source_field_constant_matches_its_string_literal`
/// unit test in the model module.
pub mod source_field {
    /// Resolver-visible match kind for mux + PR + session ↔
    /// process links. Populated by discovery adapters (hook
    /// sidecar, cross_link, codex_log) and read by the
    /// resolver's per-relation scoring.
    pub const MATCH_KIND: &str = "match_kind";
    /// Millisecond activity epoch used as a per-mux tie-break
    /// axis by the resolver.
    pub const MUX_ACTIVITY_EPOCH: &str = "mux_activity_epoch";
    /// Second-precision epoch used by the forge PR scoring.
    pub const UPDATED_EPOCH: &str = "updated_epoch";
    /// Filesystem root associated with a fork lineage link.
    pub const FORK_ROOT: &str = "fork_root";
    /// `"fork"` | `"spawn"` (etc) lineage classifier stamped
    /// by harness adapters onto `ParentSession` /
    /// `ChildSession` links.
    pub const LINEAGE_KIND: &str = "lineage_kind";
    /// Forge PR state (`"open"` / `"closed"` / `"merged"`)
    /// stamped by the GitHub adapter and consumed by the PR
    /// comparator.
    pub const STATE: &str = "state";
    /// Forge PR draft flag stamped by the GitHub adapter.
    pub const IS_DRAFT: &str = "is_draft";
    /// Canonical filesystem-facing name atelier / agent-deck /
    /// generic workspace producers stamp onto their
    /// `WorkspaceContainsRepo` evidence so the detail-panel
    /// member row shows the workspace-visible label.
    pub const LOGICAL_PATH: &str = "logical_path";
    /// Free-form evidence provenance scope stamp (per-adapter
    /// substring; used by cross_link tie-breakers).
    pub const SCOPE: &str = "scope";
}

/// The `GraphNode` variants as a flat enum, so call sites can name a
/// node's kind without matching on the whole node. Converts from both
/// [`GraphNode`] and [`NodeId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    Workspace,
    Repo,
    Checkout,
    AgentSession,
    MuxSession,
    Pin,
    RuntimeProcess,
    Branch,
    Fork,
    ForgePr,
}

impl NodeKind {
    /// Every kind, in canonical display order. Used by the catalog
    /// test and by config-loader iteration so new variants are
    /// caught at compile time via exhaustive matches.
    pub const ALL: [NodeKind; 10] = [
        NodeKind::Workspace,
        NodeKind::Repo,
        NodeKind::Checkout,
        NodeKind::AgentSession,
        NodeKind::MuxSession,
        NodeKind::Pin,
        NodeKind::RuntimeProcess,
        NodeKind::Branch,
        NodeKind::Fork,
        NodeKind::ForgePr,
    ];

    /// Stable snake-case tag for the kind, as used in unresolved
    /// endpoints' `node_type` and in CLI output.
    pub fn snake_case(self) -> &'static str {
        match self {
            NodeKind::Workspace => "workspace",
            NodeKind::Repo => "repo",
            NodeKind::Checkout => "checkout",
            NodeKind::AgentSession => "agent_session",
            NodeKind::MuxSession => "mux_session",
            NodeKind::Pin => "pin",
            NodeKind::RuntimeProcess => "runtime_process",
            NodeKind::Branch => "branch",
            NodeKind::Fork => "fork",
            NodeKind::ForgePr => "forge_pr",
        }
    }

    /// Display-order ordinal for sorting (ADR 0074 §4: detail-pane
    /// `Related entities` rows sort by kind first). Matches
    /// [`Self::ALL`] order so the visual scan reads
    /// `▦ ◆ ◇ ● ▣ ⚙ ⎇ ⑂ ⇄` top-to-bottom.
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|k| *k == self)
            .unwrap_or(usize::MAX)
    }

    /// Inverse of [`Self::snake_case`]. `None` for tags that don't
    /// name a node kind, such as an unresolved endpoint's `"path"`.
    pub fn from_snake_case(tag: &str) -> Option<NodeKind> {
        NodeKind::ALL.into_iter().find(|k| k.snake_case() == tag)
    }
}

impl From<&GraphNode> for NodeKind {
    fn from(node: &GraphNode) -> Self {
        match node {
            GraphNode::Workspace(_) => NodeKind::Workspace,
            GraphNode::Repo(_) => NodeKind::Repo,
            GraphNode::Checkout(_) => NodeKind::Checkout,
            GraphNode::AgentSession(_) => NodeKind::AgentSession,
            GraphNode::MuxSession(_) => NodeKind::MuxSession,
            GraphNode::Pin(_) => NodeKind::Pin,
            GraphNode::RuntimeProcess(_) => NodeKind::RuntimeProcess,
            GraphNode::Branch(_) => NodeKind::Branch,
            GraphNode::Fork(_) => NodeKind::Fork,
            GraphNode::ForgePr(_) => NodeKind::ForgePr,
        }
    }
}

impl From<&NodeId> for NodeKind {
    fn from(id: &NodeId) -> Self {
        match id {
            NodeId::Workspace(_) => NodeKind::Workspace,
            NodeId::Repo(_) => NodeKind::Repo,
            NodeId::Checkout(_) => NodeKind::Checkout,
            NodeId::AgentSession(_) => NodeKind::AgentSession,
            NodeId::MuxSession(_) => NodeKind::MuxSession,
            NodeId::Pin(_) => NodeKind::Pin,
            NodeId::RuntimeProcess(_) => NodeKind::RuntimeProcess,
            NodeId::Branch(_) => NodeKind::Branch,
            NodeId::Fork(_) => NodeKind::Fork,
            NodeId::ForgePr(_) => NodeKind::ForgePr,
        }
    }
}

/// How a discovery adapter matched the two ends of a mux-attribution
/// link. Stored as its snake_case string in
/// [`SourceMetadata::fields`] under [`source_field::MATCH_KIND`] and
/// usually repeated in [`SourceMetadata::evidence`]; the resolver
/// ranks candidates by it. Read it back with
/// [`SourceMetadata::match_kind`]. See `docs/mux-link-resolution.md`
/// for the ranking.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatchKind {
    /// The session's cwd equals the mux's cwd.
    ExactCwdMatch,
    /// One cwd is a prefix of the other.
    CwdPrefixMatch,
    /// The session file most recently written under the pane's
    /// harness state matches the session.
    SessionFileActivityMatch,
    /// The mux's active pane holds an open harness state file (session
    /// JSONL, rollout log, or OpenCode DB path).
    ActivePaneFdSessionMatch,
    /// The open state file agrees with a session key on the pane's
    /// start command.
    ActivePaneFdCommandSessionMatch,
    /// The pane's start command carries a session key argument (for
    /// example `--resume <uuid>`). Demoted by fresher hook and
    /// codex-log evidence (ADR 0028, ADR 0048).
    ActivePaneCommandSessionMatch,
    /// The active pane's process tree contains a harness process.
    ActivePaneProcessMatch,
    /// The process tree walk observed a harness process in the pane.
    ActivePaneProcessObservation,
    /// A hook sidecar record names the session (ADR 0028).
    HookSessionMatch,
    /// A hook sidecar record names the session's transcript path.
    HookSessionPathMatch,
    /// A hook sidecar record observed a harness process in the pane.
    HookProcessObservation,
    /// The Codex logs DB shows the pane's Codex process writing the
    /// session's thread (ADR 0048).
    CodexLogCurrentThreadMatch,
    /// The Codex logs DB binds a process to a thread (ADR 0048).
    CodexLogProcessThreadMatch,
    /// The Codex logs DB observed a Codex process in the pane.
    CodexLogProcessObservation,
    /// The resolver derived the link from a runtime process that
    /// identifies the session.
    RuntimeProcessIdentifiesSession,
    /// The resolver derived the link from a runtime process that is
    /// one of several candidates for the session.
    RuntimeProcessCandidatesSession,
}

impl MatchKind {
    pub const ALL: [Self; 16] = [
        Self::ExactCwdMatch,
        Self::CwdPrefixMatch,
        Self::SessionFileActivityMatch,
        Self::ActivePaneFdSessionMatch,
        Self::ActivePaneFdCommandSessionMatch,
        Self::ActivePaneCommandSessionMatch,
        Self::ActivePaneProcessMatch,
        Self::ActivePaneProcessObservation,
        Self::HookSessionMatch,
        Self::HookSessionPathMatch,
        Self::HookProcessObservation,
        Self::CodexLogCurrentThreadMatch,
        Self::CodexLogProcessThreadMatch,
        Self::CodexLogProcessObservation,
        Self::RuntimeProcessIdentifiesSession,
        Self::RuntimeProcessCandidatesSession,
    ];

    pub fn snake_case(self) -> &'static str {
        match self {
            Self::ExactCwdMatch => "exact_cwd_match",
            Self::CwdPrefixMatch => "cwd_prefix_match",
            Self::SessionFileActivityMatch => "session_file_activity_match",
            Self::ActivePaneFdSessionMatch => "active_pane_fd_session_match",
            Self::ActivePaneFdCommandSessionMatch => "active_pane_fd_command_session_match",
            Self::ActivePaneCommandSessionMatch => "active_pane_command_session_match",
            Self::ActivePaneProcessMatch => "active_pane_process_match",
            Self::ActivePaneProcessObservation => "active_pane_process_observation",
            Self::HookSessionMatch => "hook_session_match",
            Self::HookSessionPathMatch => "hook_session_path_match",
            Self::HookProcessObservation => "hook_process_observation",
            Self::CodexLogCurrentThreadMatch => "codex_log_current_thread_match",
            Self::CodexLogProcessThreadMatch => "codex_log_process_thread_match",
            Self::CodexLogProcessObservation => "codex_log_process_observation",
            Self::RuntimeProcessIdentifiesSession => "runtime_process_identifies_session",
            Self::RuntimeProcessCandidatesSession => "runtime_process_candidates_session",
        }
    }

    /// `None` for strings outside the vocabulary, such as free-form
    /// evidence text from other adapters.
    pub fn from_snake_case(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.snake_case() == raw)
    }
}

impl fmt::Display for MatchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.snake_case())
    }
}

/// Per-node producing-provider metadata (ADR 0037). The
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
    /// renderer and the ambiguity signal can both walk
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
    /// consumes this to synthesize bound-state
    /// `LinkedToMux` candidates with `LocalPin`/`GlobalPin`
    /// provenance. Pins remain in this sidecar even after binding so
    /// row builders can render unbound pins as first-class rows
    /// without walking `candidate_links`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<PinCandidate>,
    /// Per-node producing-provider metadata (ADR 0037).
    /// Empty for snapshots whose producers have not been
    /// instrumented yet; the loader falls back to the schema's
    /// `'unknown'` / `0` defaults for nodes without an entry. See
    /// [`NodeProvenance`].
    ///
    /// Serializes as a JSON array of `{node_id, provider,
    /// freshness_epoch}` entries (via `node_provenance_serde`)
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

/// Indexed view over a [`GraphSnapshot`] (ADR 0035 Stage 1).
/// Built once per snapshot publish so consumers don't linear-scan
/// `snapshot.nodes` / `snapshot.candidate_links` per render. Holds
/// the `id → &GraphNode` map, links by source and relation, by
/// relation, and by id, and per-session mux candidate counts.
///
/// Cheap to build: one pass over the nodes and two over the
/// candidate links.
#[derive(Debug)]
pub struct SnapshotIndex<'a> {
    /// Borrow of the source snapshot so links + resolved_relationships
    /// stay accessible through the same handle during migration.
    pub snapshot: &'a GraphSnapshot,
    id_to_node: BTreeMap<NodeId, &'a GraphNode>,
    /// Per-agent count of distinct active
    /// `LinkedToMux` mux targets, read through
    /// `index.mux_candidate_count(agent_node_id)`. Deduped by target
    /// mux id.
    agent_mux_candidate_counts: std::collections::HashMap<String, usize>,
    /// `(source, relation) → Vec<&GraphLink>`
    /// map. Consumers that today linear-scan `candidate_links`
    /// looking for a specific source-relation combination fold
    /// the filter through this map instead. Only holds
    /// `LinkState::Active` links because every existing hot
    /// consumer filters to active first.
    links_by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
    /// `relation → Vec<&GraphLink>` map.
    /// Consumers that scan every link with a specific relation
    /// kind (cross_link's parent-subagent override pass, the
    /// resolver's per-relation aggregation) fold their filter
    /// through this map. Only holds `LinkState::Active` links.
    links_by_relation: BTreeMap<RelationKind, Vec<&'a GraphLink>>,
    /// `link_id → &GraphLink` map. Consumers
    /// that today `.find(|link| link.id == some_id)` collapse
    /// to O(log n) lookup. Includes links in every state so
    /// consumers can inspect superseded / dead links too.
    links_by_id: BTreeMap<String, &'a GraphLink>,
}

impl<'a> SnapshotIndex<'a> {
    /// Build the index by walking `snapshot.nodes` +
    /// `snapshot.candidate_links` once.
    pub fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut id_to_node = BTreeMap::new();
        for node in &snapshot.nodes {
            id_to_node.insert(node.id(), node);
        }

        // Precompute per-agent mux candidate
        // counts once so row builders don't re-scan per render.
        let mut per_agent: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, LinkState::Active) {
                continue;
            }
            if !matches!(link.relation, RelationKind::LinkedToMux) {
                continue;
            }
            let NodeId::AgentSession(_) = &link.source else {
                continue;
            };
            let LinkEndpoint::Node { id: target_id } = &link.target else {
                continue;
            };
            per_agent
                .entry(link.source.to_string())
                .or_default()
                .insert(target_id.to_string());
        }
        let agent_mux_candidate_counts = per_agent.into_iter().map(|(k, v)| (k, v.len())).collect();

        // candidate_links indices. One pass
        // populates both `links_by_source_relation` (wave 3),
        // `links_by_relation` (wave 6), and `links_by_id`
        // (wave 6).
        let mut links_by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>> =
            BTreeMap::new();
        let mut links_by_relation: BTreeMap<RelationKind, Vec<&'a GraphLink>> = BTreeMap::new();
        let mut links_by_id: BTreeMap<String, &'a GraphLink> = BTreeMap::new();
        for link in &snapshot.candidate_links {
            // Wave 6: `links_by_id` covers every state so
            // consumers can inspect superseded links too.
            links_by_id.insert(link.id.clone(), link);
            if !matches!(link.state, LinkState::Active) {
                continue;
            }
            links_by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
            links_by_relation
                .entry(link.relation.clone())
                .or_default()
                .push(link);
        }

        Self {
            snapshot,
            id_to_node,
            agent_mux_candidate_counts,
            links_by_source_relation,
            links_by_relation,
            links_by_id,
        }
    }

    /// Look up a node by id. `None` for ids that aren't in
    /// this snapshot.
    pub fn node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.id_to_node.get(id).copied()
    }

    /// Number of indexed nodes. Consistent with `snapshot.nodes.len()`
    /// unless the snapshot has duplicate ids (which is invalid; a
    /// wave-2 consistency test asserts this).
    pub fn node_count(&self) -> usize {
        self.id_to_node.len()
    }

    /// Per-agent `LinkedToMux` candidate count
    /// keyed by `NodeId::AgentSession(...).to_string()`.
    /// Retires `tui::rows::collect_agent_mux_candidate_counts`.
    pub fn agent_mux_candidate_counts(&self) -> &std::collections::HashMap<String, usize> {
        &self.agent_mux_candidate_counts
    }

    /// Active `(source, relation)` links.
    /// Returns an empty slice for unknown `(source, relation)`
    /// combinations. Consumers that today do
    /// `snapshot.candidate_links.iter().filter(|link| link.source
    /// == id && link.relation == RelationKind::Foo)` collapse to
    /// `index.links_for(&id, RelationKind::Foo)`.
    pub fn links_for(&self, source: &NodeId, relation: RelationKind) -> &[&GraphLink] {
        static EMPTY: &[&GraphLink] = &[];
        self.links_by_source_relation
            .get(&(source.clone(), relation))
            .map_or(EMPTY, Vec::as_slice)
    }

    /// Active links matching `relation`.
    /// Consumers that scan every link with a specific relation
    /// (`cross_link.rs` subagent override pass) collapse to
    /// `index.links_with_relation(RelationKind::LinkedToMux)`.
    /// Returns an empty slice for unknown relations.
    pub fn links_with_relation(&self, relation: RelationKind) -> &[&GraphLink] {
        static EMPTY: &[&GraphLink] = &[];
        self.links_by_relation
            .get(&relation)
            .map_or(EMPTY, Vec::as_slice)
    }

    /// Link by `link.id`. Includes every state
    /// so consumers can inspect superseded / dead links too.
    /// Consumers that today do
    /// `snapshot.candidate_links.iter().find(|l| l.id == id)`
    /// collapse to `index.link(id)`.
    pub fn link(&self, link_id: &str) -> Option<&GraphLink> {
        self.links_by_id.get(link_id).copied()
    }
}

impl GraphSnapshot {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Linear-scan lookup of a node by id. For repeated lookups over
    /// one snapshot, build a [`SnapshotIndex`] instead.
    pub fn find_node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.has_id(id))
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
    /// to evict a stale provider's slice before
    /// re-running it, and by the eventual `conspectus serve` tick
    /// to swap a single provider's contribution without rebuilding
    /// the rest of the graph.
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
/// the schema in `crate::pins::PinMux` but lives in the model
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
    /// `crate::pins::PinMux::native_id` without taking a
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
#[path = "model_tests.rs"]
mod tests;
