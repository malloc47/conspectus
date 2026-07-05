// Extracted from mod.rs H-HYG-011 rolling wave via #[path = "model_tests.rs"] mod tests;
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

    let encoded =
        serde_json::to_string(&RelationKind::MuxContainsProcess).expect("serialize relation kind");
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
    snap.nodes
        .push(GraphNode::Checkout(CheckoutNode::new(checkout_id, "/r")));
    snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
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
            id: runtime_id,
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
        id: branch_id,
        refname: "refs/heads/main".to_string(),
        current_commit: Some("abc".to_string()),
        upstream: Some("origin/main".to_string()),
    }));
    snap.nodes.push(GraphNode::Fork(ForkNode {
        id: fork_id,
        provider: "github".to_string(),
        provider_source_key: "github:owner:repo".to_string(),
        name: Some("repo".to_string()),
        scope: None,
        capabilities: vec!["read".to_string()],
    }));
    snap.nodes.push(GraphNode::ForgePr(ForgePrNode {
        id: pr_id,
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
            mux: mux_id,
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
    let mut decoded: GraphSnapshot = rkyv::from_bytes::<GraphSnapshot, rkyv::rancor::Error>(&bytes)
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
    let decoded: SourceMetadata = rkyv::from_bytes::<SourceMetadata, rkyv::rancor::Error>(&bytes)
        .expect("deserialize metadata");

    assert_eq!(decoded, original);
}

/// Every `NodeId` variant must round-trip through the archive
/// since `NodeId` is the key of `GraphSnapshot::node_provenance`
/// and `AliasOverlay::entries`. If a future variant adds a
/// payload type whose archive impl is missing, this test catches
/// it before any consumer hits the failure.
/// H-HYG-006 wave 1: `SnapshotIndex::node(id)` agrees with a
/// linear scan for every node in a dense fixture. Guards
/// against index-drift once follow-up waves add more
/// derived maps.
#[test]
fn snapshot_index_agrees_with_linear_scan_on_dense_fixture() {
    let repo = RepoId::new("/r/.git");
    let nodes = vec![
        GraphNode::Repo(RepoNode::new(repo)),
        GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("codex", "/h", "sess-a"),
            "codex",
        )),
        GraphNode::MuxSession(MuxSessionNode::new(
            MuxSessionId::new("tmux:editor"),
            "tmux",
            "editor",
        )),
    ];
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes = nodes.clone();
    let index = SnapshotIndex::new(&snapshot);
    // Every node id resolves through the index.
    for node in &nodes {
        let id = node.id();
        let by_index = index.node(&id);
        let by_scan = snapshot.nodes.iter().find(|n| n.id() == id);
        assert!(
            by_index.is_some() && by_index.map(GraphNode::id) == by_scan.map(GraphNode::id),
            "index disagrees with linear scan for {id:?}"
        );
    }
    assert_eq!(index.node_count(), snapshot.nodes.len());
    // Unknown id resolves to None.
    assert!(index.node(&NodeId::Pin(PinId::new("missing"))).is_none());
}

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
        NodeId::Branch(BranchId::new(repo, "refs/heads/main")),
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

/// H-REF-002: every `RelationKind` variant must round-trip
/// through the snake_case codec. Adding a new variant that
/// misses one side surfaces here.
#[test]
fn relation_kind_snake_case_round_trips_for_every_variant() {
    let variants = [
        RelationKind::AssociatedWith,
        RelationKind::BelongsToRepo,
        RelationKind::CheckedOutBranch,
        RelationKind::WorkspaceContainsRepo,
        RelationKind::BranchHasForgePr,
        RelationKind::LinkedToMux,
        RelationKind::RootedIn,
        RelationKind::ForksWorkspace,
        RelationKind::ForksRepo,
        RelationKind::CreatedCheckout,
        RelationKind::ReferencedCheckout,
        RelationKind::ParentSession,
        RelationKind::ChildSession,
        RelationKind::CreatedBranch,
        RelationKind::AssociatedBranch,
        RelationKind::ParentFork,
        RelationKind::RootedAtPath,
        RelationKind::MuxContainsProcess,
        RelationKind::ProcessIdentifiesSession,
        RelationKind::ProcessCandidatesSession,
        RelationKind::PinTargetsMux,
        RelationKind::PinRealizedBySession,
    ];
    for r in &variants {
        let label = r.snake_case();
        let parsed = RelationKind::from_snake_case(label).expect("round-trips");
        assert_eq!(&parsed, r, "snake_case round-trip mismatch for {r:?}");
    }
}

#[test]
fn relation_kind_from_snake_case_rejects_unknown() {
    let err = RelationKind::from_snake_case("nothing").expect_err("unknown label");
    assert!(err.contains("invalid relation"), "got {err:?}");
}
