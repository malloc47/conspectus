mod support;

use std::collections::{BTreeSet, HashMap};

use conspectus::hook::{HookRecord, HookTmuxRecord, SCHEMA_VERSION};
use conspectus::model::{GraphLink, GraphSnapshot, LinkState, NodeId, RelationKind};
use conspectus::resolve::resolve_snapshot;
use conspectus::tui::rows::{MuxIndicator, RowId, RowKind};
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
fn replay_write_snapshot_fixture_round_trips_through_snapshot_tool_format() {
    // ReplayWorld can drop a normalized JSON the snapshot tool's
    // `--snapshot-fixture` / `--fixture` paths accept — bridges
    // discovery-test scaffolding to renderer iteration.
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    world.write_codex_session("session-x", &work);
    world.add_tmux_row(TmuxReplayRow::new("editor").with_cwd(&work));

    let temp = tempfile::tempdir().expect("temp dir");
    let fixture_path = temp.path().join("replay.json");
    world
        .write_snapshot_fixture(&fixture_path)
        .expect("write fixture");

    let raw = std::fs::read_to_string(&fixture_path).expect("read fixture");
    assert!(
        !raw.contains(&world.root().display().to_string()),
        "fixture must not leak the replay temp path; got:\n{raw}",
    );
    assert!(
        raw.contains("/fixture"),
        "normalize should rewrite the temp root to `/fixture`; got:\n{raw}",
    );
    let snapshot: GraphSnapshot =
        serde_json::from_str(&raw).expect("fixture parses as GraphSnapshot");
    assert!(
        !snapshot.nodes.is_empty(),
        "exported snapshot should carry replay nodes",
    );
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

/// A PID that exists on any host. Hook sidecar links stay active only
/// while the recorded process is live in `/proc`, so a fixed PID makes
/// the outcome depend on what else the machine is running.
fn live_pid() -> i64 {
    i64::from(std::process::id())
}

#[test]
fn replay_writes_hook_spool_records_into_discovery_pipeline() {
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
        pid: Some(live_pid()),
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
            rel.relation == RelationKind::LinkedToMux
                && rel.selected_link_id.as_deref() == Some(fd_link.id.as_str())
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

fn linked_to_mux(snapshot: &GraphSnapshot) -> impl Iterator<Item = &GraphLink> {
    snapshot.candidate_links.iter().filter(|link| {
        link.relation == RelationKind::LinkedToMux && matches!(link.state, LinkState::Active)
    })
}

fn find_session_row<'a>(
    result: &'a support::replay::ReplayResult,
    session_key: &str,
) -> Option<&'a conspectus::tui::rows::AgentSessionRow> {
    result.sessions.rows.iter().find_map(|row| match &row.kind {
        RowKind::AgentSession(session) if session.session.session_key == session_key => {
            Some(session)
        }
        _ => None,
    })
}

// --- Drift and stale-evidence replays ------------------------------------

#[test]
fn same_pane_hook_supersession_freshest_wins_and_tui_shows_active() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let session_a = "aaaaaaaa-1111-2222-3333-444444444444";
    let session_b = "bbbbbbbb-1111-2222-3333-444444444444";

    world.write_claude_code_session(session_a, &work);
    world.write_claude_code_session(session_b, &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane("claude", 123, &work, "claude"),
    );

    let make_hook = |session_key: &str, observed_epoch: i64| HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: session_key.to_string(),
        cwd: Some(work.to_string_lossy().to_string()),
        pid: Some(live_pid()),
        ppid: Some(456),
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch,
        harness_version: Some("1.0.0".to_string()),
    };

    world.write_legacy_hook_record("older", &make_hook(session_a, 1_700_000_500));
    world.write_legacy_hook_record("newer", &make_hook(session_b, 1_700_000_600));

    let result = world.run();

    let active_hook_links: Vec<_> = result
        .snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.source_metadata.adapter == "hook_sidecar"
                && link.relation == RelationKind::LinkedToMux
                && matches!(link.state, LinkState::Active)
        })
        .collect();
    assert_eq!(
        active_hook_links.len(),
        1,
        "exactly one active hook-sidecar LinkedToMux link expected, got {active_hook_links:?}"
    );

    let overridden_hook_links: Vec<_> = result
        .snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.source_metadata.adapter == "hook_sidecar"
                && link.relation == RelationKind::LinkedToMux
                && matches!(link.state, LinkState::Overridden { .. })
        })
        .collect();
    assert_eq!(
        overridden_hook_links.len(),
        1,
        "exactly one overridden hook-sidecar LinkedToMux link expected, got {overridden_hook_links:?}"
    );

    let session_b_row =
        find_session_row(&result, session_b).expect("session B should appear in row tree");
    assert_eq!(
        session_b_row.mux_state,
        MuxIndicator::Attached,
        "session B (fresher hook) should be Attached"
    );

    assert_at_most_one_active_hook_link_per_mux_pane(&result.snapshot);
}

