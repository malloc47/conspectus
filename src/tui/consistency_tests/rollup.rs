//! Live state rolls up the same way wherever it is counted: a group
//! header counts the agents beneath it that wait on the operator, each
//! once, and agrees with the marks of the rows it counts; a mux's mark is
//! its agents' state by the operator's order; and the counts follow the
//! rows when the live state changes, in place.

use std::collections::BTreeMap;
use std::time::Instant;

use super::live::{
    BESIDE, HOSTED, listing_with, live_world, overlay_over, row_lines, running, session,
    with_table, without_waiting_count, written_store,
};
use super::*;
use crate::live_store::{LiveEvent, LiveEventKind, now_ms};
use crate::model::live::LiveState;
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::{MixedLive, RowKind, ViewLabel};
use crate::tui::{Grouping, MuxGrouping, SearchGrouping};

/// `view` over `snapshot` grouped so it has headers, with every row
/// unfolded.
fn grouped(view: View, snapshot: &GraphSnapshot, overlay: LiveOverlay) -> App {
    let mut app = listing_with(view, snapshot, overlay);
    match view {
        View::Mux => {
            app.update(Msg::SetGrouping(Grouping::Mux(MuxGrouping::Repo)));
        }
        View::Search => {
            app.update(Msg::SetGrouping(Grouping::Search(SearchGrouping::Project)));
        }
        _ => {}
    }
    unfold_all(&mut app);
    app
}

fn unfold_all(app: &mut App) {
    let foldable: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| row.expandable)
        .map(|row| row.id.clone())
        .collect();
    for id in foldable {
        app.set_selection(id);
        app.update(Msg::ExpandRow);
    }
}

/// The waiting count each showing header draws (`None`: it draws none),
/// with the header's row index in the tree.
fn drawn_counts(app: &mut App) -> Vec<(usize, Option<usize>)> {
    let lines = row_lines(app, 220);
    let shown: Vec<RowId> = app
        .visible_rows()
        .iter()
        .map(|row| row.id.clone())
        .collect();
    shown
        .iter()
        .zip(&lines)
        .filter_map(|(id, line)| {
            let index = app.tree().rows.iter().position(|row| &row.id == id)?;
            matches!(app.tree().rows[index].kind, RowKind::Group(_)).then(|| {
                let (_, count) = without_waiting_count(line);
                (
                    index,
                    count.map(|count| count.parse().expect("a count is a number")),
                )
            })
        })
        .collect()
}

/// What the model alone says the header at `index` of `app`'s tree
/// counts: the distinct sessions beneath it whose state is waiting, or
/// in the Mux view the waiting agents of the distinct muxes beneath it.
fn expected_count(app: &App, snapshot: &GraphSnapshot, index: usize) -> usize {
    let now = crate::tui::rows::tree_current_unix_ms();
    let rows = &app.tree().rows;
    let depth = rows[index].depth;
    let beneath = rows[index + 1..].iter().take_while(|row| row.depth > depth);
    let mut counted = std::collections::BTreeSet::new();
    let mut total = 0;
    for row in beneath {
        match (&row.kind, app.tree().view) {
            (RowKind::MuxSession(row), ViewLabel::Mux) => {
                if counted.insert(row.primary_node.clone()) {
                    total += snapshot.live_counts_of_mux(&row.mux, now).waiting as usize;
                }
            }
            (RowKind::Mixed(row), ViewLabel::Mux) => {
                if let (NodeId::MuxSession(id), true) =
                    (&row.primary_node, counted.insert(row.primary_node.clone()))
                {
                    total += snapshot.live_counts_of_mux(id, now).waiting as usize;
                }
            }
            (RowKind::AgentSession(row), view) if view != ViewLabel::Mux => {
                let waiting = snapshot
                    .live_state_of(&row.primary_node, now)
                    .is_some_and(|live| live.state == LiveState::Waiting);
                if waiting && counted.insert(row.primary_node.clone()) {
                    total += 1;
                }
            }
            _ => {}
        }
    }
    total
}

/// Every header of every view draws the count the model gives it, and
/// none when it is zero; returns how many headers drew one.
fn assert_counts_follow_the_model(snapshot: &GraphSnapshot) -> BTreeMap<View, usize> {
    let mut drawn = BTreeMap::new();
    for view in ALL_VIEWS {
        let mut app = grouped(view, snapshot, LiveOverlay::off());
        for (index, count) in drawn_counts(&mut app) {
            let expected = expected_count(&app, snapshot, index);
            assert_eq!(
                count.unwrap_or(0),
                expected,
                "{view:?}: the header {:?} counts otherwise",
                app.tree().rows[index].id
            );
            if count.is_some() {
                *drawn.entry(view).or_default() += 1;
            }
        }
    }
    drawn
}

