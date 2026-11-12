//! Live state rides on the rows the same way in every view: a session or
//! a mux carries one live value wherever it is listed, whether the rows
//! read the snapshot's own table or the overlay the live TUI binds from
//! the store. Nothing draws it yet, so carrying it changes no frame.

use std::collections::BTreeMap;
use std::time::Instant;

use super::*;
use crate::live_store::{
    LiveContext, LiveEvent, LiveEventKind, LiveStore, LiveTmux, ProcProbe, WaitKind, now_ms,
};
use crate::model::live::{
    LiveBinding, LiveDetail, LiveObservation, LiveSource, LiveState, Liveness,
};
use crate::model::{GraphLink, LinkEndpoint, LinkState, Provenance, RelationKind};
use crate::tui::live_overlay::{LiveOverlay, SystemFiles};
use crate::tui::rows::{LiveCell, MixedLive, RowKind};

/// The session the world runs in `conspectus-main`, and a second one
/// added beside it so the Mux view lists that mux's agents as rows.
const HOSTED: &str = "e44d01cb5a";
const BESIDE: &str = "f00dfeed01";
/// The world's loose session, which the Search view finds.
const SOLO: &str = "solo000001";

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

fn mux(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

/// The shared world with a second agent in `conspectus-main` and no live
/// state.
fn live_world() -> GraphSnapshot {
    let mut snapshot = world();
    let mut beside = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", BESIDE),
        "claude-code",
    );
    beside.cwd = Some("/home/op/src/conspectus".into());
    beside.title = Some("Review the merge".into());
    beside.last_active_epoch = Some(NOW - 300);
    snapshot.nodes.push(GraphNode::AgentSession(beside));
    let mut hosted = GraphLink::new(
        "fixture:beside-in-main".to_string(),
        session(BESIDE),
        LinkEndpoint::Node {
            id: NodeId::MuxSession(mux("conspectus-main")),
        },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    hosted.state = LinkState::Active;
    snapshot.candidate_links.push(hosted);
    resolve_snapshot(snapshot)
}

/// A binding of `state` whose process was found running: the states
/// chosen do not age while it runs, so the rows read them alike at any
/// time.
fn running(state: LiveState, detail: Option<LiveDetail>, subagents: u16) -> LiveBinding {
    let mut observation = LiveObservation::new(state, LiveSource::Hook, NOW * 1000)
        .with_since_ms(NOW * 1000 - 90_000)
        .with_subagents(subagents);
    if let Some(detail) = detail {
        observation = observation.with_detail(detail);
    }
    LiveBinding::new(observation, Liveness::Alive)
}

/// [`live_world`] with a live table: each session in a state of its own,
/// and an agent no session matched in a pane of `scratch`.
fn with_table() -> GraphSnapshot {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([
        (
            session(HOSTED),
            running(LiveState::Waiting, Some(LiveDetail::Permission), 1),
        ),
        (
            session(BESIDE),
            running(LiveState::Error, Some(LiveDetail::RateLimited), 0),
        ),
        (
            session(SOLO),
            running(LiveState::Ready, Some(LiveDetail::Finished), 0),
        ),
        (
            NodeId::MuxSession(mux("scratch")),
            running(LiveState::Waiting, Some(LiveDetail::Input), 0),
        ),
    ]);
    snapshot
}

/// `view` listing every row it can (History with pinned records, the
/// Search view with its search), its rows reading live state through
/// `overlay`.
fn listing_with(view: View, snapshot: &GraphSnapshot, overlay: LiveOverlay) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = view;
    let mut app = App::new(config);
    app.update(Msg::SwitchView(view));
    app.update(Msg::SetFilter(RowFilter {
        show_pinned_history: true,
        ..RowFilter::default()
    }));
    app.set_live_overlay(overlay);
    // What the runtime does with every snapshot it puts on screen.
    app.rebind_live(snapshot, Instant::now());
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: NOW,
        initial_selection_hint: None,
    });
    if view == View::Search {
        with_search(app)
    } else {
        app
    }
}

/// The entity a row stands for, a pin's row standing for its target.
fn entity_of(snapshot: &GraphSnapshot, node: &NodeId) -> Option<NodeId> {
    match node {
        NodeId::AgentSession(_) | NodeId::MuxSession(_) => Some(node.clone()),
        NodeId::Pin(_) => match snapshot.find_node(node)? {
            GraphNode::Pin(pin) => pin.target_node.clone(),
            _ => None,
        },
        _ => None,
    }
}

/// The live value every session and mux row of `app` carries, by entity.
fn carried(app: &App, snapshot: &GraphSnapshot) -> Vec<(NodeId, MixedLive)> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(row) => Some((
                row.primary_node.clone(),
                row.live.map_or(MixedLive::None, MixedLive::Session),
            )),
            RowKind::MuxSession(row) => Some((row.primary_node.clone(), MixedLive::Mux(row.live))),
            RowKind::Mixed(row) => entity_of(snapshot, &row.primary_node)
                .filter(|node| matches!(node, NodeId::AgentSession(_) | NodeId::MuxSession(_)))
                .map(|node| (node, row.live)),
            _ => None,
        })
        .collect()
}

