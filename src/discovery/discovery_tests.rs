use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, LinkEndpoint, MuxSessionId, MuxSessionNode,
    NodeId, Provenance, RelationKind,
};

struct StaticProvider(GraphFragment);

impl DiscoveryProvider for StaticProvider {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        Ok(self.0.clone())
    }
}

#[test]
fn empty_local_discovery_returns_empty_snapshot() {
    let snapshot = LocalDiscovery::new()
        .discover(&DiscoveryContext::from_root("/workspace"))
        .expect("empty discovery succeeds");

    assert_eq!(snapshot, GraphSnapshot::empty());
}

#[test]
fn local_discovery_merges_provider_fragments_without_resolving() {
    let session = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
    let mux = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
    let link = GraphLink::new(
        "session-mux",
        session,
        LinkEndpoint::Node { id: mux },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    let discovery = LocalDiscovery::new()
        .with_provider(StaticProvider(GraphFragment {
            nodes: vec![GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("codex", "global", "s1"),
                "codex".to_string(),
            ))],
            candidate_links: vec![link.clone()],
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
            mux_probes: Default::default(),
        }))
        .with_provider(StaticProvider(GraphFragment {
            nodes: vec![GraphNode::MuxSession(MuxSessionNode::new(
                MuxSessionId::new("tmux:s1"),
                "tmux".to_string(),
                "s1".to_string(),
            ))],
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
            mux_probes: Default::default(),
        }));

    let snapshot = discovery
        .discover(&DiscoveryContext::from_root("/workspace"))
        .expect("discovery succeeds");

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.candidate_links, vec![link]);
    assert!(snapshot.resolved_relationships.is_empty());
}

#[test]
fn merge_fragments_deduplicates_nodes_and_links_by_identity() {
    let node = GraphNode::MuxSession(MuxSessionNode::new(
        MuxSessionId::new("tmux:s1"),
        "tmux".to_string(),
        "s1".to_string(),
    ));
    let source = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
    let target = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
    let link = GraphLink::new(
        "session-mux",
        source,
        LinkEndpoint::Node { id: target },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );

    let snapshot = merge_fragments([
        GraphFragment {
            nodes: vec![node.clone()],
            candidate_links: vec![link.clone()],
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
            mux_probes: Default::default(),
        },
        GraphFragment {
            nodes: vec![node],
            candidate_links: vec![link.clone()],
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
            mux_probes: Default::default(),
        },
    ]);

    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.candidate_links, vec![link]);
}

#[test]
fn merge_fragments_folds_node_provenance_first_write_wins() {
    // Two fragments contribute the same node id with different
    // provenance entries. First-write-wins, matching the
    // dedup-on-node-id semantics one block above. The
    // unique-to-fragment-B node carries its provider through.
    let mux_id = MuxSessionId::new("tmux:s1");
    let mux_node = GraphNode::MuxSession(MuxSessionNode::new(
        mux_id.clone(),
        "tmux".to_string(),
        "s1".to_string(),
    ));
    let agent_node = GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("codex", "/state", "alpha"),
        "codex".to_string(),
    ));
    let mux_node_id = NodeId::MuxSession(mux_id);
    let agent_node_id = agent_node.id();

    let mut prov_a = BTreeMap::new();
    prov_a.insert(
        mux_node_id.clone(),
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(1_700_000_100),
        },
    );
    let mut prov_b = BTreeMap::new();
    // B contributes a competing entry for the mux node — it
    // should lose to A's earlier write — plus a fresh entry for
    // the agent node that A did not touch.
    prov_b.insert(
        mux_node_id.clone(),
        NodeProvenance {
            provider: "cross_link".to_string(),
            freshness_epoch: Some(1_700_000_999),
        },
    );
    prov_b.insert(
        agent_node_id.clone(),
        NodeProvenance {
            provider: "harness::codex".to_string(),
            freshness_epoch: Some(1_700_000_200),
        },
    );

    let snapshot = merge_fragments([
        GraphFragment {
            nodes: vec![mux_node.clone()],
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: prov_a,
            mux_probes: Default::default(),
        },
        GraphFragment {
            nodes: vec![mux_node, agent_node],
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: prov_b,
            mux_probes: Default::default(),
        },
    ]);

    assert_eq!(snapshot.node_provenance.len(), 2);
    assert_eq!(
        snapshot.node_provenance.get(&mux_node_id).unwrap().provider,
        "tmux",
        "mux node provenance should come from the first fragment, not the second"
    );
    assert_eq!(
        snapshot
            .node_provenance
            .get(&agent_node_id)
            .unwrap()
            .provider,
        "harness::codex"
    );
}

#[test]
fn merge_with_prior_lets_fresh_win_and_keeps_prior_only_nodes() {
    // Backstop merge semantics: collisions resolve to `fresh`,
    // prior-only nodes survive as a stale-but-better-than-empty
    // fallback. The persisted cache continues to surface state
    // the live scan did not see (e.g. a repo from yesterday's
    // cwd) without overriding anything the live scan refreshed.
    let mux_id = MuxSessionId::new("tmux:keep");
    let fresh_mux = GraphNode::MuxSession(
        MuxSessionNode::new(mux_id.clone(), "tmux".to_string(), "keep".to_string())
            .with_cwd("/fresh/cwd".to_string()),
    );
    let prior_mux = GraphNode::MuxSession(
        MuxSessionNode::new(mux_id.clone(), "tmux".to_string(), "keep".to_string())
            .with_cwd("/stale/cwd".to_string()),
    );
    let prior_only = GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("codex", "/state", "from-cache"),
        "codex".to_string(),
    ));
    let prior_only_id = prior_only.id();

    let mut fresh = GraphSnapshot::empty();
    fresh.nodes.push(fresh_mux);
    fresh.node_provenance.insert(
        NodeId::MuxSession(mux_id.clone()),
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(1_700_000_900),
        },
    );

    let mut prior = GraphSnapshot::empty();
    prior.nodes.push(prior_mux);
    prior.nodes.push(prior_only);
    prior.node_provenance.insert(
        NodeId::MuxSession(mux_id.clone()),
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(1_700_000_100),
        },
    );
    prior.node_provenance.insert(
        prior_only_id.clone(),
        NodeProvenance {
            provider: "harness::codex".to_string(),
            freshness_epoch: Some(1_700_000_050),
        },
    );

    let merged = merge_with_prior(fresh, prior);

    // The fresh mux's cwd survives the merge — prior loses on
    // the collision.
    let merged_mux = merged
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::MuxSession(s) if s.id == mux_id => Some(s),
            _ => None,
        })
        .expect("mux node present in merged snapshot");
    assert_eq!(merged_mux.cwd.as_deref(), Some("/fresh/cwd"));

    // The prior-only agent session still appears.
    assert!(
        merged.nodes.iter().any(|node| node.id() == prior_only_id),
        "prior-only node should survive the backstop merge"
    );

    // Provenance for the collision uses fresh's epoch.
    let mux_prov = merged
        .node_provenance
        .get(&NodeId::MuxSession(mux_id.clone()))
        .expect("mux provenance");
    assert_eq!(mux_prov.freshness_epoch, Some(1_700_000_900));

    // Resolver re-runs on the merged snapshot — warm-start
    // never carries forward resolved relationships.
    assert!(merged.resolved_relationships.is_empty());
}

