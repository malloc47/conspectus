use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::*;
use crate::model::live::{
    LiveBinding, LiveDetail, LiveObservation, LiveSource, LiveState, Liveness,
};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, GraphNode, LinkEndpoint, LinkState,
    MuxSessionNode, Provenance, RelationKind,
};
use crate::tui::rows::seen::SEEN_REST;
use crate::tui::rows::{LiveSign, MuxLive, RowKind};

/// When the first snapshot loads, Unix seconds.
const LOADED: i64 = 1_700_000_000;
/// When the turns end: a minute before the first snapshot loads.
const ENDED_MS: i64 = (LOADED - 60) * 1000;

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

fn work() -> MuxSessionId {
    MuxSessionId::new("tmux:work")
}

/// A turn that ended at `ended_ms`, its process found running, so it
/// reads `ready` at any time.
fn ready(ended_ms: i64) -> LiveBinding {
    LiveBinding::new(
        LiveObservation::new(LiveState::Ready, LiveSource::Hook, ended_ms)
            .with_detail(LiveDetail::Finished),
        Liveness::Alive,
    )
}

/// `hosted`, in the mux `work` (a client attached or not, last attached
/// at `last_attached`), and `loose`, in no mux; each ready since its
/// turn ended at the time given.
fn world(attached: bool, last_attached: Option<i64>, hosted: i64, loose: i64) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for key in ["hosted", "loose"] {
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", key),
                "claude-code",
            )));
    }
    let mut mux = MuxSessionNode::new(work(), "tmux", "work");
    mux.client_attached = Some(attached);
    mux.last_attached_epoch = last_attached;
    snapshot.nodes.push(GraphNode::MuxSession(mux));
    let mut link = GraphLink::new(
        "fixture:hosted-in-work".to_string(),
        session("hosted"),
        LinkEndpoint::Node {
            id: NodeId::MuxSession(work()),
        },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    link.state = LinkState::Active;
    snapshot.candidate_links.push(link);
    snapshot.node_live = BTreeMap::from([
        (session("hosted"), ready(hosted)),
        (session("loose"), ready(loose)),
    ]);
    crate::resolve::resolve_snapshot(snapshot)
}

/// Put `snapshot` on screen as the runtime does, loaded at `loaded_at`.
fn load(app: &mut App, snapshot: &GraphSnapshot, loaded_at: i64) {
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        snapshot, app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: loaded_at,
        initial_selection_hint: None,
    });
}

fn app_over(snapshot: &GraphSnapshot) -> App {
    let mut app = App::new(RunConfig::defaults());
    load(&mut app, snapshot, LOADED);
    app
}

/// Whether the row of session `key` shows an unseen turn.
fn unseen(app: &App, key: &str) -> bool {
    app.tree()
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(row) if row.primary_node == session(key) => {
                Some(LiveSign::of_session(row.live).unseen)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no row for {key}"))
}

/// What the row of the mux `work` carries.
fn mux_live(app: &App) -> MuxLive {
    app.tree().live.mux(&work())
}

#[test]
fn a_turn_seen_while_a_client_was_attached_stays_seen_after_it_leaves() {
    // Before the attach: the mux was last attached before the turn
    // ended, and no client is on it.
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    assert!(unseen(&app, "hosted"));
    assert_eq!(mux_live(&app).unseen, 1);
    assert!(
        LiveSign::of_mux(mux_live(&app)).unseen,
        "its mux's › is bold"
    );

    // The refresh after an attach lists a client on the mux; tmux may
    // not have moved its last attach time yet.
    load(
        &mut app,
        &world(true, Some(LOADED - 600), ENDED_MS, ENDED_MS),
        LOADED + 5,
    );
    assert!(!unseen(&app, "hosted"), "a client was on it after the turn");
    assert_eq!(mux_live(&app).unseen, 0);

    // The client left again; the listing no longer says anyone looked.
    load(
        &mut app,
        &world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS),
        LOADED + 30,
    );
    assert!(!unseen(&app, "hosted"), "seen stays seen: no flicker back");

    // A later turn ends after all that: unseen again.
    let later = (LOADED + 60) * 1000;
    load(
        &mut app,
        &world(false, Some(LOADED - 600), later, ENDED_MS),
        LOADED + 90,
    );
    assert!(unseen(&app, "hosted"));
}

#[test]
fn an_attach_after_the_turn_ended_sees_it_through_the_last_attach_time() {
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    assert!(unseen(&app, "hosted"));
    load(
        &mut app,
        &world(false, Some(LOADED + 2), ENDED_MS, ENDED_MS),
        LOADED + 5,
    );
    assert!(!unseen(&app, "hosted"));
    assert!(unseen(&app, "loose"), "a mux's attach sees only its own");
}

#[test]
fn a_listing_older_than_the_turn_does_not_see_it() {
    // The snapshot loaded before the turn ended, so its attached client
    // says nothing about the turn.
    let ended = (LOADED + 10) * 1000;
    let app = app_over(&world(true, Some(LOADED - 600), ended, ended));
    assert!(unseen(&app, "hosted"));
}

#[test]
fn the_cursor_resting_a_second_on_a_session_with_no_mux_sees_its_turn() {
    let mut app = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    app.set_selection(RowId::AgentSession(session("loose")));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST / 2);
    assert!(unseen(&app, "loose"), "half a rest is not a look");
    app.note_cursor_rest(start + SEEN_REST);
    assert!(!unseen(&app, "loose"), "a full rest is");
    assert!(unseen(&app, "hosted"), "only the rested session");

    // A later turn of it is unseen again until the cursor rests anew.
    let later = (LOADED + 60) * 1000;
    load(&mut app, &world(false, None, ENDED_MS, later), LOADED + 90);
    assert!(unseen(&app, "loose"));
}

#[test]
fn a_cursor_passing_through_does_not_see() {
    let mut app = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    let start = Instant::now();
    let tick = Duration::from_millis(100);
    app.set_selection(RowId::AgentSession(session("loose")));
    app.note_cursor_rest(start);
    app.set_selection(RowId::AgentSession(session("hosted")));
    app.note_cursor_rest(start + tick * 5);
    app.set_selection(RowId::AgentSession(session("loose")));
    app.note_cursor_rest(start + tick * 9);
    app.note_cursor_rest(start + tick * 12);
    assert!(
        unseen(&app, "loose"),
        "the rest starts again each time the cursor comes back"
    );
    app.note_cursor_rest(start + tick * 9 + SEEN_REST);
    assert!(!unseen(&app, "loose"));
}

#[test]
fn resting_on_a_session_in_a_mux_does_not_see_it() {
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    app.set_selection(RowId::AgentSession(session("hosted")));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST * 3);
    assert!(
        unseen(&app, "hosted"),
        "a turn in a mux is seen by attaching to it"
    );
}

#[test]
fn seeing_a_turn_persists_nothing_and_changes_no_other_value() {
    let mut app = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    app.set_selection(RowId::AgentSession(session("loose")));
    let before = app.build_persisted_state();
    let rows_before: Vec<_> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    assert!(!unseen(&app, "loose"));
    assert_eq!(
        app.build_persisted_state(),
        before,
        "the state file would be written the same"
    );
    let rows_after: Vec<_> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
    assert_eq!(rows_after, rows_before, "the rows stay where they were");
    assert_eq!(
        app.selection(),
        Some(&RowId::AgentSession(session("loose")))
    );
    // A new TUI remembers nothing.
    let fresh = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    assert!(unseen(&fresh, "loose"));
}
