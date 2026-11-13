//! Live state rides on the rows the same way in every view: a session or
//! a mux carries one live value wherever it is listed, whether the rows
//! read the snapshot's own table or the overlay the live TUI binds from
//! the store, and it draws as one mark (or, in Pinned, one word) for that
//! entity everywhere. The mark, the waiting count of a group header
//! above it, and a session's subagent chip (`subagents`) are the only
//! cells live state changes.

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
pub(super) const HOSTED: &str = "e44d01cb5a";
pub(super) const BESIDE: &str = "f00dfeed01";
/// The world's loose session, which the Search view finds.
pub(super) const SOLO: &str = "solo000001";

pub(super) fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

pub(super) fn mux(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

/// The shared world with a second agent in `conspectus-main` and no live
/// state.
pub(super) fn live_world() -> GraphSnapshot {
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
pub(super) fn running(state: LiveState, detail: Option<LiveDetail>, subagents: u16) -> LiveBinding {
    let mut observation = LiveObservation::new(state, LiveSource::Hook, NOW * 1000)
        .with_since_ms(NOW * 1000 - 90_000)
        .with_subagents(subagents);
    if let Some(detail) = detail {
        observation = observation.with_detail(detail);
    }
    LiveBinding::new(observation, Liveness::Alive)
}

/// [`live_world`] with live tables: each session in a state of its own,
/// and two agents no session matched in two panes of `scratch`.
pub(super) fn with_table() -> GraphSnapshot {
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
    ]);
    snapshot.pane_live = BTreeMap::from([
        (
            crate::model::LivePane::new(mux("scratch"), Some("%4".into())),
            running(LiveState::Waiting, Some(LiveDetail::Input), 0),
        ),
        (
            crate::model::LivePane::new(mux("scratch"), Some("%5".into())),
            running(LiveState::Starting, None, 0),
        ),
    ]);
    snapshot
}

/// `view` listing every row it can (History with pinned records, the
/// Search view with its search), its rows reading live state through
/// `overlay`.
pub(super) fn listing_with(view: View, snapshot: &GraphSnapshot, overlay: LiveOverlay) -> App {
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
        NodeId::MuxSession(id) => MixedLive::Mux(snapshot.live_counts_of_mux(id, now_ms).into()),
        _ => snapshot
            .node_live
            .get(node)
            .map_or(MixedLive::None, |binding| {
                MixedLive::Session(LiveCell::of(binding, now_ms))
            }),
    };
    // Whether a finished turn was seen is the TUI's judgment, not the
    // model's: every listing must carry the same one, and the rest must
    // be the model's.
    let model_part = |live: MixedLive| match live {
        MixedLive::Session(cell) => MixedLive::Session(LiveCell {
            unseen: false,
            ..cell
        }),
        MixedLive::Mux(mux) => MixedLive::Mux(mux.counts.into()),
        MixedLive::None => MixedLive::None,
    };
    for (node, values) in &seen {
        for (view, live) in values {
            assert_eq!(
                model_part(*live),
                expected(node),
                "{view:?} lists {node} with another live value; every listing: {values:?}"
            );
            assert_eq!(
                *live, values[0].1,
                "{view:?} judges {node}'s turn unlike {:?}",
                values[0].0
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

/// An agent's context in pane `pane` of `scratch`.
fn in_scratch(pane: &str) -> LiveContext {
    LiveContext {
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: Some("scratch".into()),
            pane: Some(pane.into()),
        }),
        ..LiveContext::default()
    }
}

/// The store [`with_table`]'s tables describe, written the way hooks
/// write them, in `dir`.
pub(super) fn written_store(dir: &std::path::Path) -> LiveStore {
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
        .with_context(in_scratch("%4")),
        event("other-pane-only-agent", LiveEventKind::TurnStarted).with_context(in_scratch("%5")),
    ] {
        store
            .apply_event(&event)
            .expect("the store takes the event");
    }
    store
}

pub(super) fn overlay_over(store: &LiveStore) -> LiveOverlay {
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
    let tables = binder.table(&snapshot);
    bound.node_live = tables.nodes.clone();
    bound.pane_live = tables.panes.clone();
    assert_eq!(
        (bound.node_live.len(), bound.pane_live.len()),
        (3, 2),
        "every entry binds"
    );
    assert_one_value_per_entity(&bound, overlay);
}