#[test]
fn merge_with_prior_keeps_the_fresh_live_table_and_drops_the_prior_one() {
    // The prior's live observations are a cycle old and the live-state
    // pass rebinds the table after the merge (ADR 0205), so none of
    // them leaks into the merged snapshot, not even for a node that
    // only the prior has; the fresh table passes through whole.
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};

    let kept = AgentSessionId::new("codex", "/state", "kept");
    let ended = AgentSessionId::new("codex", "/state", "ended");
    let pane = MuxSessionId::new("tmux:pane");
    let session_node = |id: &AgentSessionId| {
        GraphNode::AgentSession(AgentSessionNode::new(id.clone(), "codex".to_string()))
    };
    let observed = |state| {
        LiveBinding::new(
            LiveObservation::new(state, LiveSource::Hook, 1_000),
            Liveness::Alive,
        )
    };

    let mut fresh = GraphSnapshot::empty();
    fresh.nodes.push(session_node(&kept));
    fresh.nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        pane.clone(),
        "tmux".to_string(),
        "pane".to_string(),
    )));
    fresh.node_live.insert(
        NodeId::AgentSession(kept.clone()),
        observed(LiveState::Waiting),
    );
    fresh
        .node_live
        .insert(NodeId::MuxSession(pane), observed(LiveState::Starting));

    let mut prior = GraphSnapshot::empty();
    prior.nodes.push(session_node(&kept));
    prior.nodes.push(session_node(&ended));
    prior
        .node_live
        .insert(NodeId::AgentSession(kept), observed(LiveState::Working));
    prior.node_live.insert(
        NodeId::AgentSession(ended.clone()),
        observed(LiveState::Working),
    );

    let expected = fresh.node_live.clone();
    let merged = merge_with_prior(fresh, prior);

    assert!(
        merged
            .nodes
            .iter()
            .any(|node| node.id() == NodeId::AgentSession(ended.clone())),
        "the prior-only node survives as usual"
    );
    assert_eq!(merged.node_live, expected);

    let only_prior = merge_with_prior(GraphSnapshot::empty(), {
        let mut prior = GraphSnapshot::empty();
        prior.nodes.push(session_node(&ended));
        prior
            .node_live
            .insert(NodeId::AgentSession(ended), observed(LiveState::Ready));
        prior
    });
    assert!(only_prior.node_live.is_empty());
}

#[test]
fn a_fragment_made_from_a_snapshot_keeps_only_provider_output() {
    // Providers emit fragments; the live table is bound after the merge,
    // so the conversion leaves it behind with the other derived fields.
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};

    let session = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "s1"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "s1"),
            "codex".to_string(),
        )));
    snapshot.node_live.insert(
        session,
        LiveBinding::new(
            LiveObservation::new(LiveState::Working, LiveSource::Hook, 1_000),
            Liveness::Alive,
        ),
    );

    let fragment = GraphFragment::from(snapshot.clone());
    assert_eq!(fragment.nodes, snapshot.nodes);
    assert!(fragment.into_snapshot().node_live.is_empty());
}

#[test]
fn context_normalizes_and_deduplicates_scan_roots() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let root = temp.path();
    let nested = root.join("nested");
    std::fs::create_dir(&nested).expect("create nested dir");

    let context =
        DiscoveryContext::from_roots([root, root, nested.as_path()]).expect("roots normalize");

    assert_eq!(context.roots().len(), 2);
    assert!(context.roots()[0].is_absolute());
}

#[test]
fn discover_local_warm_with_keeps_fresh_provider_slice_from_prior() {
    // Stage a prior snapshot with a `github` slice timestamped
    // *just now* (well within the 5-minute forge TTL). The
    // forge runner is *not* installed in the config, so a cold
    // rebuild would have no github nodes; the warm-start path
    // should observe github is fresh, skip running it (vacuous
    // here), and let the prior slice flow through the
    // backstop merge so the result still carries it.
    //
    // Uses a `RepoNode` as a stand-in payload because the
    // freshness gate runs on `node_provenance.provider`, not
    // the node type; the warm-start contract only cares about
    // the provider key, not which node variant carries it.
    use crate::config::ServerIntervals;
    use crate::model::RepoNode;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let mut prior = GraphSnapshot::empty();
    let repo_id = crate::model::RepoId::new("/fresh-github/.git");
    let node_id = NodeId::Repo(repo_id.clone());
    prior.nodes.push(GraphNode::Repo(RepoNode::new(repo_id)));
    prior.node_provenance.insert(
        node_id.clone(),
        NodeProvenance {
            provider: "github".to_string(),
            freshness_epoch: Some(current_epoch()),
        },
    );

    let snapshot = discover_local_warm_with(
        [temp.path()],
        LocalDiscoveryConfig::empty(),
        prior,
        &ServerIntervals::default(),
    )
    .expect("warm-start discovery");

    assert!(
        snapshot.nodes.iter().any(|node| node.id() == node_id),
        "fresh github slice should survive the warm-start merge"
    );
}

