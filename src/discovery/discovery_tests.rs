// Extracted from mod.rs H-HYG-011 rolling wave via #[path = "discovery_tests.rs"] mod tests;
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
        },
        GraphFragment {
            nodes: vec![node],
            candidate_links: vec![link.clone()],
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
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
        },
        GraphFragment {
            nodes: vec![mux_node, agent_node],
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: prov_b,
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
    // H-SERVE-PERF-001a (ADR 0091): the process-tree pass is
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

    let error =
        DiscoveryContext::from_roots([missing]).expect_err("missing roots should be rejected");

    assert!(error.to_string().contains("scan root does not exist"));
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
            .any(|pin| pin.id == "observed" && pin.provenance == Provenance::LocalPin),
        "project pin should load from the observed session cwd, not only the scan root"
    );
}

// ---------------------------------------------------------------------------
// H-SERVE-PERF-003: mux/harness slice fingerprint + /proc walk gate.
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
    // H-SERVE-PERF-008: MuxSessionNode.activity_epoch bumps on
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
    // H-SERVE-PERF-008 follow-up: even with last_active_epoch
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