/// What a drawn row says about its entity's live state: the mark after
/// the age on a row of a view of one kind, or the state word on a Pinned
/// or History row.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shown {
    Mark(String),
    Word(String),
}

/// The left pane's lines of `app`, one per showing row: the Search view's
/// header line is not a row.
pub(super) fn row_lines(app: &mut App, width: u16) -> Vec<String> {
    let buffer = buffer_of(app, width, 80);
    let mut lines = super::columns::left_pane(&buffer);
    if app.active_view() == View::Search {
        lines.remove(0);
    }
    lines
}

/// Every session and mux row `app` draws, with the entity it stands for,
/// what it shows of the entity's live state, and its line.
fn shown(app: &mut App, snapshot: &GraphSnapshot) -> Vec<(NodeId, Shown, String)> {
    let lines = row_lines(app, 220);
    let rows = app.visible_rows();
    assert!(rows.len() <= lines.len(), "the pane shows every row");
    let mark = |line: &str| {
        let tokens = tuikit::testkit::buffer::tokens(line);
        let age = tokens
            .iter()
            .position(|token| is_age(&token.text))
            .unwrap_or_else(|| panic!("no age: {line}"));
        Shown::Mark(tokens[age + 1].text.clone())
    };
    rows.iter()
        .zip(lines)
        .filter_map(|(row, line)| match &row.kind {
            RowKind::AgentSession(session) => {
                Some((session.primary_node.clone(), mark(&line), line))
            }
            RowKind::MuxSession(mux) => Some((mux.primary_node.clone(), mark(&line), line)),
            RowKind::Mixed(mixed) if mixed.state.is_some() => {
                Some((mixed.primary_node.clone(), mark(&line), line))
            }
            RowKind::Mixed(mixed) => {
                let entity = entity_of(snapshot, &mixed.primary_node).filter(|node| {
                    matches!(node, NodeId::AgentSession(_) | NodeId::MuxSession(_))
                })?;
                let (_, rest) = line.split_once(&mixed.label)?;
                let word = rest.trim_start().split("  ").next().unwrap_or_default();
                Some((entity, Shown::Word(word.to_string()), line))
            }
            _ => None,
        })
        .collect()
}

/// What `snapshot`'s live tables say a row standing for `node` shows, read
/// through the model alone: a session's mark from its effective state, a
/// mux's from its counts by the default precedence, and on a Pinned row
/// a running session's live word in place of `status`.
fn expected(snapshot: &GraphSnapshot, node: &NodeId, mixed: Option<&str>) -> Shown {
    let now_ms = crate::tui::rows::tree_current_unix_ms();
    let state = |node: &NodeId| snapshot.live_state_of(node, now_ms);
    let session_mark = |node: &NodeId| match state(node).map(|live| live.state) {
        None | Some(LiveState::Exited) => "·",
        Some(LiveState::Waiting) => "!",
        Some(LiveState::Working | LiveState::Compacting | LiveState::Starting) => "»",
        Some(LiveState::Ready) => "›",
        Some(LiveState::Error) => "×",
        Some(LiveState::Unknown) => "?",
    };
    match (node, mixed) {
        (NodeId::MuxSession(id), None) => {
            let counts = snapshot.live_counts_of_mux(id, now_ms);
            let mark = match counts.dominant() {
                None | Some(LiveState::Exited) => "·",
                Some(LiveState::Waiting) => "!",
                Some(LiveState::Error) => "×",
                Some(LiveState::Ready) => "›",
                Some(LiveState::Working | LiveState::Compacting | LiveState::Starting) => "»",
                Some(LiveState::Unknown) => "?",
            };
            Shown::Mark(mark.to_string())
        }
        (_, None) => Shown::Mark(session_mark(node).to_string()),
        (NodeId::AgentSession(_), Some(status)) if ["in mux", "resumable"].contains(&status) => {
            let word = match state(node).map(|live| live.state) {
                Some(LiveState::Waiting) => "waiting",
                Some(LiveState::Working | LiveState::Compacting | LiveState::Starting) => "working",
                Some(LiveState::Ready) => "ready",
                Some(LiveState::Error) => "error",
                _ => status,
            };
            Shown::Word(word.to_string())
        }
        (_, Some(status)) => Shown::Word(status.to_string()),
    }
}