#[test]
fn discover_local_warm_with_evicts_stale_slice_and_re_runs_cold() {
    // Same setup but the prior github slice is stamped at
    // epoch 0 — well past the 5-minute TTL relative to
    // wall-clock `now`. The gate marks it stale and the
    // warm-start path evicts it from the prior; since no forge
    // runner is wired up, the live run emits no github node
    // either, so the result has zero github nodes (correctly
    // reflecting deleted upstream state).
    use crate::config::ServerIntervals;
    use crate::model::RepoNode;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let mut prior = GraphSnapshot::empty();
    let repo_id = crate::model::RepoId::new("/stale-github/.git");
    let node_id = NodeId::Repo(repo_id.clone());
    prior.nodes.push(GraphNode::Repo(RepoNode::new(repo_id)));
    prior.node_provenance.insert(
        node_id.clone(),
        NodeProvenance {
            provider: "github".to_string(),
            freshness_epoch: Some(0),
        },
    );

    let snapshot = discover_local_warm_with(
        [temp.path()],
        LocalDiscoveryConfig::empty(),
        prior,
        &ServerIntervals::default(),
    )
    .expect("warm-start discovery");

    assert!(
        !snapshot.nodes.iter().any(|node| node.id() == node_id),
        "stale github slice should be evicted; no forge runner means nothing replaces it"
    );
}

#[test]
fn warm_start_preserves_process_tree_links_on_a_git_only_cycle() {
    // ADR 0091: the process-tree pass is
    // class-gated. When no mux/harness provider runs this cycle (the
    // empty config runs nothing, standing in for a git/forge-only
    // tick), the prior cross_link agent↔mux link must survive without
    // a fresh `/proc` walk re-deriving it — otherwise agent↔pane
    // links would flicker out between mux/harness ticks.
    use crate::config::ServerIntervals;
    use crate::model::SourceMetadata;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let now = current_epoch();

    let agent_id = AgentSessionId::new("codex", "global", "s1");
    let mux_id = MuxSessionId::new("tmux:s1");
    let agent_node = NodeId::AgentSession(agent_id.clone());
    let mux_node = NodeId::MuxSession(mux_id.clone());

    let mut prior = GraphSnapshot::empty();
    prior
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            agent_id,
            "codex".to_string(),
        )));
    prior.nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        mux_id,
        "tmux".to_string(),
        "s1".to_string(),
    )));
    // Mux + harness slices are fresh so a warm cycle would skip them.
    prior.node_provenance.insert(
        agent_node.clone(),
        NodeProvenance {
            provider: "codex".to_string(),
            freshness_epoch: Some(now),
        },
    );
    prior.node_provenance.insert(
        mux_node.clone(),
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(now),
        },
    );

    // The cross_link agent↔mux link the process-tree pass produced on
    // a prior cycle, stamped fresh.
    let mut link = GraphLink::new(
        "cross-link-s1",
        agent_node,
        LinkEndpoint::Node { id: mux_node },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    link.source_metadata = SourceMetadata {
        adapter: crate::discovery::providers::CROSS_LINK.to_string(),
        freshness_epoch: Some(now),
        ..SourceMetadata::default()
    };
    prior.candidate_links.push(link);

    let snapshot = discover_local_warm_with(
        [temp.path()],
        LocalDiscoveryConfig::empty(),
        prior,
        &ServerIntervals::default(),
    )
    .expect("warm-start discovery");

    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|l| l.id == "cross-link-s1"),
        "prior cross_link link should survive a cycle where mux/harness did not run",
    );
}

#[test]
fn context_rejects_missing_scan_roots() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let missing = temp.path().join("missing");

    let error = DiscoveryContext::from_roots([missing.clone()])
        .expect_err("missing roots should be rejected");

    assert!(matches!(&error, DiscoveryError::MissingScanRoot(path) if *path == missing));
    assert!(error.to_string().contains("scan root does not exist"));
}

#[test]
fn context_rejects_file_scan_roots() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let file = temp.path().join("file");
    std::fs::write(&file, "").expect("write file");

    let error = DiscoveryContext::from_roots([file]).expect_err("files should be rejected");

    assert!(matches!(error, DiscoveryError::ScanRootNotADirectory(_)));
}

#[test]
fn provider_failure_names_the_provider() {
    struct Failing;
    impl DiscoveryProvider for Failing {
        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            anyhow::bail!("backend exploded")
        }
    }

    let error = LocalDiscovery::new()
        .with_keyed_provider(&["tmux"], Failing)
        .discover(&DiscoveryContext::default())
        .expect_err("provider failure propagates");

    let DiscoveryError::Provider {
        provider_keys,
        source,
    } = &error
    else {
        panic!("expected a provider error, got {error:?}");
    };
    assert_eq!(provider_keys, &["tmux"]);
    assert_eq!(source.to_string(), "backend exploded");
    assert_eq!(error.to_string(), "discovery provider `tmux` failed");
}

#[test]
fn local_discovery_accepts_existing_non_git_roots_as_sparse_graphs() {
    let temp = tempfile::TempDir::new().expect("temp dir");

    let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
        .expect("local discovery succeeds");

    assert_eq!(snapshot, GraphSnapshot::empty());
}

#[test]
fn discover_local_with_runs_harness_and_tmux_providers_and_cross_links() {
    use crate::discovery::harness::codex::HARNESS_KEY as CODEX_KEY;
    use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};
    use crate::discovery::tmux::FakeTmux;
    use crate::model::{GraphNode, RelationKind};

    let temp = tempfile::TempDir::new().expect("temp dir");
    let scan_root = temp.path().join("scan");
    std::fs::create_dir(&scan_root).expect("scan dir");
    let harness_root = temp.path().join("state");
    std::fs::create_dir(&harness_root).expect("state dir");
    let fixture = HarnessFixture::at(&harness_root);
    fixture
        .write_codex_session(&CodexSessionRecord::new("session-x").with_cwd("/work/x"))
        .expect("write codex session");

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_KEY, fixture.codex_state_root())
        .with_tmux_runner(FakeTmux::with_sessions(
            "alpha\t/work/x\t1700000500\t1700000000\n",
        ));

    let snapshot = discover_local_with([scan_root.as_path()], config).expect("discover");

    assert!(
        snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::AgentSession(s) if s.harness_key == CODEX_KEY)),
        "codex session node should be present"
    );
    assert!(
        snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::MuxSession(_))),
        "fake tmux session node should be present"
    );
    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|link| link.relation == RelationKind::LinkedToMux),
        "cross_link should infer at least one LinkedToMux candidate"
    );
}

