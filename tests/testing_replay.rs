mod support;

use conspectus::hook::{HookRecord, HookTmuxRecord, SCHEMA_VERSION};
use conspectus::model::{GraphLink, LinkState, RelationKind};
use conspectus::tui::rows::{MuxIndicator, RowKind};
use support::replay::{ReplayWorld, TmuxReplayRow};

#[test]
fn replay_empty_world_has_empty_graph_and_sessions_tree() {
    let world = ReplayWorld::new();

    let result = world.run();

    assert!(result.snapshot.nodes.is_empty());
    assert!(result.snapshot.candidate_links.is_empty());
    assert!(result.resolved.resolved_relationships.is_empty());
    assert!(result.sessions.rows.is_empty());
}

#[test]
fn replay_links_harness_session_to_fake_tmux_and_projects_rows() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    world.write_codex_session("session-x", &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_activity(1_700_000_500),
    );

    let result = world.run();

    assert!(
        linked_to_mux(&result.snapshot)
            .any(|link| link.source_metadata.evidence.as_deref() == Some("exact_cwd_match")),
        "expected cwd replay to emit a session-to-mux candidate: {:#?}",
        result.snapshot.candidate_links
    );
    assert!(
        result.sessions.rows.iter().any(|row| matches!(
            &row.kind,
            RowKind::AgentSession(session)
                if session.session.session_key == "session-x"
                    && session.mux_state == MuxIndicator::Attached
        )),
        "expected sessions row tree to show the replayed session as attached: {:#?}",
        result.sessions.rows
    );
}

#[test]
fn replay_writes_hook_sqlite_records_into_discovery_pipeline() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    world.write_claude_code_session("current", &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane("claude", 123, &work, "claude --resume old"),
    );
    world.write_hook_record(HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: "current".to_string(),
        cwd: Some(work.to_string_lossy().to_string()),
        pid: Some(123),
        ppid: Some(456),
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: 1_700_000_550,
        harness_version: Some("1.0.0".to_string()),
    });

    let result = world.run();

    assert!(
        linked_to_mux(&result.snapshot).any(|link| {
            link.source_metadata.adapter == "hook_sidecar"
                && link.source_metadata.evidence.as_deref() == Some("hook_session_match")
        }),
        "expected hook sidecar evidence to flow through replay: {:#?}",
        result.snapshot.candidate_links
    );
}

#[test]
fn replay_injects_active_pane_fd_evidence_without_real_proc() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let session_key = "11111111-2222-3333-4444-555555555555";
    world.write_codex_session(session_key, &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane("codex", 4242, &work, "codex resume stale-session"),
    );
    world.add_fd_paths(
        4242,
        [format!(
            "{}/.codex/sessions/2026/05/24/rollout-{session_key}.jsonl",
            world.root().display()
        )],
    );

    let result = world.run();
    let fd_link = linked_to_mux(&result.snapshot)
        .find(|link| {
            link.source_metadata.evidence.as_deref() == Some("active_pane_fd_session_match")
        })
        .expect("fd replay link");

    assert!(
        result.resolved.resolved_relationships.iter().any(|rel| {
            rel.relation == RelationKind::LinkedToMux && rel.selected_link_id == fd_link.id
        }),
        "fd evidence should win resolution: {:#?}",
        result.resolved.resolved_relationships
    );
}

#[test]
fn replay_normalizes_temp_paths_for_stable_snapshots() {
    let world = ReplayWorld::new();
    let raw = format!(
        "{}/work\n{}",
        world.root().display(),
        world.hook_root().display()
    );

    assert_eq!(world.normalize(raw), "/fixture/work\n/fixture/hooks");
}

fn linked_to_mux(snapshot: &conspectus::model::GraphSnapshot) -> impl Iterator<Item = &GraphLink> {
    snapshot.candidate_links.iter().filter(|link| {
        link.relation == RelationKind::LinkedToMux && matches!(link.state, LinkState::Active)
    })
}
