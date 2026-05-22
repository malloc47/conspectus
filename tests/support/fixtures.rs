use conspectus::model::{
    AgentSessionId, AgentSessionNode, Confidence, ForkId, ForkNode, Freshness, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, MuxSessionId, MuxSessionNode, NodeId, Provenance,
    RelationKind, RepoId, RepoNode, SourceMetadata, UnresolvedEndpoint,
};
use conspectus::resolve;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}

pub fn orphan_session_graph() -> GraphSnapshot {
    resolved(GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(AgentSessionNode {
            id: agent_id("session-orphan"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: Some("orphan session".to_string()),
            last_message_preview: None,
            last_active_epoch: None,
        })],
        ..GraphSnapshot::empty()
    })
}

pub fn mux_only_graph() -> GraphSnapshot {
    resolved(GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("tmux:solo"),
            backend: "tmux".to_string(),
            native_id: "solo".to_string(),
            cwd: Some("/workspace".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            activity_epoch: None,
            created_epoch: None,
        })],
        ..GraphSnapshot::empty()
    })
}

pub fn repo_only_graph() -> GraphSnapshot {
    resolved(GraphSnapshot {
        nodes: vec![GraphNode::Repo(RepoNode::new(repo_id()))],
        ..GraphSnapshot::empty()
    })
}

pub fn unresolved_lineage_graph() -> GraphSnapshot {
    let fork = ForkNode {
        id: ForkId::new("atelier/fork-alpha"),
        provider: "atelier".to_string(),
        provider_source_key: "fork-alpha".to_string(),
        name: Some("alpha".to_string()),
        scope: Some("workspace".to_string()),
        capabilities: vec!["session".to_string()],
    };
    let mut link = GraphLink::new(
        "lineage-child-unresolved",
        NodeId::Fork(fork.id.clone()),
        LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some("codex".to_string()),
                native_id: Some("pending-child".to_string()),
                state_scope: Some("/home/me/.codex".to_string()),
                path: Some("/workspace/fork-alpha".to_string()),
                metadata: Default::default(),
            },
        },
        RelationKind::ChildSession,
        Provenance::StrongDiscovered,
    );
    link.confidence = Confidence::High;
    link.freshness = Freshness::Fresh;
    link.source_metadata = SourceMetadata {
        adapter: "atelier".to_string(),
        evidence: Some("fork harness entry".to_string()),
        fields: Default::default(),
    };

    resolved(GraphSnapshot {
        nodes: vec![GraphNode::Fork(fork)],
        candidate_links: vec![link],
        ..GraphSnapshot::empty()
    })
}

pub fn conflict_graph() -> GraphSnapshot {
    let source = NodeId::AgentSession(agent_id("session-a"));
    let first = mux_node("tmux:one", "one");
    let second = mux_node("tmux:two", "two");
    let discovered = GraphLink::new(
        "mux-discovered",
        source.clone(),
        LinkEndpoint::Node { id: first.id() },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    let declared = GraphLink::new(
        "mux-declared",
        source.clone(),
        LinkEndpoint::Node { id: second.id() },
        RelationKind::LinkedToMux,
        Provenance::LocalDeclared,
    );

    resolved(GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(AgentSessionNode {
                id: agent_id("session-a"),
                harness_key: "codex".to_string(),
                cwd: Some("/workspace".to_string()),
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
            }),
            first,
            second,
        ],
        candidate_links: vec![discovered, declared],
        ..GraphSnapshot::empty()
    })
}

pub fn mux_candidates_graph() -> GraphSnapshot {
    let source = NodeId::AgentSession(agent_id("session-a"));
    let convention = mux_node("tmux:convention", "convention");
    let strong = mux_node("tmux:process", "process");
    let cached = mux_node("tmux:cached", "cached");
    let mut convention_link = GraphLink::new(
        "mux-convention",
        source.clone(),
        LinkEndpoint::Node {
            id: convention.id(),
        },
        RelationKind::LinkedToMux,
        Provenance::Convention,
    );
    convention_link.source_metadata.adapter = "tmux".to_string();
    convention_link.source_metadata.evidence = Some("naming convention".to_string());

    let mut strong_link = GraphLink::new(
        "mux-process",
        source.clone(),
        LinkEndpoint::Node { id: strong.id() },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    strong_link.confidence = Confidence::High;
    strong_link.source_metadata.adapter = "process".to_string();
    strong_link.source_metadata.evidence = Some("launcher pid".to_string());

    let cached_link = GraphLink::new(
        "mux-cached",
        source.clone(),
        LinkEndpoint::Node { id: cached.id() },
        RelationKind::LinkedToMux,
        Provenance::Cached,
    );

    resolved(GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(AgentSessionNode {
                id: agent_id("session-a"),
                harness_key: "codex".to_string(),
                cwd: Some("/workspace".to_string()),
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
            }),
            convention,
            strong,
            cached,
        ],
        candidate_links: vec![cached_link, convention_link, strong_link],
        ..GraphSnapshot::empty()
    })
}

fn resolved(snapshot: GraphSnapshot) -> GraphSnapshot {
    resolve::resolve_snapshot(snapshot)
}

fn repo_id() -> RepoId {
    RepoId::new("/workspace/repo/.git")
}

fn agent_id(session: &str) -> AgentSessionId {
    AgentSessionId::new("codex", "/home/me/.codex", session)
}

fn mux_node(id: &str, native_id: &str) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(id),
        backend: "tmux".to_string(),
        native_id: native_id.to_string(),
        cwd: Some("/workspace".to_string()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        activity_epoch: None,
        created_epoch: None,
    })
}
