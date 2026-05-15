mod support;

use conspectus::output::render_graph_json;

use support::fixtures;

#[test]
fn empty_graph_json_snapshot() {
    assert_snapshot("empty_graph", fixtures::empty_graph());
}

#[test]
fn orphan_session_json_snapshot() {
    assert_snapshot("orphan_session", fixtures::orphan_session_graph());
}

#[test]
fn mux_only_json_snapshot() {
    assert_snapshot("mux_only", fixtures::mux_only_graph());
}

#[test]
fn repo_only_json_snapshot() {
    assert_snapshot("repo_only", fixtures::repo_only_graph());
}

#[test]
fn unresolved_lineage_json_snapshot() {
    assert_snapshot("unresolved_lineage", fixtures::unresolved_lineage_graph());
}

#[test]
fn conflict_json_snapshot() {
    assert_snapshot("conflict", fixtures::conflict_graph());
}

#[test]
fn mux_candidates_json_snapshot() {
    assert_snapshot("mux_candidates", fixtures::mux_candidates_graph());
}

fn assert_snapshot(name: &str, snapshot: conspectus::model::GraphSnapshot) {
    let rendered = render_graph_json(&snapshot).expect("render graph fixture");

    insta::assert_snapshot!(name, rendered);
}
