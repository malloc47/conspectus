//! Showcase scenario coverage pin (ADR 0070).
//!
//! Asserts that:
//!
//! 1. The programmatic `dev_scenarios::materialize("showcase")` path
//!    builds without error and produces a resolved snapshot covering
//!    the broad layer set the ADR documents (workspaces, three+ agent
//!    harnesses, forks, PRs, mux, lineage chain).
//! 2. The checked-in fixture under `tests/fixtures/showcase.json`
//!    deserializes through `GraphSnapshot::Deserialize` and carries
//!    the same shape, so it stays a valid input for
//!    `conspectus tui --fixture` / `--snapshot-fixture`.
//!
//! When this test fails after a showcase change, re-run
//! `just regen-showcase-fixture` to refresh the JSON.

use std::collections::BTreeSet;

use conspectus::model::{GraphSnapshot, RelationKind};

#[cfg(debug_assertions)]
#[test]
fn showcase_scenario_materializes_with_comprehensive_coverage() {
    let world = conspectus::dev_scenarios::materialize("showcase").expect("showcase materializes");
    let snapshot = world.snapshot().expect("showcase snapshot resolves");

    assert_layer_coverage(&snapshot);
}

#[test]
fn showcase_fixture_parses_and_matches_layer_shape() {
    let raw = std::fs::read_to_string("tests/fixtures/showcase.json")
        .expect("checked-in showcase fixture present");
    let snapshot: GraphSnapshot =
        serde_json::from_str(&raw).expect("showcase fixture deserializes");

    assert_layer_coverage(&snapshot);
}

fn assert_layer_coverage(snapshot: &GraphSnapshot) {
    use conspectus::model::GraphNode;

    let mut node_kinds: BTreeSet<&'static str> = BTreeSet::new();
    let mut workspaces_by_provider: BTreeSet<String> = BTreeSet::new();
    let mut harnesses: BTreeSet<String> = BTreeSet::new();
    let mut has_forge_pr = false;
    let mut has_fork = false;
    let mut has_mux = false;

    for node in &snapshot.nodes {
        match node {
            GraphNode::Repo(_) => {
                node_kinds.insert("repo");
            }
            GraphNode::Checkout(_) => {
                node_kinds.insert("checkout");
            }
            GraphNode::Branch(_) => {
                node_kinds.insert("branch");
            }
            GraphNode::Workspace(ws) => {
                node_kinds.insert("workspace");
                if let Some(provider) = ws.provider.as_deref() {
                    workspaces_by_provider.insert(provider.to_string());
                }
            }
            GraphNode::AgentSession(session) => {
                node_kinds.insert("agent_session");
                harnesses.insert(session.harness_key.clone());
            }
            GraphNode::MuxSession(_) => {
                node_kinds.insert("mux_session");
                has_mux = true;
            }
            GraphNode::ForgePr(_) => {
                node_kinds.insert("forge_pr");
                has_forge_pr = true;
            }
            GraphNode::Fork(_) => {
                node_kinds.insert("fork");
                has_fork = true;
            }
            _ => {}
        }
    }

    let required_kinds = [
        "repo",
        "checkout",
        "branch",
        "workspace",
        "agent_session",
        "mux_session",
        "forge_pr",
        "fork",
    ];
    for kind in required_kinds {
        assert!(
            node_kinds.contains(kind),
            "showcase missing `{kind}` node kind; have: {node_kinds:?}",
        );
    }

    for expected in ["atelier", "agent-deck"] {
        assert!(
            workspaces_by_provider.contains(expected),
            "showcase missing workspace provider `{expected}`; have: {workspaces_by_provider:?}",
        );
    }

    for expected in ["codex", "claude-code", "opencode"] {
        assert!(
            harnesses.contains(expected),
            "showcase missing harness `{expected}`; have: {harnesses:?}",
        );
    }

    assert!(has_forge_pr, "showcase missing forge_pr coverage");
    assert!(has_fork, "showcase missing fork coverage");
    assert!(has_mux, "showcase missing mux coverage");

    // Lineage relation: codex parent → child fork chain.
    let has_lineage = snapshot
        .candidate_links
        .iter()
        .any(|link| link.relation == RelationKind::ParentSession);
    assert!(
        has_lineage,
        "showcase missing `parent_session` lineage link",
    );

    // Resolver actually produced output.
    assert!(
        !snapshot.resolved_relationships.is_empty(),
        "showcase resolver produced no relationships",
    );
}
