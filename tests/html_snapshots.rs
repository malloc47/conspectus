//! Tests for `conspectus graph --format html` (GV-003a).
//!
//! Two kinds of coverage:
//!   1. Payload determinism — render the page, extract the embedded
//!      JSON, snapshot the parsed-and-canonicalized form. Locks in
//!      the library-neutral payload contract that the Cytoscape
//!      driver (and any future driver, per ADR 0050 Coupling
//!      Boundary) consumes.
//!   2. HTML scaffold — render the page, redact every inlined
//!      `<script>` and `<style>` block to a `[redacted N bytes]`
//!      marker so the snapshot stays stable across CSS tweaks and
//!      Cytoscape / fcose / cose-base / layout-base version bumps.
//!      A byte-count change still surfaces in the diff, so bundle
//!      drift isn't completely invisible.

mod support;

use conspectus::output::dot::Inclusion;
use conspectus::output::{HtmlOptions, render_graph_html};
use serde_json::Value;

use support::fixtures;

#[test]
fn empty_payload_snapshot() {
    assert_payload_snapshot(
        "empty_payload",
        fixtures::empty_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn mux_candidates_payload_snapshot() {
    assert_payload_snapshot(
        "mux_candidates_payload",
        fixtures::mux_candidates_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn unresolved_lineage_payload_snapshot() {
    assert_payload_snapshot(
        "unresolved_lineage_payload",
        fixtures::unresolved_lineage_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn branch_pr_payload_snapshot() {
    assert_payload_snapshot(
        "branch_pr_payload",
        fixtures::branch_pr_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn fork_ancestry_payload_snapshot() {
    assert_payload_snapshot(
        "fork_ancestry_payload",
        fixtures::fork_ancestry_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn mux_candidates_exclude_candidates_payload_snapshot() {
    assert_payload_snapshot(
        "mux_candidates_exclude_candidates_payload",
        fixtures::mux_candidates_graph(),
        HtmlOptions {
            candidates: Inclusion::Exclude,
            diagnostic_nodes: Inclusion::Include,
        },
    );
}

/// One scaffold snapshot. The five inlined `<script>` JS bundles
/// and the inline `<style>` block are redacted so library bumps
/// (Cytoscape, fcose, cose-base, layout-base) and CSS tweaks don't
/// invalidate the snapshot.
#[test]
fn html_scaffold_snapshot() {
    let html =
        render_graph_html(&fixtures::empty_graph(), HtmlOptions::default()).expect("render html");
    let redacted = redact_for_snapshot(&html);
    insta::assert_snapshot!("html_scaffold", redacted);
}

#[test]
fn payload_is_self_contained_and_well_formed() {
    let html = render_graph_html(&fixtures::mux_candidates_graph(), HtmlOptions::default())
        .expect("render html");
    let payload = extract_payload(&html);
    let v: Value = serde_json::from_str(&payload).expect("payload is JSON");
    assert_eq!(v["version"], 1);
    assert!(v["nodes"].is_array());
    assert!(v["edges"].is_array());
    assert!(v["unresolved_stubs"].is_array());
}

#[test]
fn ignored_and_overridden_payload_snapshot() {
    // Covers state="ignored" and state="overridden" in the payload
    // so the inspector/filter chrome have realistic data to render.
    assert_payload_snapshot(
        "ignored_and_overridden_payload",
        fixtures::ignored_and_overridden_graph(),
        HtmlOptions::default(),
    );
}

#[test]
fn rendered_html_contains_chrome_containers() {
    // Structural check that the GV-003b chrome slots are present
    // in the rendered scaffold (filter sidebar, inspector sidebar,
    // search input). Catches accidental template breakage without
    // tying the snapshot to chrome HTML layout details.
    let html = render_graph_html(&fixtures::empty_graph(), HtmlOptions::default()).expect("render");
    assert!(html.contains("id=\"conspectus-left\""));
    assert!(html.contains("id=\"conspectus-right\""));
    assert!(html.contains("id=\"conspectus-search\""));
    assert!(html.contains("ConspectusFilterPanel"));
    assert!(html.contains("ConspectusInspector"));
    assert!(html.contains("ConspectusGraphDriver"));
}

#[test]
fn diagnostic_nodes_filter_drops_runtime_process_from_payload() {
    let world = conspectus::dev_scenarios::materialize("process-cardinality")
        .expect("materialize process-cardinality scenario");
    let snapshot = world.snapshot().expect("snapshot");

    let with = render_graph_html(&snapshot, HtmlOptions::default()).expect("render html");
    let with_payload = extract_payload(&with);
    assert!(with_payload.contains("\"kind\":\"runtime_process\""));

    let without = render_graph_html(
        &snapshot,
        HtmlOptions {
            candidates: Inclusion::Include,
            diagnostic_nodes: Inclusion::Exclude,
        },
    )
    .expect("render html");
    let without_payload = extract_payload(&without);
    assert!(!without_payload.contains("\"kind\":\"runtime_process\""));
    assert!(!without_payload.contains("\"relation\":\"mux_contains_process\""));
}

// -----------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------

fn assert_payload_snapshot(
    name: &str,
    snapshot: conspectus::model::GraphSnapshot,
    opts: HtmlOptions,
) {
    let html = render_graph_html(&snapshot, opts).expect("render html");
    let payload = extract_payload(&html);
    let pretty = pretty_json(&payload);
    insta::assert_snapshot!(name, pretty);
}

fn extract_payload(html: &str) -> String {
    let open_tag = "id=\"conspectus-graph-payload\">";
    let start = html.find(open_tag).expect("payload script tag opens once");
    let after_open = start + open_tag.len();
    let end_rel = html[after_open..]
        .find("</script>")
        .expect("payload script tag closes");
    // Undo the renderer's `<` -> `<` escape so the snapshot
    // shows readable JSON.
    html[after_open..after_open + end_rel].replace("\\u003c", "<")
}

fn pretty_json(raw: &str) -> String {
    let v: Value = serde_json::from_str(raw).expect("payload is JSON");
    serde_json::to_string_pretty(&v).expect("re-serialize")
}

fn redact_for_snapshot(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        // Look for the next inline block we want to redact.
        let style_open = "<style>\n";
        let script_open = "<script>\n";
        let style_at = rest.find(style_open);
        let script_at = rest.find(script_open);
        let pick = match (style_at, script_at) {
            (None, None) => None,
            (Some(a), None) => Some((a, style_open, "</style>")),
            (None, Some(b)) => Some((b, script_open, "</script>")),
            (Some(a), Some(b)) => {
                if a < b {
                    Some((a, style_open, "</style>"))
                } else {
                    Some((b, script_open, "</script>"))
                }
            }
        };
        let (offset, open, close) = match pick {
            Some(t) => t,
            None => {
                out.push_str(rest);
                break;
            }
        };
        // Copy up to and including the opener.
        out.push_str(&rest[..offset + open.len()]);
        let after_open = &rest[offset + open.len()..];
        let close_rel = after_open
            .find(close)
            .expect("matching close tag for redacted block");
        let body = &after_open[..close_rel];
        out.push_str(&format!("[redacted {} bytes]\n", body.len()));
        out.push_str(close);
        rest = &after_open[close_rel + close.len()..];
    }
    out
}
