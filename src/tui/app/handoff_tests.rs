//! What a return from tmux freezes, what it asks the runtime to check,
//! and how each mux is released on its own (ADR 0201).

use super::*;
use crate::model::{
    AgentSessionNode, Confidence, Freshness, GraphLink, LinkEndpoint, LinkState, MuxSessionNode,
    NodePresence, Provenance, RelationKind, SourceMetadata,
};
use crate::tui::app::{App, Msg, SnapshotHandle};
use crate::tui::rows::RowKind;

fn mux(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

fn session(key: &str) -> AgentSessionId {
    AgentSessionId::new("claude-code", "/state", key)
}

/// A running mux `name`, with its tmux session id when `session_id` is
/// set and a creation time either way.
fn mux_node(name: &str, session_id: Option<&str>) -> MuxSessionNode {
    let mut node = MuxSessionNode::new(mux(name), "tmux", name).with_created_epoch(1_700_000_100);
    node.backend_session_id = session_id.map(str::to_string);
    node
}

/// A snapshot of `muxes`, with one agent session per `(key, mux)` pair
/// linked to that mux.
fn world(muxes: Vec<MuxSessionNode>, sessions: &[(&str, &str)]) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .extend(muxes.into_iter().map(GraphNode::MuxSession));
    for (index, (key, mux_name)) in sessions.iter().enumerate() {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(session(key), "claude-code".to_string())
                .with_cwd("/p/proj".to_string()),
        ));
        snapshot.candidate_links.push(GraphLink {
            id: format!("link-{index}"),
            source: NodeId::AgentSession(session(key)),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux(mux_name)),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    snapshot
}

fn app_on(snapshot: GraphSnapshot) -> App {
    let mut app = App::new(crate::tui::RunConfig::defaults());
    app.update(Msg::SwitchView(crate::tui::View::Mux));
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::new(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_200,
        initial_selection_hint: None,
    });
    app
}

fn returned_checks(app: &mut App) -> Vec<ReturnedMux> {
    app.take_handoff_refresh_request()
        .expect("the return asks for a refresh")
        .returned
}

#[test]
fn a_return_from_a_running_mux_asks_to_check_the_incarnation_it_had() {
    let mut node = mux_node("work", Some("$4"));
    node.socket = Some("lab".to_string());
    let mut app = app_on(world(vec![node], &[]));

    app.update(Msg::HandoffReturned(mux("work")));

    assert_eq!(
        returned_checks(&mut app),
        vec![ReturnedMux {
            mux: mux("work"),
            check: ReturnCheck::Incarnation(MuxIncarnationTarget {
                backend: "tmux".to_string(),
                socket: Some("lab".to_string()),
                session_id: "$4".to_string(),
                created_epoch: 1_700_000_100,
            }),
        }]
    );
}

#[test]
fn a_mux_the_screen_did_not_show_running_has_nothing_to_check() {
    let mut snapshot = world(vec![mux_node("gone", Some("$2"))], &[]);
    snapshot.node_presence.insert(
        NodeId::MuxSession(mux("gone")),
        NodePresence::Gone {
            last_seen: 1_700_000_000,
        },
    );
    let mut app = app_on(snapshot);

    app.update(Msg::HandoffReturned(mux("gone")));
    app.update(Msg::HandoffReturned(mux("brand-new")));

    let checks: Vec<ReturnCheck> = returned_checks(&mut app)
        .into_iter()
        .map(|returned| returned.check)
        .collect();
    assert_eq!(checks, vec![ReturnCheck::New, ReturnCheck::New]);
}

#[test]
fn a_mux_without_a_recorded_incarnation_is_settled_by_its_listing() {
    let mut app = app_on(world(vec![mux_node("work", None)], &[]));

    app.update(Msg::HandoffReturned(mux("work")));

    assert_eq!(
        returned_checks(&mut app)[0].check,
        ReturnCheck::Listed,
        "the check can't tell this mux from a new one with its name"
    );
}