#[test]
fn discover_local_with_runs_forge_provider_for_github_repos() {
    use crate::discovery::forge::FakeGh;
    use std::process::Command as ProcessCommand;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let repo_root = temp.path().join("repo");
    std::fs::create_dir(&repo_root).expect("repo dir");
    let run_git = |args: &[&str]| {
        let output = ProcessCommand::new("git")
            .args(args)
            .current_dir(&repo_root)
            .output()
            .expect("run git");
        assert!(output.status.success(), "git {} failed", args.join(" "));
    };
    run_git(&["init", "--initial-branch", "main"]);
    run_git(&["config", "user.name", "Conspectus Test"]);
    run_git(&["config", "user.email", "test@example.invalid"]);
    run_git(&["remote", "add", "origin", "git@github.com:octo/repo.git"]);
    std::fs::write(repo_root.join("README.md"), "fixture\n").expect("write fixture");
    run_git(&["add", "README.md"]);
    run_git(&["commit", "-m", "initial"]);

    let body = r#"[{"number": 42, "state": "OPEN", "headRefName": "main"}]"#;
    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));

    let snapshot = discover_local_with([repo_root.as_path()], config).expect("discover");

    assert!(
        snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::ForgePr(_))),
        "forge provider should emit a ForgePr node"
    );
}

#[test]
fn discover_local_with_skips_forge_when_runner_absent() {
    use crate::model::GraphNode;

    let temp = tempfile::TempDir::new().expect("temp dir");

    let snapshot =
        discover_local_with([temp.path()], LocalDiscoveryConfig::empty()).expect("discover");

    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::ForgePr(_))),
        "no forge nodes should appear when forge runner is not configured"
    );
}

#[test]
fn discover_local_with_skips_tmux_when_runner_absent() {
    use crate::model::GraphNode;

    let temp = tempfile::TempDir::new().expect("temp dir");

    let snapshot =
        discover_local_with([temp.path()], LocalDiscoveryConfig::empty()).expect("discover");

    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::MuxSession(_))),
        "no mux nodes should appear when tmux runner is not configured"
    );
}

#[test]
fn discover_local_with_loads_declared_project_links_when_configured() {
    use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
    use crate::model::Provenance;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    std::fs::create_dir(&project).expect("project dir");
    std::fs::write(
            project.join(PROJECT_CONFIG_FILENAME),
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "declared-session-mux"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:missing" }
            "#,
        )
        .expect("write project config");
    let config = LocalDiscoveryConfig::empty()
        .with_declared_config_loader(ConfigLoader::new().with_home(temp.path()));

    let snapshot = discover_local_with([project.as_path()], config).expect("discover");

    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|link| link.provenance == Provenance::LocalDeclared),
        "declared project config should contribute a local declared candidate"
    );
}

#[test]
fn discover_local_with_loads_project_pins_from_observed_session_cwd() {
    use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
    use crate::discovery::harness::codex::HARNESS_KEY as CODEX_KEY;
    use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};

    let temp = tempfile::TempDir::new().expect("temp dir");
    let scan_root = temp.path().join("scan");
    let project = temp.path().join("project");
    std::fs::create_dir(&scan_root).expect("scan dir");
    std::fs::create_dir(&project).expect("project dir");
    std::fs::write(
        project.join(PROJECT_CONFIG_FILENAME),
        format!(
            r#"
                [pins]
                schema_version = 1

                [[pins.entries]]
                id = "observed"
                display_name = "observed"
                harness = "codex"
                cwd = "{}"
                mux = {{ backend = "tmux", name = "observed" }}
                "#,
            project.display()
        ),
    )
    .expect("write project config");

    let fixture = HarnessFixture::at(temp.path().join("state"));
    fixture
        .write_codex_session(
            &CodexSessionRecord::new("session-observed")
                .with_cwd(project.to_string_lossy().into_owned()),
        )
        .expect("write codex session");

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_KEY, fixture.codex_state_root())
        .with_declared_config_loader(ConfigLoader::new().with_home(temp.path()));

    let snapshot = discover_local_with([scan_root.as_path()], config).expect("discover");

    assert!(
        snapshot
            .pins
            .iter()
            .any(|pin| pin.id == "observed" && pin.legacy),
        "project pin should load from the observed session cwd, not only the scan root"
    );
}

// ---------------------------------------------------------------------------
// Mux/harness slice fingerprint + /proc walk gate.
// ---------------------------------------------------------------------------

fn snapshot_with_mux_session(session_key: &str, native_id: &str, epoch: i64) -> GraphSnapshot {
    let agent_id = AgentSessionId::new("codex", "global", session_key);
    let mux_id = MuxSessionId::new(format!("tmux:{native_id}"));
    let agent_node = NodeId::AgentSession(agent_id.clone());
    let mux_node = NodeId::MuxSession(mux_id.clone());

    let mut snap = GraphSnapshot::empty();
    snap.nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            agent_id,
            "codex".to_string(),
        )));
    snap.nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        mux_id,
        "tmux".to_string(),
        native_id.to_string(),
    )));
    snap.node_provenance.insert(
        agent_node,
        NodeProvenance {
            provider: "codex".to_string(),
            freshness_epoch: Some(epoch),
        },
    );
    snap.node_provenance.insert(
        mux_node,
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(epoch),
        },
    );
    snap
}

#[test]
fn slice_fingerprint_returns_none_when_no_mux_or_harness_content_present() {
    // A cycle where only git/forge providers ran has no mux/harness
    // provenance. The gate must return None so today's "no walk"
    // behavior is preserved regardless of history.
    let mut snap = GraphSnapshot::empty();
    snap.nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("codex", "global", "s1"),
            "codex".to_string(),
        )));
    // Deliberately stamp with a non-mux/harness provider — the node
    // is present in `nodes` but not owned by the mux/harness slice.
    snap.node_provenance.insert(
        NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1")),
        NodeProvenance {
            provider: crate::discovery::providers::GIT.to_string(),
            freshness_epoch: Some(1_000),
        },
    );
    assert_eq!(mux_or_harness_slice_fingerprint(&snap), None);
}