/// Every view draws each session and mux of `snapshot` with the mark (or
/// word) its live state gives it, its rows reading live state through
/// `overlay`; returns what was drawn for each entity.
fn assert_one_mark_per_entity(
    snapshot: &GraphSnapshot,
    overlay: impl Fn() -> LiveOverlay,
) -> BTreeMap<NodeId, Vec<(View, Shown)>> {
    let mut seen: BTreeMap<NodeId, Vec<(View, Shown)>> = BTreeMap::new();
    for view in ALL_VIEWS {
        let mut app = listing_with(view, snapshot, overlay());
        let statuses: BTreeMap<String, String> = app
            .tree()
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::Mixed(mixed) if mixed.state.is_none() => {
                    Some((mixed.label.clone(), mixed.status.clone()))
                }
                _ => None,
            })
            .collect();
        for (node, drawn, line) in shown(&mut app, snapshot) {
            let status = match &drawn {
                Shown::Mark(_) => None,
                Shown::Word(_) => statuses
                    .iter()
                    .find(|(label, _)| line.contains(label.as_str()))
                    .map(|(_, status)| status.as_str()),
            };
            assert_eq!(
                drawn,
                expected(snapshot, &node, status),
                "{view:?} draws {node} unlike its live state:\n{line}"
            );
            seen.entry(node).or_default().push((view, drawn));
        }
    }
    seen
}

#[test]
fn one_entity_shows_one_mark_in_every_view() {
    let snapshot = with_table();
    let seen = assert_one_mark_per_entity(&snapshot, LiveOverlay::off);
    // The check means something only if live entities are drawn in
    // several views, with marks of several shapes.
    let marks = |node: &NodeId| -> Vec<Shown> {
        seen.get(node)
            .unwrap_or_else(|| panic!("no view draws {node}"))
            .iter()
            .map(|(_, shown)| shown.clone())
            .collect()
    };
    let hosted = marks(&session(HOSTED));
    assert!(hosted.contains(&Shown::Mark("!".into())), "{hosted:?}");
    assert!(
        hosted.contains(&Shown::Word("waiting".into())),
        "{hosted:?}"
    );
    let solo = marks(&session(SOLO));
    assert!(solo.contains(&Shown::Mark("›".into())), "{solo:?}");
    assert!(solo.contains(&Shown::Word("ready".into())), "{solo:?}");
    let beside = marks(&session(BESIDE));
    assert!(beside.contains(&Shown::Mark("×".into())), "{beside:?}");
    // The mux hosts a waiting agent and a failed one: waiting first.
    let main = marks(&NodeId::MuxSession(mux("conspectus-main")));
    assert!(main.contains(&Shown::Mark("!".into())), "{main:?}");
    assert!(
        seen.values()
            .flatten()
            .any(|(_, shown)| *shown == Shown::Mark("·".into())),
        "a row with no live state"
    );
}

#[test]
fn the_overlay_draws_one_mark_on_every_listing_of_an_entity() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let mut binder = overlay_over(&store);
    binder.rebind(&snapshot, Instant::now());
    let mut bound = snapshot.clone();
    let tables = binder.table(&snapshot);
    bound.node_live = tables.nodes.clone();
    bound.pane_live = tables.panes.clone();
    let seen = assert_one_mark_per_entity(&bound, || overlay_over(&store));
    assert!(
        seen.values()
            .flatten()
            .any(|(_, shown)| *shown == Shown::Mark("!".into())),
        "the store's states are drawn"
    );
}

/// [`live_world`] with the marks [`with_table`] leaves out: a working
/// agent gone quiet, one whose evidence is too old to trust, and one that
/// exited.
fn with_quiet_and_stale() -> GraphSnapshot {
    let mut snapshot = live_world();
    let quiet = LiveObservation::new(
        LiveState::Working,
        LiveSource::Hook,
        NOW * 1000 - 11 * 60_000,
    )
    .with_detail(LiveDetail::Tool);
    let stale = LiveObservation::new(
        LiveState::Waiting,
        LiveSource::Hook,
        NOW * 1000 - 20 * 60_000,
    );
    let exited = LiveObservation::new(LiveState::Exited, LiveSource::Hook, NOW * 1000)
        .with_detail(LiveDetail::Ended);
    snapshot.node_live = BTreeMap::from([
        (session(HOSTED), LiveBinding::new(quiet, Liveness::Alive)),
        (session(BESIDE), LiveBinding::new(stale, Liveness::Unknown)),
        (session(SOLO), LiveBinding::new(exited, Liveness::Unknown)),
    ]);
    snapshot
}

