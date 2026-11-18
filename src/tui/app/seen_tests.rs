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
use crate::tui::rows::seen::{ClientReturn, SEEN_REST};
use crate::tui::rows::{LiveMark, LiveSign, MuxLive, RowKind};

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

/// An agent that stopped in `state` at `at_ms`, its process found running.
fn stopped(state: LiveState, detail: LiveDetail, at_ms: i64) -> LiveBinding {
    LiveBinding::new(
        LiveObservation::new(state, LiveSource::Hook, at_ms).with_detail(detail),
        Liveness::Alive,
    )
}

/// `hosted`, in the mux `work` (a client attached or not, last attached
/// at `last_attached`), and `loose`, in no mux; each ready since its
/// turn ended at the time given.
fn world(attached: bool, last_attached: Option<i64>, hosted: i64, loose: i64) -> GraphSnapshot {
    world_of(
        (LiveState::Ready, LiveDetail::Finished),
        attached,
        last_attached,
        hosted,
        loose,
    )
}

/// [`world`] with both sessions stopped in `stop` (a state and its detail)
/// instead of ready.
fn world_of(
    stop: (LiveState, LiveDetail),
    attached: bool,
    last_attached: Option<i64>,
    hosted: i64,
    loose: i64,
) -> GraphSnapshot {
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
        (session("hosted"), stopped(stop.0, stop.1, hosted)),
        (session("loose"), stopped(stop.0, stop.1, loose)),
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
    assert_eq!(mux_live(&app).unseen.ready, 1);
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
    assert_eq!(mux_live(&app).unseen.ready, 0);

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
fn reading_a_stop_adds_its_mark_to_the_state_file_and_changes_no_other_value() {
    let mut app = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    app.set_selection(RowId::AgentSession(session("loose")));
    let before = app.build_persisted_state();
    assert!(before.read_marks.is_empty(), "nothing is read yet");
    let rows_before: Vec<_> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    assert!(!unseen(&app, "loose"));
    let after = app.build_persisted_state();
    assert_eq!(
        after.read_marks.values().copied().collect::<Vec<_>>(),
        [ENDED_MS],
        "the state file would hold the mark: the stop that was read"
    );
    assert_eq!(
        crate::tui_state::PersistedState {
            read_marks: before.read_marks.clone(),
            ..after.clone()
        },
        before,
        "and nothing else about it changes"
    );
    let rows_after: Vec<_> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
    assert_eq!(rows_after, rows_before, "the rows stay where they were");
    assert_eq!(
        app.selection(),
        Some(&RowId::AgentSession(session("loose")))
    );
    // A TUI that starts without the file reads it unread again; one that
    // loads the mark does not.
    let mut fresh = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    assert!(unseen(&fresh, "loose"));
    fresh.seen.load_marks(&after.read_marks);
    fresh.settle_seen();
    assert!(!unseen(&fresh, "loose"), "the mark survived the restart");
}

#[test]
fn a_fixture_or_scenario_tui_keeps_its_marks_in_memory() {
    // The marks a state file holds are not taken, and a mark made here is
    // not offered to it: a stop read in one demo run is unread in the next.
    let stopped = world(false, None, ENDED_MS, ENDED_MS);
    let mut reader = app_over(&stopped);
    reader.set_selection(RowId::AgentSession(session("loose")));
    let start = Instant::now();
    reader.note_cursor_rest(start);
    reader.note_cursor_rest(start + SEEN_REST);
    let marks = reader.build_persisted_state().read_marks;
    assert!(!marks.is_empty(), "an ordinary TUI keeps the mark");

    let mut demo = app_over(&stopped);
    demo.keep_read_marks_in_memory();
    demo.seen.load_marks(&marks);
    demo.settle_seen();
    assert!(unseen(&demo, "loose"), "the file's marks are not taken");

    demo.set_selection(RowId::AgentSession(session("loose")));
    let start = Instant::now();
    demo.note_cursor_rest(start);
    demo.note_cursor_rest(start + SEEN_REST);
    assert!(!unseen(&demo, "loose"), "its own marks still read");
    assert!(
        demo.build_persisted_state().read_marks.is_empty(),
        "none is written"
    );
    assert!(!demo.seen.take_changed(), "and none is due a write");
}

#[test]
fn a_mark_is_written_once_when_it_moves_and_a_stop_after_it_is_unread() {
    let mut app = app_over(&world(false, None, ENDED_MS, ENDED_MS));
    assert!(!app.seen.take_changed(), "nothing moved yet");
    app.set_selection(RowId::AgentSession(session("loose")));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    assert!(app.seen.take_changed(), "the mark moved");
    assert!(!app.seen.take_changed(), "and is taken once");
    // A stop that began after the mark is another stop.
    let later = app_over(&world(false, None, ENDED_MS + 1, ENDED_MS + 1));
    let marks = app.build_persisted_state().read_marks;
    let mut later = later;
    later.seen.load_marks(&marks);
    later.settle_seen();
    assert!(unseen(&later, "loose"), "a later stop is unread");
    assert!(
        !later.seen.take_changed(),
        "marks taken from the file are no news to write back"
    );
}

fn client_came_back(session: &str, at_ms: i64) -> Msg {
    Msg::ClientReturned(ClientReturn {
        socket: "/tmp/tmux-1000/default".to_string(),
        session: session.to_string(),
        at_ms,
    })
}

#[test]
fn a_turn_watched_in_an_attached_client_reads_seen_when_the_client_comes_back() {
    // The mux was last attached before the turn ended and no refresh
    // listed a client on it: only the return can say it was watched.
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    assert!(unseen(&app, "hosted"));
    assert_eq!(mux_live(&app).unseen.ready, 1);

    app.update(client_came_back("work", ENDED_MS + 20_000));
    assert!(
        !unseen(&app, "hosted"),
        "the client left the mux after the turn ended"
    );
    assert_eq!(mux_live(&app).unseen.ready, 0, "its mux's mark follows");
    assert!(
        unseen(&app, "loose"),
        "a session in no mux is seen by resting on it"
    );

    // A refresh that lists nothing of the sort keeps it seen.
    load(
        &mut app,
        &world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS),
        LOADED + 30,
    );
    assert!(!unseen(&app, "hosted"), "no flicker back");
}