#[test]
fn slice_fingerprint_ignores_freshness_epoch_changes_only() {
    // Same content, different `freshness_epoch` stamps. The wall
    // clock advances every cycle; if the fingerprint tracked it,
    // the whole optimization would collapse. Only real state
    // changes should differ.
    let cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = snapshot_with_mux_session("s1", "s1", 2_000);
    // Also stamp a mux/harness-owned link with the new epoch — both
    // node and link freshness_epoch fields must be excluded.
    let agent_node = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
    let mux_node = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
    let mut link = GraphLink::new(
        "session-mux",
        agent_node,
        LinkEndpoint::Node { id: mux_node },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    link.source_metadata.adapter = "codex".to_string();
    link.source_metadata.freshness_epoch = Some(2_000);
    cycle_b.candidate_links.push(link);

    // cycle_a needs the same link (without link changes, cycle_b would
    // add content) — copy with a different epoch to prove the epoch
    // field is what's excluded.
    let mut cycle_a = cycle_a;
    let agent_node = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
    let mux_node = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
    let mut link_a = GraphLink::new(
        "session-mux",
        agent_node,
        LinkEndpoint::Node { id: mux_node },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    link_a.source_metadata.adapter = "codex".to_string();
    link_a.source_metadata.freshness_epoch = Some(1_000);
    cycle_a.candidate_links.push(link_a);

    assert_eq!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "cycles that differ only in freshness_epoch must hash equal"
    );
}

#[test]
fn slice_fingerprint_ignores_mux_activity_epoch_advances() {
    // MuxSessionNode.activity_epoch bumps on
    // every tmux pane keystroke; treating that as a real content
    // change defeats the /proc walk gate on any operator with a
    // live tmux (i.e. all of them). Two snapshots that differ
    // only in activity_epoch / last_attached_epoch must hash equal.
    let mut cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = snapshot_with_mux_session("s1", "s1", 1_000);

    for node in &mut cycle_a.nodes {
        if let GraphNode::MuxSession(mux) = node {
            mux.activity_epoch = Some(5_000);
            mux.last_attached_epoch = Some(5_000);
        }
    }
    for node in &mut cycle_b.nodes {
        if let GraphNode::MuxSession(mux) = node {
            mux.activity_epoch = Some(9_000);
            mux.last_attached_epoch = Some(9_000);
        }
    }

    assert_eq!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "activity_epoch / last_attached_epoch must not affect the fingerprint"
    );
}

#[test]
fn slice_fingerprint_ignores_agent_last_active_epoch_advances() {
    // Session file mtime advances whenever an active session
    // appends a message; the /proc walk output is unaffected by
    // that so it must not force a re-walk.
    let mut cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = snapshot_with_mux_session("s1", "s1", 1_000);

    for node in &mut cycle_a.nodes {
        if let GraphNode::AgentSession(session) = node {
            session.last_active_epoch = Some(5_000);
        }
    }
    for node in &mut cycle_b.nodes {
        if let GraphNode::AgentSession(session) = node {
            session.last_active_epoch = Some(9_999);
        }
    }

    assert_eq!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "last_active_epoch must not affect the fingerprint"
    );
}

#[test]
fn slice_fingerprint_ignores_agent_last_message_preview_and_title_churn() {
    // Even with last_active_epoch
    // zeroed, the fingerprint still churned on any active box
    // because `last_message_preview` and `title` refresh whenever
    // an active session appends. Both are display-only — the
    // /proc walk output is unchanged — so both must be zeroed
    // on the fingerprint's normalized clone.
    let mut cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = snapshot_with_mux_session("s1", "s1", 1_000);

    for node in &mut cycle_a.nodes {
        if let GraphNode::AgentSession(session) = node {
            session.last_message_preview = Some("first preview".to_string());
            session.title = Some("first title".to_string());
        }
    }
    for node in &mut cycle_b.nodes {
        if let GraphNode::AgentSession(session) = node {
            session.last_message_preview = Some("SECOND preview".to_string());
            session.title = Some("updated title".to_string());
        }
    }

    assert_eq!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "last_message_preview and title must not affect the fingerprint"
    );
}

#[test]
fn slice_fingerprint_ignores_live_state() {
    // Live state changes every few seconds and says nothing about
    // which processes a pane runs, so a change in `node_live` alone
    // must never reopen the /proc walk (ADR 0205).
    use crate::model::live::{
        LiveBinding, LiveDetail, LiveObservation, LiveSource, LiveState, Liveness,
    };

    let quiet = snapshot_with_mux_session("s1", "s1", 1_000);
    let agent_node = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
    let mux_node = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));

    let mut working = quiet.clone();
    working.node_live.insert(
        agent_node.clone(),
        LiveBinding::new(
            LiveObservation::new(LiveState::Working, LiveSource::Hook, 1_000_000)
                .with_detail(LiveDetail::Tool)
                .with_pid(4242, Some(7)),
            Liveness::Alive,
        ),
    );
    let mut waiting = working.clone();
    waiting.node_live.insert(
        agent_node,
        LiveBinding::new(
            LiveObservation::new(LiveState::Waiting, LiveSource::Hook, 2_000_000)
                .with_detail(LiveDetail::Permission)
                .with_pid(4242, Some(7)),
            Liveness::Dead,
        ),
    );
    waiting.node_live.insert(
        mux_node,
        LiveBinding::new(
            LiveObservation::new(LiveState::Starting, LiveSource::Hook, 2_000_000),
            Liveness::Unknown,
        ),
    );

    let expected = mux_or_harness_slice_fingerprint(&quiet);
    assert!(expected.is_some());
    assert_eq!(mux_or_harness_slice_fingerprint(&working), expected);
    assert_eq!(mux_or_harness_slice_fingerprint(&waiting), expected);
}

#[test]
fn slice_fingerprint_differs_when_mux_active_pane_pid_changes() {
    // active_pane_pid *does* matter — a new pid means a fresh
    // process to attribute in the walk. Must not be zeroed by
    // the fingerprint normalizer.
    let mut cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = cycle_a.clone();

    for node in &mut cycle_a.nodes {
        if let GraphNode::MuxSession(mux) = node {
            mux.active_pane_pid = Some(12345);
        }
    }
    for node in &mut cycle_b.nodes {
        if let GraphNode::MuxSession(mux) = node {
            mux.active_pane_pid = Some(67890);
        }
    }

    assert_ne!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "active_pane_pid change must invalidate the fingerprint"
    );
}

#[test]
fn slice_fingerprint_differs_when_new_session_appears() {
    // Adding a mux session to the slice is a real state change —
    // the walk must fire on the next cycle.
    let cycle_a = snapshot_with_mux_session("s1", "s1", 1_000);
    let mut cycle_b = cycle_a.clone();
    let agent_id = AgentSessionId::new("codex", "global", "s2");
    let node = NodeId::AgentSession(agent_id.clone());
    cycle_b
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            agent_id,
            "codex".to_string(),
        )));
    cycle_b.node_provenance.insert(
        node,
        NodeProvenance {
            provider: "codex".to_string(),
            freshness_epoch: Some(1_000),
        },
    );

    assert_ne!(
        mux_or_harness_slice_fingerprint(&cycle_a),
        mux_or_harness_slice_fingerprint(&cycle_b),
        "adding a new agent session must change the fingerprint"
    );
}

