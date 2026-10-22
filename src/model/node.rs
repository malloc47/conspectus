//! Graph nodes: one struct per node kind, the `GraphNode` sum over them, and `NodeKind`.

use super::*;

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
    /// The commit HEAD points at. History keeps it so a worktree whose
    /// branch was deleted can be recreated from it (ADR 0140).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_commit: Option<String>,
}

impl CheckoutNode {
    pub fn new(id: CheckoutId, root: impl Into<String>) -> Self {
        Self {
            id,
            root: root.into(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
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
    /// Consumers that need the prefixed key reconstruct it by
    /// formatting `<backend>:<native_id>`, which matches
    /// `MuxRecipe::native_id`.
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
    /// The non-default server the mux lives on (tmux `-L <socket>`);
    /// `None` for the default server. `native_id` carries it as a
    /// `<socket>:` prefix too, so qualified names stay unique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// The backend's own id for the session (tmux `#{session_id}`,
    /// e.g. `$3`). With `created_epoch` it identifies one incarnation,
    /// which survives an outside rename (ADR 0140).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_session_id: Option<String>,
}

// Builder helpers on `MuxSessionNode` mirror
// `RepoNode::new` / `with_*`.
impl MuxSessionNode {
    /// `<backend>:<native_id>`, the name a pin's `mux.native_id()`
    /// declares and an operator types to reference this mux. Pin
    /// binding keys muxes by it, so it must stay stable; use
    /// [`Self::display_label`] for what to show.
    pub fn qualified_name(&self) -> String {
        format!("{}:{}", self.backend, self.native_id)
    }

    /// The operator-facing label for this mux in tables, detail
    /// fields, and explorer rows. Today it is the qualified name.
    pub fn display_label(&self) -> String {
        self.qualified_name()
    }

    /// The bare session name the backend knows, without the socket
    /// prefix `native_id` carries for a non-default server.
    pub fn session_name(&self) -> &str {
        match self.socket.as_deref() {
            Some(socket) => self
                .native_id
                .strip_prefix(socket)
                .and_then(|rest| rest.strip_prefix(':'))
                .unwrap_or(&self.native_id),
            None => &self.native_id,
        }
    }

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
            socket: None,
            backend_session_id: None,
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
    /// The PR title on one line, as the forge listing returned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_epoch: Option<i64>,
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub is_draft: bool,
    /// The branch the PR merges from, as the forge names it
    /// (`feature/extra`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_ref: Option<String>,
}

/// A pin slot's graph identity (ADR 0084, ADR 0141): `pin:<id>`.
///
/// The slot in `pins.toml` stays the source of truth; this node is
/// rebuilt from it each resolve, with the slot's status and effective
/// recipe worked out against the graph and history, so Pinned rows have
/// a detail target and slots show up in search and exports.
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
    pub kind: PinKind,
    /// What lists show: the slot's label, else its id.
    pub label: String,
    /// `history:<id>`, a node id, or none for a planned slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The node the slot points at, live or gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node: Option<NodeId>,
    pub status: SlotStatus,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: SlotFields,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub snapshot: SlotFields,
    /// Each recipe field with the layer that supplied it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective: Vec<RecipeField>,
    /// Position among the slots.
    pub order: u32,
    /// Loaded from a legacy `[[pins.entries]]` table.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub legacy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<String>,
}

impl PinNode {
    /// A slot node for tests: `fields` become effective text fields
    /// sourced from the live graph.
    #[cfg(test)]
    pub fn for_test(
        id: &str,
        kind: PinKind,
        target_node: Option<NodeId>,
        status: SlotStatus,
        fields: &[(&str, &str)],
    ) -> Self {
        Self {
            id: PinId::new(id),
            kind,
            label: id.to_string(),
            target: target_node.as_ref().map(ToString::to_string),
            target_node,
            status,
            overrides: SlotFields::new(),
            snapshot: SlotFields::new(),
            effective: fields
                .iter()
                .map(|(name, value)| RecipeField {
                    name: (*name).to_string(),
                    value: FieldValue::Text((*value).to_string()),
                    source: FieldSource::Live,
                })
                .collect(),
            order: 0,
            legacy: false,
            pinned_at: None,
        }
    }

    /// The effective value of a recipe field.
    pub fn field(&self, name: &str) -> Option<&FieldValue> {
        self.effective
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.field(name).and_then(FieldValue::as_text)
    }

    pub fn list(&self, name: &str) -> Option<&[String]> {
        self.field(name).and_then(FieldValue::as_list)
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