#[test]
fn quiet_stale_and_exited_agents_show_their_marks_and_keep_their_words() {
    let snapshot = with_quiet_and_stale();
    let seen = assert_one_mark_per_entity(&snapshot, LiveOverlay::off);
    let shown = |key: &str| -> Vec<Shown> {
        seen[&session(key)]
            .iter()
            .map(|(_, shown)| shown.clone())
            .collect()
    };
    assert!(shown(HOSTED).contains(&Shown::Mark("»".into())));
    assert!(shown(HOSTED).contains(&Shown::Word("working".into())));
    assert!(shown(BESIDE).contains(&Shown::Mark("?".into())));
    // Stale and exited say nothing live, so Pinned keeps the link word.
    assert!(shown(SOLO).contains(&Shown::Mark("·".into())));
    assert!(
        shown(SOLO)
            .iter()
            .all(|shown| !matches!(shown, Shown::Word(word) if word == "ready")),
        "{:?}",
        shown(SOLO)
    );

    // Quiet dims the same shape, and the same word in Pinned.
    let theme = crate::tui::Theme::default();
    for (view, needle) in [(View::Sessions, "»"), (View::Pinned, "working")] {
        let mut app = listing_with(view, &snapshot, LiveOverlay::off());
        let buffer = buffer_of(&mut app, 220, 40);
        let lines = tuikit::testkit::buffer::rows(&buffer);
        let y = lines
            .iter()
            .position(|line| line.contains("e44d01cb") && line.contains(needle))
            .unwrap_or_else(|| panic!("{view:?}: the hosted session's row"));
        let x = lines[y]
            .split(needle)
            .next()
            .map(|before| before.chars().count())
            .expect("the mark or word");
        let cell = &buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())];
        assert_eq!(cell.fg, theme.info, "{view:?}");
        assert!(
            cell.modifier.contains(theme.muted),
            "{view:?}: quiet is dimmed"
        );
    }
}

/// The words of a group header's line less its waiting count (`!` and the
/// number after it), and that count, if it has one.
pub(super) fn without_waiting_count(line: &str) -> (Vec<String>, Option<String>) {
    let words: Vec<String> = tuikit::testkit::buffer::tokens(line)
        .into_iter()
        .map(|token| token.text)
        .collect();
    match words
        .iter()
        .position(|word| word == crate::tui::icons::live::WAITING)
    {
        Some(at) if at + 1 < words.len() => {
            let count = words[at + 1].clone();
            let mut rest = words;
            rest.drain(at..=at + 1);
            (rest, Some(count))
        }
        _ => (words, None),
    }
}

/// `snapshot` with every agent's subagent count cleared, so live state
/// draws its marks and counts but no subagent chip.
pub(super) fn without_subagents(mut snapshot: GraphSnapshot) -> GraphSnapshot {
    for binding in snapshot
        .node_live
        .values_mut()
        .chain(snapshot.pane_live.values_mut())
    {
        binding.observation.subagents = 0;
    }
    snapshot
}