#[test]
fn every_header_counts_the_agents_beneath_it_that_wait_in_every_view() {
    // The table, and the table with the session the Search view finds
    // waiting too, so a header of every view that has headers counts.
    let mut searched_waits = with_table();
    searched_waits.node_live.insert(
        session(super::live::SOLO),
        running(LiveState::Waiting, None, 0),
    );
    let mut drawn = assert_counts_follow_the_model(&with_table());
    for (view, headers) in assert_counts_follow_the_model(&searched_waits) {
        *drawn.entry(view).or_default() += headers;
    }
    for view in [View::Sessions, View::Mux, View::Search] {
        assert!(
            drawn.get(&view).is_some_and(|n| *n > 0),
            "{view:?}: {drawn:?}"
        );
    }
    assert!(
        !drawn.contains_key(&View::Pinned) && !drawn.contains_key(&View::History),
        "Pinned and History buckets carry no counts: {drawn:?}"
    );
}

#[test]
fn a_header_count_is_the_number_of_waiting_marks_beneath_it() {
    let snapshot = with_table();
    for view in [View::Sessions, View::Prs, View::Search] {
        let mut app = grouped(view, &snapshot, LiveOverlay::off());
        let lines = row_lines(&mut app, 220);
        let rows: Vec<(u8, bool, String)> = app
            .visible_rows()
            .iter()
            .map(|row| {
                let key = match &row.kind {
                    RowKind::AgentSession(session) => session.session.session_key.clone(),
                    _ => String::new(),
                };
                (row.depth, matches!(row.kind, RowKind::Group(_)), key)
            })
            .collect();
        for (at, (depth, header, _)) in rows.iter().enumerate() {
            if !header {
                continue;
            }
            let (_, count) = without_waiting_count(&lines[at]);
            let waiting: std::collections::BTreeSet<&str> = rows[at + 1..]
                .iter()
                .zip(&lines[at + 1..])
                .take_while(|((beneath, _, _), _)| beneath > depth)
                .filter(|((_, header, key), line)| {
                    !header
                        && !key.is_empty()
                        && tuikit::testkit::buffer::tokens(line)
                            .iter()
                            .any(|token| token.text == crate::tui::icons::live::WAITING)
                })
                .map(|((_, _, key), _)| key.as_str())
                .collect();
            assert_eq!(
                count.map_or(0, |count| count.parse::<usize>().unwrap()),
                waiting.len(),
                "{view:?}: {}",
                lines[at]
            );
        }
    }
}

#[test]
fn a_mux_view_header_counts_each_agent_once_through_its_mux() {
    // `conspectus-main` hosts a waiting agent, listed beneath it, and one
    // in error; `scratch` runs a waiting agent and a starting one in panes
    // no session matches. The header counts the two waiting agents once
    // each, the listed one through its mux, and keeps its session count.
    let snapshot = with_table();
    let mut app = grouped(View::Mux, &snapshot, LiveOverlay::off());
    let lines = row_lines(&mut app, 220);
    let header = lines
        .iter()
        .find(|line| line.contains("/home/op/src/conspectus"))
        .expect("the repo header");
    assert!(header.contains("(2)  ! 2"), "{header}");
}

/// The Mux view's line for `name`, and the lines of the agents listed
/// beneath it.
fn mux_and_agents(app: &mut App, name: &str) -> (String, Vec<String>) {
    let lines = row_lines(app, 220);
    let rows: Vec<(u8, bool)> = app
        .visible_rows()
        .iter()
        .map(|row| (row.depth, matches!(row.kind, RowKind::AgentSession(_))))
        .collect();
    let at = lines
        .iter()
        .position(|line| line.contains(name) && !line.contains('●'))
        .unwrap_or_else(|| panic!("no mux row for {name}:\n{}", lines.join("\n")));
    let agents = rows[at + 1..]
        .iter()
        .zip(&lines[at + 1..])
        .take_while(|((depth, agent), _)| *agent && *depth > rows[at].0)
        .map(|(_, line)| line.clone())
        .collect();
    (lines[at].clone(), agents)
}

/// The live mark a drawn row shows: the word after its age.
fn mark(line: &str) -> String {
    let tokens = tuikit::testkit::buffer::tokens(line);
    let age = tokens
        .iter()
        .position(|token| is_age(&token.text))
        .unwrap_or_else(|| panic!("no age: {line}"));
    tokens[age + 1].text.clone()
}