#[test]
fn slice_fingerprint_ignores_node_ordering() {
    // Discovery providers push into `Vec`s in whatever order they
    // iterate. If Vec order affected the fingerprint we'd get
    // spurious cache misses. Two snapshots with the same content
    // in different orders must hash equal.
    let a = snapshot_with_mux_session("s1", "mux-a", 1_000);
    let mut b = GraphSnapshot::empty();
    // Push the mux node before the agent node — reverse of the
    // helper's order.
    let mux_id = MuxSessionId::new("tmux:mux-a");
    b.nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        mux_id.clone(),
        "tmux".to_string(),
        "mux-a".to_string(),
    )));
    let agent_id = AgentSessionId::new("codex", "global", "s1");
    b.nodes.push(GraphNode::AgentSession(AgentSessionNode::new(
        agent_id.clone(),
        "codex".to_string(),
    )));
    b.node_provenance.insert(
        NodeId::MuxSession(mux_id),
        NodeProvenance {
            provider: "tmux".to_string(),
            freshness_epoch: Some(1_000),
        },
    );
    b.node_provenance.insert(
        NodeId::AgentSession(agent_id),
        NodeProvenance {
            provider: "codex".to_string(),
            freshness_epoch: Some(1_000),
        },
    );

    assert_eq!(
        mux_or_harness_slice_fingerprint(&a),
        mux_or_harness_slice_fingerprint(&b),
        "node insertion order must not affect the fingerprint"
    );
}

#[test]
fn gate_stays_closed_when_slice_is_empty_regardless_of_history() {
    // (None, _) => false. This preserves today's "no mux/harness
    // provenance stamped → no walk" behavior even if we walked
    // last cycle.
    assert!(!should_open_process_tree_gate(None, None));
    assert!(!should_open_process_tree_gate(None, Some(1234)));
}

#[test]
fn gate_opens_on_first_cycle_with_content() {
    // (Some, None) => true. Daemon startup / first observation of
    // any mux/harness content must seed the walk + downstream
    // caches (codex_log, cross_link inference).
    assert!(should_open_process_tree_gate(Some(1234), None));
}

#[test]
fn gate_stays_closed_when_fingerprint_matches_previous() {
    // (Some(x), Some(x)) => false. The whole point of B: skip the
    // walk on quiet cycles.
    assert!(!should_open_process_tree_gate(Some(1234), Some(1234)));
}

#[test]
fn gate_opens_when_fingerprint_changes() {
    // (Some(x), Some(y)) => true when x != y. Real content change
    // (session added/removed, activity epoch bumped) must fire the
    // walk.
    assert!(should_open_process_tree_gate(Some(1234), Some(5678)));
}

#[test]
fn discover_local_with_reuses_the_callers_caches() {
    use crate::discovery::tmux::FakeTmux;

    let temp = tempfile::TempDir::new().expect("temp dir");
    let caches = Arc::new(DiscoveryCaches::default());
    let config = LocalDiscoveryConfig::empty()
        .with_tmux_runner(FakeTmux::with_sessions("alpha\t/work/alpha\t1\t0\n"))
        .with_caches(Arc::clone(&caches));

    discover_local_with([temp.path()], config).expect("discover");

    let cached = caches
        .tmux
        .get(&())
        .expect("the tmux fragment lands in the caller's caches");
    assert_eq!(cached.nodes.len(), 1);
}

#[test]
fn system_mux_backend_returns_the_runner_for_each_registered_backend() {
    for key in tmux::KNOWN_MUX_BACKENDS {
        let runner = system_mux_backend(key).expect("every registered backend has a runner");
        assert_eq!(runner.backend_key(), *key);
    }
    assert!(system_mux_backend("screen").is_none());
}

#[test]
fn only_backends_that_implement_new_session_claim_they_can_create_sessions() {
    let tmux = system_mux_backend(tmux::TMUX_BACKEND).expect("tmux runner");
    let zellij = system_mux_backend(zellij::ZELLIJ_BACKEND).expect("zellij runner");
    assert!(tmux.can_create_sessions());
    assert!(!zellij.can_create_sessions());
}

#[test]
fn mux_runner_for_prefers_the_callers_runner_for_its_own_backend() {
    let fake = tmux::FakeTmux::with_sessions("");
    let mut slot = None;
    let runner = mux_runner_for(tmux::TMUX_BACKEND, &fake, &mut slot).expect("tmux runner");
    assert_eq!(runner.backend_key(), tmux::TMUX_BACKEND);
    assert!(
        slot.is_none(),
        "the caller's tmux runner is used, not a new one"
    );

    let mut slot = None;
    let runner = mux_runner_for(zellij::ZELLIJ_BACKEND, &fake, &mut slot).expect("zellij runner");
    assert_eq!(runner.backend_key(), zellij::ZELLIJ_BACKEND);

    let mut slot = None;
    assert!(mux_runner_for("screen", &fake, &mut slot).is_none());
}

#[test]
fn cwd_git_probes_skip_the_cwd_of_a_gone_mux() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir.path())
            .status()
            .expect("git init")
            .success()
    );
    let mut mux = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    mux.cwd = Some(dir.path().display().to_string());
    let id = NodeId::MuxSession(mux.id.clone());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(mux)],
        ..GraphSnapshot::empty()
    };

    let live = observed_cwd_git_fragment(&snapshot, &DiscoveryCaches::default());
    assert!(!live.nodes.is_empty(), "a live mux's cwd is probed");

    snapshot
        .node_presence
        .insert(id, crate::model::NodePresence::Gone { last_seen: 1 });
    let gone = observed_cwd_git_fragment(&snapshot, &DiscoveryCaches::default());
    assert!(gone.nodes.is_empty(), "a ghost's cwd is not");
}

/// A codex session in a scratch harness root, a tmux mux `alpha` on the
/// default server, and a scratch live-state store, for the tests of the
/// `live_state` pass.
struct LiveWorld {
    temp: tempfile::TempDir,
    caches: Arc<DiscoveryCaches>,
}

impl LiveWorld {
    const SESSION: &str = "session-x";

    fn new() -> Self {
        use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};

