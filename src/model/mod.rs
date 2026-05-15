//! Core graph model boundaries.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Metadata = BTreeMap<String, Value>;

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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct WorktreeId {
    pub repo: RepoId,
    pub root: String,
}

impl WorktreeId {
    pub fn new(repo: RepoId, root: impl Into<String>) -> Self {
        Self {
            repo,
            root: root.into(),
        }
    }
}

impl fmt::Display for WorktreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "worktree:{}@{}", self.repo, self.root)
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
    Worktree(WorktreeId),
    Workspace(WorkspaceId),
    AgentSession(AgentSessionId),
    MuxSession(MuxSessionId),
    Branch(BranchId),
    Fork(ForkId),
    ForgePr(ForgePrId),
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repo(id) => id.fmt(f),
            Self::Worktree(id) => id.fmt(f),
            Self::Workspace(id) => id.fmt(f),
            Self::AgentSession(id) => id.fmt(f),
            Self::MuxSession(id) => id.fmt(f),
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
    Worktree(WorktreeNode),
    Workspace(WorkspaceNode),
    AgentSession(AgentSessionNode),
    MuxSession(MuxSessionNode),
    Branch(BranchNode),
    Fork(ForkNode),
    ForgePr(ForgePrNode),
}

impl GraphNode {
    pub fn id(&self) -> NodeId {
        match self {
            Self::Repo(node) => NodeId::Repo(node.id.clone()),
            Self::Worktree(node) => NodeId::Worktree(node.id.clone()),
            Self::Workspace(node) => NodeId::Workspace(node.id.clone()),
            Self::AgentSession(node) => NodeId::AgentSession(node.id.clone()),
            Self::MuxSession(node) => NodeId::MuxSession(node.id.clone()),
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
pub struct WorktreeNode {
    pub id: WorktreeId,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<BranchId>,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct MuxSessionNode {
    pub id: MuxSessionId,
    pub backend: String,
    pub native_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
    CreatedWorktree,
    ReferencedWorktree,
    ParentSession,
    ChildSession,
    CreatedBranch,
    AssociatedBranch,
    ParentFork,
    RootedAtPath,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    LocalDeclared,
    GlobalDeclared,
    StrongDiscovered,
    Discovered,
    Convention,
    Cached,
}

impl Provenance {
    pub fn precedence(self) -> u8 {
        match self {
            Self::LocalDeclared => 5,
            Self::GlobalDeclared => 4,
            Self::StrongDiscovered => 3,
            Self::Discovered | Self::Convention => 2,
            Self::Cached => 1,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ResolvedRelationship {
    pub source: NodeId,
    pub target: NodeId,
    pub relation: RelationKind,
    pub selected_link_id: String,
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
    Conflict {
        source: NodeId,
        relation: RelationKind,
        selected_link_id: String,
        competing_link_ids: Vec<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub candidate_links: Vec<GraphLink>,
    pub resolved_relationships: Vec<ResolvedRelationship>,
    pub diagnostics: Vec<Diagnostic>,
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
    }
}

fn stable_json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("graph model values serialize")
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
    }

    #[test]
    fn node_id_round_trips_through_json() {
        let id = NodeId::Branch(BranchId::new(RepoId::new("/repo/.git"), "refs/heads/main"));

        let encoded = serde_json::to_string(&id).expect("serialize node id");
        let decoded: NodeId = serde_json::from_str(&encoded).expect("deserialize node id");

        assert_eq!(decoded, id);
    }

    #[test]
    fn relation_kind_serializes_as_snake_case() {
        let encoded =
            serde_json::to_string(&RelationKind::CreatedWorktree).expect("serialize relation kind");

        assert_eq!(encoded, r#""created_worktree""#);
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
}