#[test]
fn codex_fd_evidence_beats_stale_argv_and_tui_follows_current_rollout() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let session_current = "b0000000-1111-2222-3333-444444444444";

    world.write_codex_session(session_current, &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane(
                "codex",
                4242,
                &work,
                "codex resume a0000000-1111-2222-3333-444444444444",
            ),
    );
    world.add_fd_paths(
        4242,
        [format!(
            "{}/.codex/sessions/2026/05/26/rollout-{session_current}.jsonl",
            world.root().display()
        )],
    );

    let result = world.run();

    let fd_link = linked_to_mux(&result.snapshot)
        .find(|link| {
            link.source_metadata.evidence.as_deref() == Some("active_pane_fd_session_match")
        })
        .expect("fd evidence link must exist");

    assert!(
        result.resolved.resolved_relationships.iter().any(|rel| {
            rel.relation == RelationKind::LinkedToMux
                && rel.selected_link_id.as_deref() == Some(fd_link.id.as_str())
        }),
        "fd evidence should win resolution: {:#?}",
        result.resolved.resolved_relationships
    );

    let session_current_row = find_session_row(&result, session_current)
        .expect("session-current should appear in row tree");
    assert_eq!(
        session_current_row.mux_state,
        MuxIndicator::Attached,
        "session-current (fd evidence) should be Attached"
    );
}

// --- Graph and row-projection invariants ---------------------------------

#[test]
fn invariant_ignored_mux_candidates_remain_evidence_but_never_resolve() {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    world.write_codex_session("ignored-candidate", &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_activity(1_700_000_500),
    );
    let result = world.run();
    let ignored_link_id = linked_to_mux(&result.snapshot)
        .find(|link| link.source_metadata.evidence.as_deref() == Some("exact_cwd_match"))
        .expect("exact cwd candidate")
        .id
        .clone();

    let mut snapshot = result.snapshot;
    let ignored_link = snapshot
        .candidate_links
        .iter_mut()
        .find(|link| link.id == ignored_link_id)
        .expect("ignored candidate still present");
    ignored_link.state = LinkState::Ignored {
        reason: Some("operator rejected fixture match".to_string()),
    };

    let resolved = resolve_snapshot(snapshot);

    assert!(
        resolved.candidate_links.iter().any(|link| {
            link.id == ignored_link_id && matches!(link.state, LinkState::Ignored { .. })
        }),
        "ignored candidate should remain visible as evidence: {:#?}",
        resolved.candidate_links
    );
    assert!(
        !resolved
            .resolved_relationships
            .iter()
            .any(|rel| rel.selected_link_id.as_deref() == Some(ignored_link_id.as_str())),
        "ignored candidate must not be selected: {:#?}",
        resolved.resolved_relationships
    );
}

#[test]
fn invariant_ambiguous_mux_session_renders_as_leaf_after_adr_0071() {
    // ADR 0071 retired the per-session candidate subtree. The
    // ambiguous session still flashes its `◐` chip with the
    // correct candidate count, but the row no longer expands and
    // no `AgentSessionMuxCandidate` rows are emitted — the muxes
    // surface on the shared-ancestor group detail instead.
    //
    // The tree row builder now consumes resolver winners
    // only, so the "two cwd-equal candidates for a single session"
    // case resolves to a single Attached row (the resolver
    // tie-breaks alphabetically). Genuine ambiguity in the tree
    // comes from `suppress_ambiguous_cwd_mux_links`, which fires
    // when ≥2 distinct sessions share the same cwd evidence for
    // the same mux. Build that scenario here so the test exercises
    // ADR 0071's leaf-row behavior on a session that's *actually*
    // ambiguous from the resolver's perspective.
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    world.write_codex_session("ambiguous", &work);
    world.write_codex_session("companion", &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor-a")
            .with_cwd(&work)
            .with_activity(1_700_000_500),
    );
    world.add_tmux_row(
        TmuxReplayRow::new("editor-b")
            .with_cwd(&work)
            .with_activity(1_700_000_550),
    );

    let result = world.run();

    let session_rows: Vec<_> = result
        .sessions
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.session.session_key == "ambiguous" => {
                Some(session)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        session_rows.len(),
        1,
        "one visible session row should represent the ambiguous session"
    );
    assert_eq!(
        session_rows[0].mux_state,
        MuxIndicator::Ambiguous { candidate_count: 2 }
    );

    let candidate_rows: BTreeSet<NodeId> = result
        .sessions
        .rows
        .iter()
        .filter_map(|row| match &row.id {
            RowId::AgentSessionMuxCandidate { agent, mux }
                if agent == &session_rows[0].primary_node =>
            {
                Some(mux.clone())
            }
            _ => None,
        })
        .collect();
    assert!(
        candidate_rows.is_empty(),
        "no candidate child rows after ADR 0071: {:#?}",
        result.sessions.rows
    );
}