#[test]
fn live_state_changes_the_live_cells_and_the_waiting_counts_and_nothing_else() {
    // The subagent chip is live state's one other change; its own suite
    // checks it is all subagents change.
    let bare = live_world();
    let tabled = without_subagents(with_table());
    let live_marks = crate::tui::icons::live::ALL;
    for view in [View::Sessions, View::Mux, View::Prs, View::Search] {
        for width in [220, 110, 80, 60] {
            let lines = |snapshot: &GraphSnapshot| {
                let mut app = listing_with(view, snapshot, LiveOverlay::off());
                let headers: Vec<bool> = app
                    .visible_rows()
                    .iter()
                    .map(|row| matches!(row.kind, RowKind::Group(_)))
                    .collect();
                let mut lines = row_lines(&mut app, width);
                lines.truncate(headers.len());
                (lines, headers)
            };
            let ((without, headers), (with, _)) = (lines(&bare), lines(&tabled));
            assert_eq!(without.len(), with.len(), "{view:?} at {width}");
            let mut changed = 0;
            for ((before, after), header) in without.iter().zip(&with).zip(headers) {
                // A header gains the count of the agents beneath it that
                // wait, and its warning moves past that column; its other
                // words stay.
                if header {
                    let (words, count) = without_waiting_count(after);
                    assert_eq!(
                        without_waiting_count(before),
                        (words, None),
                        "{view:?} at {width}: a header changed beyond its waiting count:\n{before}\n{after}"
                    );
                    if count.is_some() {
                        changed += 1;
                    }
                    continue;
                }
                let diffs: Vec<(char, char)> = before
                    .chars()
                    .zip(after.chars())
                    .filter(|(a, b)| a != b)
                    .collect();
                assert!(
                    diffs.len() <= 1,
                    "{view:?} at {width}: more than the live cell changed:\n{before}\n{after}"
                );
                if let Some((was, is)) = diffs.first() {
                    assert!(
                        was.to_string() == crate::tui::icons::live::NONE
                            && live_marks.contains(&is.to_string().as_str()),
                        "{view:?} at {width}: the change is not a live mark:\n{before}\n{after}"
                    );
                    changed += 1;
                }
            }
            if view != View::Prs {
                assert!(changed > 0, "{view:?} at {width} drew no live state");
            }
        }
    }
}

#[test]
fn the_live_mark_reads_on_the_cursor_row_focused_and_not() {
    let snapshot = with_table();
    let theme = crate::tui::Theme::default();
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    select(&mut app, HOSTED);
    for focused in [true, false] {
        if !focused {
            app.update(Msg::CycleFocus);
        }
        let buffer = buffer_of(&mut app, 220, 40);
        let lines = tuikit::testkit::buffer::rows(&buffer);
        let y = lines
            .iter()
            .position(|line| {
                line.contains("e44d01cb")
                    && line
                        .trim_start_matches('│')
                        .starts_with(tuikit::glyphs::marks::POINTER)
            })
            .expect("the cursor row");
        let x = lines[y]
            .chars()
            .position(|c| c == '!')
            .unwrap_or_else(|| panic!("no mark on the cursor row:\n{}", lines.join("\n")));
        let cell = &buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())];
        assert_eq!(cell.symbol(), "!", "the shape is drawn");
        assert_eq!(cell.fg, theme.warning, "the mark keeps its color");
        let highlight = if focused {
            theme.selection_active
        } else {
            theme.selection_inactive
        };
        assert!(
            cell.modifier.contains(highlight),
            "focused {focused}: the mark takes the row's highlight: {:?}",
            cell.modifier
        );
    }
}

#[test]
fn a_store_change_redraws_the_mark_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let mut app = listing_with(View::Sessions, &snapshot, overlay_over(&store));
    select(&mut app, HOSTED);
    let selected = app.selection().cloned();
    let mark_of_hosted = |app: &mut App| {
        let lines = row_lines(app, 220);
        let line = lines
            .iter()
            .find(|line| line.contains("e44d01cb"))
            .expect("the hosted row")
            .clone();
        let tokens = tuikit::testkit::buffer::tokens(&line);
        let age = tokens.iter().position(|t| is_age(&t.text)).expect("an age");
        tokens[age + 1].text.clone()
    };
    assert_eq!(mark_of_hosted(&mut app), "!");
    let ended = LiveEvent::new(
        "claude-code",
        HOSTED,
        now_ms(),
        LiveEventKind::TurnEnded { interrupted: false },
    );
    // The permission was answered and the turn ended.
    store
        .apply_event(&LiveEvent::new(
            "claude-code",
            HOSTED,
            now_ms(),
            LiveEventKind::WaitEnded { agent: None },
        ))
        .expect("the store takes the event");
    store
        .apply_event(&ended)
        .expect("the store takes the event");
    app.poll_live(
        Instant::now() + std::time::Duration::from_secs(5),
        crate::tui::rows::tree_current_unix_ms(),
    );
    assert_eq!(mark_of_hosted(&mut app), "›", "the row reads ready");
    assert_eq!(app.selection().cloned(), selected, "the cursor stays");
}
