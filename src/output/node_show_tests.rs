// Extracted from node_show.rs H-HYG-011 rolling wave via #[path = "node_show_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, MuxSessionId, MuxSessionNode, Provenance, RelationKind, RepoId, RepoNode,
    SourceMetadata, WorkspaceId, WorkspaceNode,
};
use crate::resolve::resolve_snapshot;

fn agent_node(harness: &str, scope: &str, key: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new(harness, scope, key),
            harness.to_string(),
        )
        .with_cwd("/work".to_string()),
    )
}

fn mux_node(backend: &str, name: &str) -> GraphNode {
    GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new(format!("{backend}:{name}")),
            backend.to_string(),
            name.to_string(),
        )
        .with_cwd("/work".to_string()),
    )
}

fn linked_to_mux(id: &str, source: NodeId, target: NodeId) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn resolve_node_id_matches_short_hex_prefix() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_node("codex", "/state", "alpha")],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();
    let prefix = &node_short_id_from_display(&id.to_string())[..6];
    let resolved = resolve_node_id(prefix, &snapshot).expect("resolved");
    assert_eq!(resolved, id);
}

#[test]
fn resolve_node_id_matches_display_form() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_node("codex", "/state", "alpha")],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();
    let resolved = resolve_node_id(&id.to_string(), &snapshot).expect("resolved");
    assert_eq!(resolved, id);
}

#[test]
fn resolve_node_id_matches_harness_label() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_node("codex", "/state", "alpha")],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();
    let resolved = resolve_node_id("codex:alpha", &snapshot).expect("resolved");
    assert_eq!(resolved, id);
}

#[test]
fn resolve_node_id_matches_mux_label() {
    let snapshot = GraphSnapshot {
        nodes: vec![mux_node("tmux", "editor")],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();
    let resolved = resolve_node_id("tmux:editor", &snapshot).expect("resolved");
    assert_eq!(resolved, id);
}

#[test]
fn resolve_node_id_errors_on_unknown_input() {
    let snapshot = GraphSnapshot::empty();
    let err = resolve_node_id("does-not-exist", &snapshot).unwrap_err();
    assert!(matches!(err, NodeResolveError::NotFound { .. }));
}

#[test]
fn resolve_node_id_errors_on_ambiguous_prefix() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_node("codex", "/state", "one"),
            agent_node("codex", "/state", "two"),
        ],
        ..GraphSnapshot::empty()
    };
    let first = node_short_id_from_display(&snapshot.nodes[0].id().to_string());
    let second = node_short_id_from_display(&snapshot.nodes[1].id().to_string());
    let shared_len = first
        .chars()
        .zip(second.chars())
        .take_while(|(a, b)| a == b)
        .count();
    if shared_len == 0 {
        return;
    }
    let prefix = &first[..shared_len];
    let err = resolve_node_id(prefix, &snapshot).unwrap_err();
    assert!(matches!(err, NodeResolveError::Ambiguous { .. }));
}

#[test]
fn node_show_renders_alias_in_place_of_title() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("opencode", "/state", "alpha"),
                "opencode".to_string(),
            )
            .with_cwd("/work".to_string())
            .with_title("harness title that should be hidden".to_string()),
        )],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();
    snapshot
        .aliases
        .insert(id.clone(), "ingest-refactor".to_string());

    let output = render_node_show(&snapshot, &id, false);
    assert!(
        output.contains("alias:       ingest-refactor"),
        "alias row missing in:\n{output}",
    );
    assert!(
        !output.contains("title:"),
        "title row should be hidden when alias is set:\n{output}",
    );
}

#[test]
fn node_show_falls_back_to_title_when_no_alias() {
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("opencode", "/state", "alpha"),
                "opencode".to_string(),
            )
            .with_cwd("/work".to_string())
            .with_title("Phase 8 mockup".to_string()),
        )],
        ..GraphSnapshot::empty()
    };
    let id = snapshot.nodes[0].id();

    let output = render_node_show(&snapshot, &id, false);
    assert!(
        output.contains("title:       Phase 8 mockup"),
        "title row should appear when alias is unset:\n{output}",
    );
    assert!(!output.contains("alias:"));
}

fn workspace_node(root: &str, provider: Option<&str>, name: Option<&str>) -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(root),
        root: root.to_string(),
        provider: provider.map(str::to_string),
        name: name.map(str::to_string),
    })
}

fn repo_node(common_dir: &str) -> GraphNode {
    GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
}