#[test]
fn a_launch_return_is_settled_by_its_listing_even_with_an_incarnation() {
    // A rebuild relaunched what runs in the mux: that the session is
    // still there says nothing about the agent sessions in it.
    let mut app = app_on(world(vec![mux_node("work", Some("$4"))], &[]));

    app.update(Msg::LaunchStarted(mux("work")));
    app.update(Msg::HandoffReturned(mux("work")));

    assert_eq!(returned_checks(&mut app)[0].check, ReturnCheck::Listed);
}

#[test]
fn the_pane_coming_back_into_view_asks_for_a_refresh_that_returned_from_no_mux() {
    let mut app = app_on(world(vec![mux_node("work", Some("$4"))], &[]));

    app.update(Msg::ViewRegained);

    assert_eq!(
        app.take_handoff_refresh_request(),
        Some(HandoffRequest {
            returned: Vec::new(),
            regained: true,
        })
    );
    assert_eq!(app.take_handoff_refresh_request(), None, "taken once");
}

#[test]
fn releasing_one_mux_leaves_the_others_and_their_sessions_frozen() {
    let mut app = app_on(world(
        vec![mux_node("a", Some("$1")), mux_node("b", Some("$2"))],
        &[("sa", "a"), ("sb", "b")],
    ));
    app.update(Msg::HandoffReturned(mux("a")));
    app.update(Msg::HandoffReturned(mux("b")));
    assert!(app.take_handoff_refresh_request().is_some());

    app.update(Msg::HandoffReleased(mux("a")));

    let handoff = app.handoff().expect("b is still frozen");
    assert_eq!(handoff.muxes(), BTreeSet::from([mux("b")]));
    assert_eq!(handoff.sessions(), BTreeSet::from([session("sb")]));
    let frozen = |app: &App, kind: fn(&RowKind) -> bool| {
        app.tree()
            .rows
            .iter()
            .filter(|row| kind(&row.kind))
            .filter(|row| app.row_is_frozen(row))
            .count()
    };
    assert_eq!(
        frozen(&app, |kind| matches!(kind, RowKind::MuxSession(_))),
        1
    );

    app.update(Msg::HandoffReleased(mux("b")));
    assert!(
        app.handoff().is_some(),
        "the hand-off lasts until its refresh lands, for the entity to follow"
    );
    assert!(app.handoff().unwrap().muxes().is_empty());
    assert_eq!(
        frozen(&app, |kind| matches!(
            kind,
            RowKind::MuxSession(_) | RowKind::AgentSession(_)
        )),
        0
    );

    app.update(Msg::HandoffSettled);
    assert!(app.handoff().is_none());
}

#[test]
fn a_released_mux_accepts_actions_before_its_refresh_lands() {
    let mut app = app_on(world(vec![mux_node("work", Some("$4"))], &[]));
    let row = app
        .visible_rows()
        .iter()
        .find(|row| matches!(row.kind, RowKind::MuxSession(_)))
        .map(|row| row.id.clone())
        .expect("mux row");
    app.set_selection(row);
    app.update(Msg::HandoffReturned(mux("work")));
    assert!(app.take_handoff_refresh_request().is_some());
    assert!(app.selected_row_awaits_handoff());

    app.update(Msg::HandoffReleased(mux("work")));

    assert!(!app.selected_row_awaits_handoff());
    let effects = app.update(Msg::AttachSelected);
    assert!(
        matches!(effects.as_slice(), [crate::tui::Effect::Exec(_)]),
        "{effects:?}"
    );
}

#[test]
fn a_new_return_to_a_released_mux_freezes_it_again() {
    let mut app = app_on(world(vec![mux_node("work", Some("$4"))], &[]));
    app.update(Msg::HandoffReturned(mux("work")));
    assert!(app.take_handoff_refresh_request().is_some());
    app.update(Msg::HandoffReleased(mux("work")));

    app.update(Msg::HandoffReturned(mux("work")));

    assert!(app.handoff().unwrap().holds_mux(&mux("work")));
    assert_eq!(returned_checks(&mut app).len(), 1, "one check per return");
}
