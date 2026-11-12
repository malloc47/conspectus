use super::*;
use crate::model::live::{LiveSource, Liveness};
use crate::model::{AgentSessionId, PinId};
use crate::tui::rows::{RowId, TreeInputs, build_tree_for_view, pinned::fixture_world};
use crate::tui::{Grouping, MuxRecency, Sort, View};

const NOW_MS: i64 = 1_700_000_000_000;
const MINUTE_MS: i64 = 60_000;

/// The fixture's session, hosted by its mux `conspectus-main`.
fn hosted() -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "e44d01cb5a"))
}

fn main_mux() -> MuxSessionId {
    MuxSessionId::new("tmux:conspectus-main")
}

/// `state` observed `minutes_ago`, its process found running.
fn running(state: LiveState, minutes_ago: i64) -> LiveBinding {
    LiveBinding::new(
        LiveObservation::new(state, LiveSource::Hook, NOW_MS - minutes_ago * MINUTE_MS)
            .with_pid(42, None),
        Liveness::Alive,
    )
}

/// The pinned fixture with the hosted session waiting on a question for
/// two subagents, and an agent that no session matches in the mux.
fn world() -> GraphSnapshot {
    let mut snapshot = fixture_world();
    let mut waiting = running(LiveState::Waiting, 3);
    waiting.observation = waiting
        .observation
        .with_detail(LiveDetail::Question)
        .with_since_ms(NOW_MS - 5 * MINUTE_MS)
        .with_subagents(2);
    snapshot.node_live.insert(hosted(), waiting);
    snapshot
        .node_live
        .insert(NodeId::MuxSession(main_mux()), running(LiveState::Ready, 1));
    snapshot
}

fn built(snapshot: &GraphSnapshot, view: View, now_ms: i64) -> RowTree {
    let mut tree = build_tree_for_view(TreeInputs {
        snapshot,
        live: snapshot.live_tables(),
        view,
        grouping: Grouping::default_for(view),
        filter: crate::filter::RowFilter::default(),
        sort: Sort::Hierarchy,
        mux_recency: MuxRecency::default(),
        cwd: None,
    });
    // Read at a fixed time, so the test does not depend on the clock.
    annotate(
        &mut tree,
        snapshot,
        &LiveValues::at(snapshot, snapshot.live_tables(), now_ms),
    );
    tree
}

#[test]
fn a_cell_is_the_effective_state_beside_the_observations_facts() {
    let binding = world().node_live[&hosted()];

    assert_eq!(
        LiveCell::of(&binding, NOW_MS),
        LiveCell {
            state: LiveState::Waiting,
            detail: Some(LiveDetail::Question),
            confidence: LiveConfidence::Fresh,
            since_ms: NOW_MS - 5 * MINUTE_MS,
            subagents: 2,
        }
    );
}

#[test]
fn a_cell_is_read_at_the_time_it_is_filled() {
    let working = running(LiveState::Working, 0);
    assert_eq!(
        LiveCell::of(&working, NOW_MS).confidence,
        LiveConfidence::Fresh
    );
    let later = LiveCell::of(&working, NOW_MS + 11 * MINUTE_MS);
    assert_eq!(
        (later.state, later.confidence),
        (LiveState::Working, LiveConfidence::Quiet),
        "a long turn stays working, flagged quiet"
    );

    let dead = LiveBinding::new(working.observation, Liveness::Dead);
    assert_eq!(
        (
            LiveCell::of(&dead, NOW_MS).state,
            LiveCell::of(&dead, NOW_MS).detail
        ),
        (LiveState::Exited, Some(LiveDetail::Killed))
    );
}

#[test]
fn a_session_row_carries_its_cell_and_a_session_without_evidence_none() {
    let mut snapshot = world();
    let tree = built(&snapshot, View::Sessions, NOW_MS);
    let cell = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.primary_node == hosted() => {
                Some(session.live)
            }
            _ => None,
        })
        .expect("the session has a row");
    assert_eq!(
        cell,
        snapshot
            .node_live
            .get(&hosted())
            .map(|b| LiveCell::of(b, NOW_MS))
    );

    snapshot.node_live.remove(&hosted());
    let tree = built(&snapshot, View::Sessions, NOW_MS);
    assert!(tree.rows.iter().all(|row| match &row.kind {
        RowKind::AgentSession(session) => session.live.is_none(),
        _ => true,
    }));
}

#[test]
fn a_mux_row_counts_the_agents_it_hosts_with_the_snapshots_rule() {
    let snapshot = world();
    let tree = built(&snapshot, View::Mux, NOW_MS);
    let counts = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(mux) if mux.mux == main_mux() => Some(mux.live),
            _ => None,
        })
        .expect("the mux has a row");

    assert_eq!(
        counts,
        LiveCounts {
            waiting: 1,
            ready: 1,
            ..LiveCounts::default()
        },
        "its session and the agent in a pane no session matched"
    );
    assert_eq!(counts, snapshot.live_counts_of_mux(&main_mux(), NOW_MS));
}

#[test]
fn a_pins_row_carries_the_value_of_the_entity_it_points_at() {
    let snapshot = world();
    let tree = built(&snapshot, View::Pinned, NOW_MS);
    let slot = |name: &str| {
        tree.rows
            .iter()
            .find_map(|row| match (&row.id, &row.kind) {
                (RowId::Pin { pin_id }, RowKind::Mixed(mixed)) if pin_id == name => {
                    Some(mixed.live)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no row for the pin {name}"))
    };

    assert_eq!(
        slot("e44d01cb"),
        MixedLive::Session(LiveCell::of(&snapshot.node_live[&hosted()], NOW_MS))
    );
    assert_eq!(
        slot("conspectus-main"),
        MixedLive::Mux(snapshot.live_counts_of_mux(&main_mux(), NOW_MS))
    );
    assert_eq!(slot("ledger-pr"), MixedLive::None, "a PR has no live state");
    assert_eq!(
        slot("nightly"),
        MixedLive::None,
        "a planned mux has no node"
    );
}

#[test]
fn a_mixed_row_for_a_mux_without_evidence_has_empty_counts() {
    let values = LiveValues::at(&world(), GraphSnapshot::empty().live_tables(), NOW_MS);

    assert_eq!(
        values.of_node(&NodeId::MuxSession(main_mux())),
        MixedLive::Mux(LiveCounts::default())
    );
    assert_eq!(values.of_node(&hosted()), MixedLive::None);
    assert_eq!(
        values.of_node(&NodeId::Pin(PinId::new("e44d01cb"))),
        MixedLive::None,
        "a pin itself is no session"
    );
}

#[test]
fn the_rows_read_the_table_they_are_given() {
    let snapshot = world();
    let mut later = GraphSnapshot::empty();
    later
        .node_live
        .insert(hosted(), running(LiveState::Error, 0));
    let mut tree = built(&snapshot, View::Sessions, NOW_MS);

    annotate(
        &mut tree,
        &snapshot,
        &LiveValues::at(&snapshot, later.live_tables(), NOW_MS),
    );

    let states: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some((session.primary_node.clone(), session.live)),
            _ => None,
        })
        .filter_map(|(node, live)| live.map(|cell| (node, cell.state)))
        .collect();
    assert_eq!(states, vec![(hosted(), LiveState::Error)]);
}