fn workspace_contains_repo_link(
    link_id: &str,
    workspace_root: &str,
    repo_common_dir: &str,
    logical_path: &str,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(logical_path.to_string()),
    );
    GraphLink {
        id: link_id.to_string(),
        source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
        target: LinkEndpoint::Node {
            id: NodeId::Repo(RepoId::new(repo_common_dir)),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: None,
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn session_workspace_link(link_id: &str, session_id: NodeId, workspace_root: &str) -> GraphLink {
    GraphLink {
        id: link_id.to_string(),
        source: session_id,
        target: LinkEndpoint::Node {
            id: NodeId::Workspace(WorkspaceId::new(workspace_root)),
        },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn workspace_node_show_includes_provider_name_and_member_rows() {
    let workspace_root = "/work/atelier";
    let repo_a = "/work/atelier/atelier/.git";
    let repo_b = "/work/atelier/conspectus/.git";
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
            repo_node(repo_a),
            repo_node(repo_b),
        ],
        candidate_links: vec![
            workspace_contains_repo_link(
                "ws-atelier",
                workspace_root,
                repo_a,
                "/work/atelier/atelier",
            ),
            workspace_contains_repo_link(
                "ws-conspectus",
                workspace_root,
                repo_b,
                "/work/atelier/conspectus",
            ),
        ],
        ..GraphSnapshot::empty()
    });
    let target = NodeId::Workspace(WorkspaceId::new(workspace_root));
    let rendered = render_node_show(&snapshot, &target, false);

    assert!(rendered.contains("kind: workspace"), "got:\n{rendered}");
    assert!(rendered.contains("provider: atelier"), "got:\n{rendered}");
    assert!(
        rendered.contains("name:     atelier-ws"),
        "got:\n{rendered}"
    );
    assert!(rendered.contains("member:   atelier"), "got:\n{rendered}");
    assert!(
        rendered.contains("member:   conspectus"),
        "got:\n{rendered}",
    );
}

#[test]
fn workspace_node_show_omits_member_rows_when_no_members() {
    let workspace_root = "/work/empty";
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![workspace_node(workspace_root, None, None)],
        ..GraphSnapshot::empty()
    });
    let target = NodeId::Workspace(WorkspaceId::new(workspace_root));
    let rendered = render_node_show(&snapshot, &target, false);

    assert!(rendered.contains("kind: workspace"));
    assert!(
        !rendered.contains("member:"),
        "no member rows expected when the workspace has no resolved repos:\n{rendered}",
    );
    assert!(
        !rendered.contains("provider:"),
        "no provider row expected when the workspace has no provider:\n{rendered}",
    );
}

#[test]
fn agent_session_node_show_includes_workspace_row() {
    let workspace_root = "/work/atelier";
    let session = agent_node("codex", "/state", "alpha");
    let session_id = session.id();
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            session,
            workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
        ],
        candidate_links: vec![session_workspace_link(
            "assoc-1",
            session_id.clone(),
            workspace_root,
        )],
        ..GraphSnapshot::empty()
    });
    let rendered = render_node_show(&snapshot, &session_id, false);

    assert!(rendered.contains("kind: agent_session"));
    assert!(
        rendered.contains("workspace:   atelier-ws"),
        "workspace row missing from agent_session output:\n{rendered}",
    );
}

#[test]
fn agent_session_node_show_emits_one_workspace_row_per_association() {
    let session = agent_node("codex", "/state", "alpha");
    let session_id = session.id();
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            session,
            workspace_node("/work/a", None, Some("alpha-ws")),
            workspace_node("/work/b", None, Some("beta-ws")),
        ],
        candidate_links: vec![
            session_workspace_link("assoc-a", session_id.clone(), "/work/a"),
            session_workspace_link("assoc-b", session_id.clone(), "/work/b"),
        ],
        ..GraphSnapshot::empty()
    });
    let rendered = render_node_show(&snapshot, &session_id, false);

    assert!(rendered.contains("workspace:   alpha-ws"));
    assert!(rendered.contains("workspace:   beta-ws"));
}

#[test]
fn agent_session_node_show_omits_workspace_row_when_unassociated() {
    let session = agent_node("codex", "/state", "alpha");
    let session_id = session.id();
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![session],
        ..GraphSnapshot::empty()
    });
    let rendered = render_node_show(&snapshot, &session_id, false);
    assert!(
        !rendered.contains("workspace:"),
        "no workspace row expected without an association:\n{rendered}",
    );
}

#[test]
fn render_node_show_includes_outgoing_link_and_resolved_relationship() {
    let agent = agent_node("codex", "/state", "alpha");
    let mux = mux_node("tmux", "editor");
    let agent_id = agent.id();
    let mux_id = mux.id();
    let snapshot = GraphSnapshot {
        nodes: vec![agent, mux],
        candidate_links: vec![linked_to_mux("link-1", agent_id.clone(), mux_id.clone())],
        resolved_relationships: vec![crate::model::ResolvedRelationship {
            source: agent_id.clone(),
            target: mux_id,
            relation: RelationKind::LinkedToMux,
            selected_link_id: Some("link-1".to_string()),
            competing_link_ids: vec![],
            explanation: None,
        }],
        ..GraphSnapshot::empty()
    };
    let rendered = render_node_show(&snapshot, &agent_id, false);
    assert!(rendered.contains("kind: agent_session"));
    assert!(rendered.contains("outgoing candidate links: 1"));
    assert!(rendered.contains("linked_to_mux"));
    assert!(
        rendered.contains("linked_to_mux   → tmux:editor"),
        "candidate link should use mux-native label:\n{rendered}",
    );
    assert!(rendered.contains("resolved relationships: 1"));
    assert!(
        rendered.contains("linked_to_mux   codex:alpha → tmux:editor"),
        "resolved relationship should use external labels:\n{rendered}",
    );
    assert!(rendered.contains("selected=link-1"));
}