        let temp = tempfile::TempDir::new().expect("temp dir");
        std::fs::create_dir(temp.path().join("scan")).expect("scan dir");
        HarnessFixture::at(temp.path().join("state"))
            .write_codex_session(&CodexSessionRecord::new(Self::SESSION).with_cwd("/work/x"))
            .expect("write codex session");
        Self {
            temp,
            caches: Arc::default(),
        }
    }

    fn live_root(&self) -> PathBuf {
        self.temp.path().join("live")
    }

    /// Fold `kind` for `harness:session` into the store, as `hook write`
    /// does, with `context`.
    fn event(
        &self,
        harness: &str,
        session: &str,
        kind: crate::live_store::LiveEventKind,
        context: crate::live_store::LiveContext,
    ) {
        let event =
            crate::live_store::LiveEvent::new(harness, session, crate::live_store::now_ms(), kind)
                .with_context(context);
        crate::live_store::LiveStore::at(self.live_root())
            .apply_event(&event)
            .expect("write the store");
    }

    fn config(&self) -> LocalDiscoveryConfig {
        self.config_listing("alpha\t/work/x\t1700000500\t1700000000\n")
    }

    /// The config with the tmux server listing `listing`.
    fn config_listing(&self, listing: &str) -> LocalDiscoveryConfig {
        use crate::discovery::harness::codex::HARNESS_KEY as CODEX_KEY;
        use crate::discovery::harness::fixtures::HarnessFixture;
        use crate::discovery::tmux::FakeTmux;

        LocalDiscoveryConfig::empty()
            .with_harness_state_root(
                CODEX_KEY,
                HarnessFixture::at(self.temp.path().join("state")).codex_state_root(),
            )
            .with_tmux_runner(FakeTmux::with_sessions(listing))
            .with_live_state_root(self.live_root())
            .with_caches(Arc::clone(&self.caches))
    }

    /// One discovery cycle on top of `prior`, as the daemon runs them.
    fn discover(&self, prior: GraphSnapshot) -> GraphSnapshot {
        discover_local_warm_with(
            [self.temp.path().join("scan")],
            self.config(),
            prior,
            &crate::config::ServerIntervals::default(),
        )
        .expect("discover")
    }

    fn fingerprint(&self) -> Option<u64> {
        *self
            .caches
            .process_tree_fingerprint
            .lock()
            .expect("fingerprint lock")
    }
}

/// The codex session's node id in `snapshot`.
fn codex_session(snapshot: &GraphSnapshot) -> NodeId {
    snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::AgentSession(session) if session.id.session_key == LiveWorld::SESSION => {
                Some(node.id())
            }
            _ => None,
        })
        .expect("the codex session is discovered")
}

fn this_process() -> crate::live_store::LiveContext {
    let pid = i64::from(std::process::id());
    crate::live_store::LiveContext {
        pid: Some(pid),
        pid_start: crate::live_store::process_start_time(pid),
        ..Default::default()
    }
}

fn in_alpha(pane: &str) -> crate::live_store::LiveContext {
    crate::live_store::LiveContext {
        tmux: Some(crate::live_store::LiveTmux {
            socket: Some("/tmp/tmux-1000/default".to_string()),
            session: Some("alpha".to_string()),
            pane: Some(pane.to_string()),
        }),
        ..Default::default()
    }
}

#[test]
fn discovery_binds_the_live_state_store_to_sessions_and_muxes() {
    use crate::live_store::LiveEventKind;
    use crate::model::live::{LiveState, Liveness};

    let world = LiveWorld::new();
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::TurnStarted,
        this_process(),
    );
    world.event(
        "pi",
        "no-transcript-yet",
        LiveEventKind::TurnStarted,
        in_alpha("%2"),
    );
    world.event(
        "codex",
        "nowhere",
        LiveEventKind::TurnStarted,
        Default::default(),
    );

    let snapshot = world.discover(GraphSnapshot::empty());

    let session = codex_session(&snapshot);
    let table: Vec<(NodeId, LiveState, Liveness)> = snapshot
        .node_live
        .iter()
        .map(|(id, binding)| (id.clone(), binding.observation.state, binding.liveness))
        .collect();
    assert_eq!(
        table,
        vec![(session, LiveState::Working, Liveness::Alive)],
        "one binding per bound session entry"
    );
    let panes: Vec<(crate::model::LivePane, LiveState, Liveness)> = snapshot
        .pane_live
        .iter()
        .map(|(pane, binding)| (pane.clone(), binding.observation.state, binding.liveness))
        .collect();
    assert_eq!(
        panes,
        vec![(
            crate::model::LivePane::new(MuxSessionId::new("tmux:alpha"), Some("%2".into())),
            LiveState::Working,
            Liveness::Unknown
        )],
        "the agent with no session binds to its pane"
    );
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::AgentSession(_)))
            .count(),
        1,
        "an unbound entry creates no session"
    );
}

#[test]
fn every_cycle_rebinds_the_store_without_reopening_the_process_tree_walk() {
    use crate::live_store::LiveEventKind;
    use crate::model::live::LiveState;

    let world = LiveWorld::new();
    let state_of = |snapshot: &GraphSnapshot| {
        snapshot
            .node_live
            .get(&codex_session(snapshot))
            .map(|binding| binding.observation.state)
    };
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::TurnStarted,
        Default::default(),
    );
    let first = world.discover(GraphSnapshot::empty());
    assert_eq!(state_of(&first), Some(LiveState::Working));
    let fingerprint = world.fingerprint();
    assert!(fingerprint.is_some());

    // The turn ends. A cold rebuild (a one-shot command, the daemonless
    // TUI) binds the new state, and the mux and harness slice, which is
    // all the process-tree gate looks at, is as it was.
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::TurnEnded { interrupted: false },
        Default::default(),
    );
    let second = world.discover(GraphSnapshot::empty());
    assert_eq!(state_of(&second), Some(LiveState::Ready));
    assert_eq!(
        world.fingerprint(),
        fingerprint,
        "a state change alone must not reopen the process-tree walk"
    );

    // A cycle on top of the last snapshot, as the daemon runs them,
    // takes the table from the store, not from the prior.
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::TurnStarted,
        Default::default(),
    );
    let third = world.discover(second);
    assert_eq!(state_of(&third), Some(LiveState::Working));

    // An emptied store empties the table on the next cycle, and the
    // session stays.
    std::fs::write(
        world.live_root().join(crate::live_store::LIVE_STATE_FILE),
        r#"{ "schema_version": 1, "rev": 9, "entries": {} }"#,
    )
    .expect("empty the store");
    let fourth = world.discover(third);
    assert!(fourth.node_live.is_empty(), "{:?}", fourth.node_live);
    assert_eq!(state_of(&fourth), None);
}