#[test]
fn invariant_replay_worlds_have_at_most_one_active_hook_link_per_mux_pane() {
    let worlds = [
        hook_supersession_world().run(),
        codex_fd_beats_stale_argv_world().run(),
    ];

    for result in worlds {
        assert_at_most_one_active_hook_link_per_mux_pane(&result.snapshot);
    }
}

#[test]
fn invariant_stronger_current_session_evidence_beats_launch_history() {
    let result = codex_fd_beats_stale_argv_world().run();
    let fd_link = linked_to_mux(&result.snapshot)
        .find(|link| {
            link.source_metadata.evidence.as_deref() == Some("active_pane_fd_session_match")
        })
        .expect("fd evidence link");

    assert!(
        result.resolved.resolved_relationships.iter().any(|rel| {
            rel.relation == RelationKind::LinkedToMux
                && rel.selected_link_id.as_deref() == Some(fd_link.id.as_str())
        }),
        "stronger fd evidence should be the preferred current-session link: {:#?}",
        result.resolved.resolved_relationships
    );
}

fn assert_at_most_one_active_hook_link_per_mux_pane(snapshot: &GraphSnapshot) {
    let mut active_count: HashMap<(String, Option<String>), usize> = HashMap::new();

    for link in &snapshot.candidate_links {
        if link.source_metadata.adapter != "hook_sidecar" {
            continue;
        }
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if link.relation != RelationKind::LinkedToMux {
            continue;
        }
        let Some(mux_node_id) = link.target_node_id() else {
            continue;
        };
        let mux_id = mux_node_id.to_string();
        let pane_id = link
            .source_metadata
            .fields
            .get("hook_pane_id")
            .or_else(|| link.source_metadata.fields.get("pane_id"))
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);
        let key = (mux_id, pane_id);
        *active_count.entry(key).or_default() += 1;
    }

    for ((mux_id, pane_id), count) in &active_count {
        assert!(
            *count <= 1,
            "at most one active hook-sidecar LinkedToMux per (mux, pane_id): \
             found {count} active links for mux={mux_id:?} pane={pane_id:?}"
        );
    }
}

fn hook_supersession_world() -> ReplayWorld {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let session_a = "aaaaaaaa-1111-2222-3333-444444444444";
    let session_b = "bbbbbbbb-1111-2222-3333-444444444444";

    world.write_claude_code_session(session_a, &work);
    world.write_claude_code_session(session_b, &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane("claude", 123, &work, "claude"),
    );

    for (name, session_key, observed_epoch) in [
        ("older", session_a, 1_700_000_500),
        ("newer", session_b, 1_700_000_600),
    ] {
        world.write_legacy_hook_record(
            name,
            &HookRecord {
                schema_version: SCHEMA_VERSION,
                harness_key: "claude-code".to_string(),
                session_key: session_key.to_string(),
                cwd: Some(work.to_string_lossy().to_string()),
                pid: Some(live_pid()),
                ppid: Some(456),
                tmux: Some(HookTmuxRecord {
                    session_name: Some("editor".to_string()),
                    native_id: None,
                    pane_id: Some("%1".to_string()),
                    socket_path: None,
                }),
                transcript_path: None,
                hook_event_name: Some("SessionStart".to_string()),
                observed_epoch,
                harness_version: Some("1.0.0".to_string()),
            },
        );
    }

    world
}

fn codex_fd_beats_stale_argv_world() -> ReplayWorld {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let session_current = "b0000000-1111-2222-3333-444444444444";

    world.write_codex_session(session_current, &work);
    world.add_tmux_row(
        TmuxReplayRow::new("editor")
            .with_cwd(&work)
            .with_active_pane(
                "codex",
                4242,
                &work,
                "codex resume a0000000-1111-2222-3333-444444444444",
            ),
    );
    world.add_fd_paths(
        4242,
        [format!(
            "{}/.codex/sessions/2026/05/26/rollout-{session_current}.jsonl",
            world.root().display()
        )],
    );

    world
}