/// Every view's rows of one entity carry one value, the one the
/// snapshot's accessors give for it.
fn assert_one_value_per_entity(snapshot: &GraphSnapshot, overlay: impl Fn() -> LiveOverlay) {
    let mut seen: BTreeMap<NodeId, Vec<(View, MixedLive)>> = BTreeMap::new();
    for view in ALL_VIEWS {
        let app = listing_with(view, snapshot, overlay());
        for (node, live) in carried(&app, snapshot) {
            seen.entry(node).or_default().push((view, live));
        }
    }
    let now_ms = crate::tui::rows::tree_current_unix_ms();
    let expected = |node: &NodeId| match node {
        NodeId::MuxSession(id) => MixedLive::Mux(snapshot.live_counts_of_mux(id, now_ms)),
        _ => snapshot
            .node_live
            .get(node)
            .map_or(MixedLive::None, |binding| {
                MixedLive::Session(LiveCell::of(binding, now_ms))
            }),
    };
    for (node, values) in &seen {
        for (view, live) in values {
            assert_eq!(
                *live,
                expected(node),
                "{view:?} lists {node} with another live value; every listing: {values:?}"
            );
        }
    }
    // The check means something only if entities are listed in several
    // views and carry state there.
    for (node, views) in [
        (session(HOSTED), 2),
        (session(BESIDE), 2),
        (session(SOLO), 3),
        (NodeId::MuxSession(mux("conspectus-main")), 2),
    ] {
        let listed: std::collections::BTreeSet<View> = seen
            .get(&node)
            .unwrap_or_else(|| panic!("no view lists {node}"))
            .iter()
            .map(|(view, _)| *view)
            .collect();
        assert!(listed.len() >= views, "{node} is listed in {listed:?} only");
        assert!(has_state(expected(&node)), "{node} has live state");
    }
}

fn has_state(live: MixedLive) -> bool {
    match live {
        MixedLive::None => false,
        MixedLive::Session(_) => true,
        MixedLive::Mux(counts) => !counts.is_empty(),
    }
}

#[test]
fn one_entity_carries_one_live_value_in_every_view() {
    let snapshot = with_table();
    assert_one_value_per_entity(&snapshot, LiveOverlay::off);
    assert_eq!(
        snapshot.live_counts_of_mux(&mux("conspectus-main"), NOW * 1000),
        crate::model::live::LiveCounts {
            waiting: 1,
            error: 1,
            ..Default::default()
        },
        "the mux counts both agents it hosts"
    );
}

/// The store [`with_table`]'s table describes, written the way hooks
/// write it, in `dir`.
fn written_store(dir: &std::path::Path) -> LiveStore {
    let store = LiveStore::at(dir.join("conspectus"));
    let at = now_ms();
    let event = |key: &str, kind: LiveEventKind| LiveEvent::new("claude-code", key, at, kind);
    for event in [
        event(
            HOSTED,
            LiveEventKind::WaitStarted {
                agent: None,
                kind: WaitKind::Permission,
            },
        ),
        event(
            BESIDE,
            LiveEventKind::TurnFailed {
                class: crate::live_store::FailureClass::RateLimited,
            },
        ),
        event(SOLO, LiveEventKind::TurnEnded { interrupted: false }),
        event(
            "pane-only-agent",
            LiveEventKind::WaitStarted {
                agent: None,
                kind: WaitKind::Input,
            },
        )
        .with_context(LiveContext {
            tmux: Some(LiveTmux {
                socket: Some("/tmp/tmux-1000/default".into()),
                session: Some("scratch".into()),
                pane: Some("%4".into()),
            }),
            ..LiveContext::default()
        }),
    ] {
        store
            .apply_event(&event)
            .expect("the store takes the event");
    }
    store
}

fn overlay_over(store: &LiveStore) -> LiveOverlay {
    LiveOverlay::over(
        store.path(),
        Box::new(SystemFiles),
        Box::new(ProcProbe::system()),
    )
}

#[test]
fn the_overlay_puts_one_live_value_on_every_listing_of_an_entity() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let overlay = || overlay_over(&store);

    // The values the rows must carry: the store bound to this snapshot.
    let mut binder = overlay_over(&store);
    binder.rebind(&snapshot, Instant::now());
    let mut bound = snapshot.clone();
    bound.node_live = binder.table(&snapshot).clone();
    assert_eq!(bound.node_live.len(), 4, "every entry binds");
    assert_one_value_per_entity(&bound, overlay);
}

#[test]
fn live_state_on_the_rows_changes_no_frame() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let bare = live_world();
    let tabled = with_table();
    for view in ALL_VIEWS {
        for (width, height) in [(220, 40), (150, 34), (80, 24)] {
            let draw = |snapshot: &GraphSnapshot, overlay: LiveOverlay| {
                let mut app = listing_with(view, snapshot, overlay);
                let buffer = buffer_of(&mut app, width, height);
                tuikit::testkit::ansi::to_ansi(&buffer, buffer.area)
            };
            let without = draw(&bare, LiveOverlay::off());
            assert_eq!(
                draw(&tabled, LiveOverlay::off()),
                without,
                "{view:?} at {width}x{height}: the snapshot's live table shows"
            );
            assert_eq!(
                draw(&bare, overlay_over(&store)),
                without,
                "{view:?} at {width}x{height}: the overlay's live table shows"
            );
        }
    }
    // The rows did carry it.
    let app = listing_with(View::Sessions, &bare, overlay_over(&store));
    assert!(
        carried(&app, &bare)
            .into_iter()
            .any(|(_, live)| has_state(live))
    );
}
