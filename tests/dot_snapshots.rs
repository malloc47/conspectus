mod support;

use conspectus::output::{DotOptions, Inclusion, render_graph_dot};

use support::fixtures;

#[test]
fn empty_graph_dot_snapshot() {
    assert_dot(
        "empty_graph",
        fixtures::empty_graph(),
        DotOptions::default(),
    );
}

#[test]
fn orphan_session_dot_snapshot() {
    assert_dot(
        "orphan_session",
        fixtures::orphan_session_graph(),
        DotOptions::default(),
    );
}

#[test]
fn mux_candidates_dot_snapshot() {
    // Ambiguous mux: one agent session with multiple linked_to_mux
    // candidates of varying provenance. Exercises resolver-preferred
    // styling vs. dashed losing candidates.
    assert_dot(
        "mux_candidates",
        fixtures::mux_candidates_graph(),
        DotOptions::default(),
    );
}

#[test]
fn unresolved_lineage_dot_snapshot() {
    // Session lineage from a Fork to an unresolved child session.
    // Exercises the dashed-stub rendering.
    assert_dot(
        "unresolved_lineage",
        fixtures::unresolved_lineage_graph(),
        DotOptions::default(),
    );
}

#[test]
fn fork_ancestry_dot_snapshot() {
    assert_dot(
        "fork_ancestry",
        fixtures::fork_ancestry_graph(),
        DotOptions::default(),
    );
}

#[test]
fn branch_pr_dot_snapshot() {
    assert_dot(
        "branch_pr",
        fixtures::branch_pr_graph(),
        DotOptions::default(),
    );
}

#[test]
fn mux_candidates_exclude_candidates_dot_snapshot() {
    // Flag coverage: with --candidates exclude, only the resolver's
    // pick remains and the dashed losers disappear.
    assert_dot(
        "mux_candidates_exclude_candidates",
        fixtures::mux_candidates_graph(),
        DotOptions {
            candidates: Inclusion::Exclude,
            diagnostic_nodes: Inclusion::Include,
        },
    );
}

#[test]
fn process_cardinality_scenario_renders_with_diagnostic_filter() {
    // Named TEST-006 replay scenario. process-cardinality has two
    // RuntimeProcess observations on one mux. Asserted structurally
    // rather than as an insta snapshot because the scenario embeds a
    // unique temp-path (pid+nanos) in node ids per run.
    let world = conspectus::dev_scenarios::materialize("process-cardinality")
        .expect("materialize process-cardinality scenario");
    let snapshot = world.snapshot().expect("snapshot");

    let with_diag = render_graph_dot(&snapshot, DotOptions::default()).expect("render DOT");
    assert!(with_diag.starts_with("digraph conspectus {"));
    assert!(with_diag.contains("cluster_5_agent_session"));
    assert!(with_diag.contains("cluster_6_mux_session"));
    assert!(
        with_diag.contains("cluster_7_runtime_process"),
        "diagnostic-nodes=include should keep the RuntimeProcess cluster"
    );
    assert!(with_diag.contains("mux_contains_process"));

    let without_diag = render_graph_dot(
        &snapshot,
        DotOptions {
            candidates: Inclusion::Include,
            diagnostic_nodes: Inclusion::Exclude,
        },
    )
    .expect("render DOT");
    assert!(
        !without_diag.contains("cluster_7_runtime_process"),
        "diagnostic-nodes=exclude should drop the RuntimeProcess cluster"
    );
    assert!(
        !without_diag.contains("mux_contains_process"),
        "edges touching RuntimeProcess nodes should be dropped"
    );
    // The agent_session and mux_session clusters remain.
    assert!(without_diag.contains("cluster_5_agent_session"));
    assert!(without_diag.contains("cluster_6_mux_session"));
}

fn assert_dot(name: &str, snapshot: conspectus::model::GraphSnapshot, opts: DotOptions) {
    assert_dot_snapshot(name, &snapshot, opts);
}

fn assert_dot_snapshot(name: &str, snapshot: &conspectus::model::GraphSnapshot, opts: DotOptions) {
    let rendered = render_graph_dot(snapshot, opts).expect("render DOT");
    insta::assert_snapshot!(name, rendered);
}