#[test]
fn a_return_from_another_session_or_before_the_turn_leaves_it_unseen() {
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    app.update(client_came_back("elsewhere", ENDED_MS + 20_000));
    assert!(unseen(&app, "hosted"), "the client was last on `elsewhere`");
    app.update(client_came_back("work", ENDED_MS - 1));
    assert!(
        unseen(&app, "hosted"),
        "the client had left the mux before the turn ended"
    );
    assert_eq!(mux_live(&app).unseen.ready, 1);
}

#[test]
fn a_return_noted_before_the_next_snapshot_still_sees_the_turn_in_it() {
    let mut app = app_over(&world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS));
    app.update(client_came_back("work", ENDED_MS + 20_000));
    // The refresh the regain asks for lands with a newer snapshot of the
    // same turn.
    load(
        &mut app,
        &world(false, Some(LOADED - 600), ENDED_MS, ENDED_MS),
        LOADED + 5,
    );
    assert!(!unseen(&app, "hosted"));
    // A turn that ends later is unseen until it is looked at.
    load(
        &mut app,
        &world(false, Some(LOADED - 600), ENDED_MS + 120_000, ENDED_MS),
        LOADED + 150,
    );
    assert!(unseen(&app, "hosted"), "the client has not been back since");
}

#[test]
fn a_wait_and_a_failure_read_unread_like_a_finished_turn_until_looked_at() {
    for (state, detail, mark) in [
        (
            LiveState::Waiting,
            LiveDetail::Permission,
            LiveMark::Waiting,
        ),
        (LiveState::Ready, LiveDetail::Finished, LiveMark::Ready),
        (LiveState::Error, LiveDetail::Failed, LiveMark::Error),
    ] {
        let stop = (state, detail);
        // The mux was last attached before the stop began, and nobody has
        // looked at the session that is in no mux.
        let mut app = app_over(&world_of(
            stop,
            false,
            Some(LOADED - 600),
            ENDED_MS,
            ENDED_MS,
        ));
        assert!(unseen(&app, "hosted"), "{state:?}: unread in a mux");
        assert!(unseen(&app, "loose"), "{state:?}: unread with no mux");
        let sign = LiveSign::of_mux(mux_live(&app));
        assert_eq!(sign.mark, mark, "{state:?}: the mux's mark is the state's");
        assert!(
            sign.unseen,
            "{state:?}: its mark is bold while any agent is"
        );
        assert_eq!(mux_live(&app).unseen.get(state), 1);

        // A client attached to the mux after the stop: the mux's agent is
        // read, and the mux's mark with it; the loose one is read by
        // resting the cursor on its row.
        load(
            &mut app,
            &world_of(stop, true, Some(LOADED - 600), ENDED_MS, ENDED_MS),
            LOADED + 5,
        );
        assert!(!unseen(&app, "hosted"), "{state:?}: attaching reads it");
        assert!(!LiveSign::of_mux(mux_live(&app)).unseen, "{state:?}");
        assert!(
            unseen(&app, "loose"),
            "{state:?}: attaching reads only that mux"
        );
        app.set_selection(RowId::AgentSession(session("loose")));
        let start = Instant::now();
        app.note_cursor_rest(start);
        app.note_cursor_rest(start + SEEN_REST);
        assert!(!unseen(&app, "loose"), "{state:?}: resting reads it");
    }
}

#[test]
fn working_and_the_states_that_are_no_stop_are_never_unread() {
    for state in [
        LiveState::Working,
        LiveState::Compacting,
        LiveState::Starting,
    ] {
        let mut snapshot = world(false, None, ENDED_MS, ENDED_MS);
        snapshot.node_live = BTreeMap::from([(
            session("loose"),
            LiveBinding::new(
                LiveObservation::new(state, LiveSource::Hook, ENDED_MS),
                Liveness::Alive,
            ),
        )]);
        let app = app_over(&snapshot);
        assert!(!unseen(&app, "loose"), "{state:?} is not a stop");
    }
}