#[test]
fn discovery_without_a_live_state_root_binds_nothing() {
    use crate::live_store::LiveEventKind;

    let world = LiveWorld::new();
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::TurnStarted,
        Default::default(),
    );
    let snapshot = discover_local_with(
        [world.temp.path().join("scan")],
        world.config().without_live_state(),
    )
    .expect("discover");
    assert!(snapshot.node_live.is_empty());
}

/// A listing of tmux session `alpha` whose active pane `%0` runs Codex
/// under the title `title`.
fn codex_in_alpha(title: &str) -> String {
    format!(
        "alpha\t/work/x\t1700000500\t1700000000\tcodex\t42\t/work/x\tcodex\t0\t\t\t$1\t%0\t\t{title}\n"
    )
}

#[test]
fn discovery_shows_a_codex_pane_waiting_from_its_title_without_hooks() {
    use crate::model::live::{LiveDetail, LiveSource, LiveState};

    let world = LiveWorld::new();
    let listing = codex_in_alpha("[ ! ] Action Required fix-auth | conspectus");
    let snapshot = discover_local_with(
        [world.temp.path().join("scan")],
        world.config_listing(&listing),
    )
    .expect("discover");
    let pane = crate::model::LivePane::new(
        crate::model::MuxSessionId::new("tmux:alpha"),
        Some("%0".to_string()),
    );
    let binding = snapshot
        .pane_live
        .get(&pane)
        .expect("the pane's title binds");
    assert_eq!(
        (
            binding.observation.state,
            binding.observation.detail,
            binding.observation.source
        ),
        (
            LiveState::Waiting,
            Some(LiveDetail::Input),
            LiveSource::PaneTitle
        )
    );
    assert!(snapshot.node_live.is_empty());

    // Turned off, the probe reads no title and binds nothing.
    let world = LiveWorld::new();
    let snapshot = discover_local_with(
        [world.temp.path().join("scan")],
        world.config_listing(&listing).without_pane_titles(),
    )
    .expect("discover");
    assert!(snapshot.pane_live.is_empty());
    assert!(snapshot.mux_probes["tmux"].titles.is_empty());
}

#[test]
fn two_probes_without_the_marker_clear_a_codex_wait_its_hooks_reported() {
    use crate::live_store::{LiveEventKind, WaitKind};
    use crate::model::live::{LiveSource, LiveState};

    let world = LiveWorld::new();
    let mut context = this_process();
    context.tmux = in_alpha("%0").tmux;
    world.event(
        "codex",
        LiveWorld::SESSION,
        LiveEventKind::WaitStarted {
            agent: None,
            kind: WaitKind::Permission,
        },
        context,
    );
    let listing = codex_in_alpha("⠋ fix-auth | conspectus");
    let cycle = || {
        std::thread::sleep(std::time::Duration::from_millis(2));
        world.caches.clear_mux();
        discover_local_warm_with(
            [world.temp.path().join("scan")],
            world.config_listing(&listing),
            GraphSnapshot::empty(),
            &crate::config::ServerIntervals::default(),
        )
        .expect("discover")
    };
    let read = |snapshot: &GraphSnapshot| {
        let binding = snapshot.node_live[&codex_session(snapshot)];
        (binding.observation.state, binding.observation.source)
    };

    // The first probe after the wait could have beaten Codex's redraw.
    let first = cycle();
    assert_eq!(read(&first), (LiveState::Waiting, LiveSource::Hook));
    // The second confirms it: the approval was answered.
    let second = cycle();
    assert_eq!(read(&second), (LiveState::Working, LiveSource::PaneTitle));
    assert!(second.pane_live.is_empty(), "the agent counts once");
}

#[test]
fn agents_of_two_harnesses_in_split_panes_of_one_tmux_session_are_both_attributed() {
    use crate::discovery::tmux::{FakeTmux, TmuxOutcome};
    use crate::hook::{HookRecord, HookStore, HookTmuxRecord, SCHEMA_VERSION};
    use crate::model::{GraphNode, LinkState, NodeId, RelationKind};

    let temp = tempfile::TempDir::new().expect("temp dir");
    let hooks = temp.path().join("hooks");
    let record = |harness: &str, session: &str, pane: &str| HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: harness.to_string(),
        session_key: session.to_string(),
        cwd: Some("/work/x".to_string()),
        pid: Some(i64::from(std::process::id())),
        ppid: None,
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some(pane.to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: 1_700_000_100,
        harness_version: None,
        background_harness: false,
    };
    let store = HookStore::new(&hooks);
    store
        .write_record(&record("codex", "cx-left", "%1"))
        .expect("write the codex record");
    store
        .write_record(&record("claude-code", "cc-right", "%2"))
        .expect("write the claude record");

    // Codex is in the active pane %1, Claude Code in pane %2 beside it.
    let tmux = FakeTmux::with_sessions(
        "editor\t/work/x\t1700000500\t1700000000\tcodex\t42\t/work/x\tcodex\t0\t\t\t$1\t%1\n",
    )
    .with_panes(
        None,
        TmuxOutcome::Sessions("%1\teditor\tcodex\n%2\teditor\tclaude\n".into()),
    );
    std::fs::create_dir(temp.path().join("scan")).expect("scan dir");
    let snapshot = discover_local_with(
        [temp.path().join("scan")],
        LocalDiscoveryConfig::empty()
            .with_tmux_runner(tmux)
            .with_hook_sidecar_root(&hooks),
    )
    .expect("discover");
    let snapshot = crate::resolve::resolve_snapshot(snapshot);

    let editor = NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:editor"));
    let mut hosted: Vec<String> = snapshot
        .resolved_relationships
        .iter()
        .filter(|rel| {
            rel.relation == RelationKind::LinkedToMux
                && rel.target == editor
                && rel.selected_link_id.is_some()
        })
        .filter_map(|rel| match snapshot.find_node(&rel.source) {
            Some(GraphNode::AgentSession(session)) => Some(session.id.session_key.clone()),
            _ => None,
        })
        .collect();
    hosted.sort();
    assert_eq!(
        hosted,
        ["cc-right", "cx-left"],
        "{:#?}",
        snapshot.candidate_links
    );
    assert!(
        snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .all(|link| matches!(link.state, LinkState::Active))
    );
}