#[test]
fn a_mux_with_one_two_or_mixed_agents_draws_their_rollup() {
    use LiveState::{Error, Ready, Waiting, Working};
    let live = |state| running(state, None, 0);
    // One agent: the mux shows its mark and lists no row for it.
    let mut one = world();
    one.node_live = BTreeMap::from([(session(HOSTED), live(Ready))]);
    let mut app = listing_with(View::Mux, &one, LiveOverlay::off());
    let (row, agents) = mux_and_agents(&mut app, "conspectus-main");
    assert_eq!(mark(&row), "›", "{row}");
    assert!(agents.is_empty(), "{agents:?}");

    // Two agents, then mixed states: the mux's mark by the operator's
    // order, each agent beneath it with its own.
    for (hosted, beside, rolled) in [
        (Working, Working, "»"),
        (Working, Waiting, "!"),
        (Ready, Error, "×"),
        (Working, Ready, "›"),
    ] {
        let mut two = live_world();
        two.node_live = BTreeMap::from([
            (session(HOSTED), live(hosted)),
            (session(BESIDE), live(beside)),
        ]);
        let mut app = listing_with(View::Mux, &two, LiveOverlay::off());
        unfold_all(&mut app);
        let (row, agents) = mux_and_agents(&mut app, "conspectus-main");
        assert_eq!(mark(&row), rolled, "{hosted:?} and {beside:?}: {row}");
        let mut marks: Vec<String> = agents.iter().map(|line| mark(line)).collect();
        marks.sort();
        let mut own = vec![
            crate::tui::rows::LiveMark::of_state(hosted)
                .glyph()
                .to_string(),
            crate::tui::rows::LiveMark::of_state(beside)
                .glyph()
                .to_string(),
        ];
        own.sort();
        assert_eq!(marks, own, "the agents keep their own marks: {agents:?}");
    }
}

#[test]
fn the_waiting_counts_follow_a_store_change_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let mut app = grouped(View::Sessions, &snapshot, overlay_over(&store));
    app.set_selection(row_id_for(&app, HOSTED).expect("the hosted row"));
    let selected = app.selection().cloned();
    let header = |app: &mut App| -> Option<usize> {
        drawn_counts(app).into_iter().find_map(|(_, count)| count)
    };
    assert_eq!(header(&mut app), Some(1), "the hosted agent waits");
    for kind in [
        LiveEventKind::WaitEnded { agent: None },
        LiveEventKind::TurnEnded { interrupted: false },
    ] {
        store
            .apply_event(&LiveEvent::new("claude-code", HOSTED, now_ms(), kind))
            .expect("the store takes the event");
    }
    let rebuilds = app.tree_rebuilds;
    app.poll_live(
        Instant::now() + std::time::Duration::from_secs(5),
        crate::tui::rows::tree_current_unix_ms(),
    );
    assert_eq!(header(&mut app), None, "nothing waits any more");
    assert_eq!(app.tree_rebuilds, rebuilds, "the rows refilled in place");
    assert_eq!(app.selection().cloned(), selected, "the cursor stays");
}

#[test]
fn the_waiting_count_reads_in_the_waiting_mark_style() {
    let snapshot = with_table();
    let theme = crate::tui::Theme::default();
    let mut app = grouped(View::Sessions, &snapshot, LiveOverlay::off());
    let buffer = buffer_of(&mut app, 220, 40);
    let lines = tuikit::testkit::buffer::rows(&buffer);
    let y = lines
        .iter()
        .position(|line| line.contains("/home/op/src/conspectus") && line.contains("! 1"))
        .expect("the repo header with its count");
    let x = lines[y]
        .find("! 1")
        .map(|at| lines[y][..at].chars().count())
        .unwrap();
    for offset in [0, 2] {
        let cell = &buffer[(
            u16::try_from(x + offset).unwrap(),
            u16::try_from(y).unwrap(),
        )];
        assert_eq!(cell.fg, theme.warning, "the waiting mark's color");
    }
    // The mixed rows of Pinned keep the counts out of their buckets even
    // when what they stand for waits.
    let mut pinned = grouped(View::Pinned, &snapshot, LiveOverlay::off());
    assert!(
        pinned.tree().rows.iter().any(|row| matches!(
            &row.kind,
            RowKind::Mixed(mixed) if matches!(mixed.live, MixedLive::Session(_))
        )),
        "Pinned lists a session with live state"
    );
    assert!(
        drawn_counts(&mut pinned)
            .iter()
            .all(|(_, count)| count.is_none())
    );
}
