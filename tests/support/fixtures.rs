use conspectus::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, Confidence, ForgePrId, ForgePrNode,
    ForkId, ForkNode, Freshness, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState,
    MuxSessionId, MuxSessionNode, NodeId, Provenance, RelationKind, RepoId, RepoNode,
    SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
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
            session_kind: None,
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
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
            last_attached_epoch: None,
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
        freshness_epoch: None,
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
        source,
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
                session_kind: None,
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
        source,
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
                session_kind: None,
            }),
            convention,
            strong,
            cached,
        ],
        candidate_links: vec![cached_link, convention_link, strong_link],
        ..GraphSnapshot::empty()
    })
}

pub fn fork_ancestry_graph() -> GraphSnapshot {
    // Four-fork ParentFork chain a -> b -> c -> d (child cites
    // parent, per the v_fork_ancestry convention used in
    // tests/query_regression.rs).
    let mut snap = GraphSnapshot::empty();
    for key in ["a", "b", "c", "d"] {
        snap.nodes.push(GraphNode::Fork(ForkNode {
            id: ForkId::new(format!("atelier/{key}")),
            provider: "atelier".to_string(),
            provider_source_key: key.to_string(),
            name: Some(key.to_string()),
            scope: None,
            capabilities: vec![],
        }));
    }
    for (child, parent, id) in [("b", "a", "L_ba"), ("c", "b", "L_cb"), ("d", "c", "L_dc")] {
        snap.candidate_links.push(GraphLink {
            id: id.to_string(),
            source: NodeId::Fork(ForkId::new(format!("atelier/{child}"))),
            target: LinkEndpoint::Node {
                id: NodeId::Fork(ForkId::new(format!("atelier/{parent}"))),
            },
            relation: RelationKind::ParentFork,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    resolved(snap)
}

pub fn branch_pr_graph() -> GraphSnapshot {
    // One workspace, two member repos, one branch per repo, one PR
    // per branch. Exercises workspace_contains_repo, belongs_to_repo,
    // and branch_has_forge_pr together.
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/w"),
        root: "/w".to_string(),
        provider: Some("atelier".to_string()),
        name: Some("ws".to_string()),
    }));
    let workspace_id = NodeId::Workspace(WorkspaceId::new("/w"));

    for (idx, (repo_common, refname, pr_number)) in [
        ("/repo1/.git", "refs/heads/main", 1u64),
        ("/repo2/.git", "refs/heads/feature", 2u64),
    ]
    .into_iter()
    .enumerate()
    {
        let repo_id = RepoId::new(repo_common);
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        let branch_id = BranchId::new(repo_id.clone(), refname);
        snap.nodes.push(GraphNode::Branch(BranchNode {
            id: branch_id.clone(),
            refname: refname.to_string(),
            current_commit: None,
            upstream: None,
        }));
        let pr = ForgePrNode {
            id: ForgePrId::new("github", "github.com", "owner", "repo", pr_number),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: pr_number,
            state: Some("open".to_string()),
            url: Some(format!("https://github.com/owner/repo/pull/{pr_number}")),
            updated_epoch: Some(1_700_000_000 + (pr_number as i64) * 60),
            is_draft: false,
        };
        snap.nodes.push(GraphNode::ForgePr(pr.clone()));

        snap.candidate_links.push(GraphLink {
            id: format!("L_wcr_{idx}"),
            source: workspace_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(repo_id),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snap.candidate_links.push(GraphLink {
            id: format!("L_bpr_{idx}"),
            source: NodeId::ForgePr(pr.id.clone()),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    resolved(snap)
}

pub fn ignored_and_overridden_graph() -> GraphSnapshot {
    // One agent session with three mux candidates: one active and
    // resolver-preferred, one explicitly ignored, one overridden by
    // a declared link. Exercises every LinkState variant in the
    // payload so the inspector/filter chrome has real data to
    // render.
    let source = NodeId::AgentSession(agent_id("session-mixed"));
    let preferred = mux_node("tmux:preferred", "preferred");
    let ignored_target = mux_node("tmux:ignored", "ignored");
    let overridden_target = mux_node("tmux:overridden", "overridden");

    let mut active = GraphLink::new(
        "mux-active",
        source.clone(),
        LinkEndpoint::Node { id: preferred.id() },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    active.confidence = Confidence::High;

    let mut ignored = GraphLink::new(
        "mux-ignored",
        source.clone(),
        LinkEndpoint::Node {
            id: ignored_target.id(),
        },
        RelationKind::LinkedToMux,
        Provenance::Convention,
    );
    ignored.state = LinkState::Ignored {
        reason: Some("operator ignored convention match".to_string()),
    };

    let mut overridden = GraphLink::new(
        "mux-overridden",
        source,
        LinkEndpoint::Node {
            id: overridden_target.id(),
        },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    overridden.state = LinkState::Overridden {
        by: "mux-active".to_string(),
        reason: Some("local declared link wins".to_string()),
    };

    resolved(GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(AgentSessionNode {
                id: agent_id("session-mixed"),
                harness_key: "codex".to_string(),
                cwd: Some("/workspace".to_string()),
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            }),
            preferred,
            ignored_target,
            overridden_target,
        ],
        candidate_links: vec![active, ignored, overridden],
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
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
    })
}
