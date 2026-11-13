//! A session whose agent runs subagents shows one `+N` chip after its
//! label in every view that lists it as a session row, and nowhere else:
//! not on a mux row, not in Pinned or History. The chip is all that
//! subagents change, and it follows the store in place.

use std::collections::BTreeMap;
use std::time::Instant;

use super::live::{
    BESIDE, HOSTED, SOLO, listing_with, overlay_over, row_lines, running, session, with_table,
    without_subagents, written_store,
};
use super::*;
use crate::live_store::{LiveEvent, LiveEventKind, now_ms};
use crate::model::live::{LiveDetail, LiveState};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::{LiveCell, RowKind};

/// [`with_table`] with the failed session running four subagents too,
/// and the pinned one an exited session that still records two.
fn with_subagents() -> GraphSnapshot {
    let mut snapshot = with_table();
    snapshot.node_live.insert(
        session(BESIDE),
        running(LiveState::Error, Some(LiveDetail::RateLimited), 4),
    );
    snapshot
        .node_live
        .insert(session(SOLO), running(LiveState::Exited, None, 2));
    snapshot
}

/// The chip text a session in `snapshot` shows, by the model alone.
fn expected_chip(snapshot: &GraphSnapshot, node: &NodeId) -> Option<String> {
    let now = crate::tui::rows::tree_current_unix_ms();
    let binding = snapshot.node_live.get(node)?;
    let running = LiveCell::of(binding, now).running_subagents();
    (running > 0).then(|| {
        if running > crate::tui::widgets::badge::SUBAGENTS_SHOWN {
            "+9+".to_string()
        } else {
            format!("+{running}")
        }
    })
}

/// The `+N` tokens of a drawn line.
fn chips(line: &str) -> Vec<String> {
    tuikit::testkit::buffer::tokens(line)
        .into_iter()
        .map(|token| token.text)
        .filter(|word| {
            word.strip_prefix('+')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect()
}

/// Every view's drawn lines, row by row, with the session each session
/// row stands for.
fn drawn(view: View, snapshot: &GraphSnapshot, width: u16) -> Vec<(Option<NodeId>, String)> {
    let mut app = listing_with(view, snapshot, LiveOverlay::off());
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
    let lines = row_lines(&mut app, width);
    app.visible_rows()
        .iter()
        .zip(lines)
        .map(|(row, line)| match &row.kind {
            RowKind::AgentSession(session) => (Some(session.primary_node.clone()), line),
            _ => (None, line),
        })
        .collect()
}

#[test]
fn one_session_shows_one_subagent_chip_in_every_view() {
    let snapshot = with_subagents();
    let mut seen: BTreeMap<NodeId, Vec<View>> = BTreeMap::new();
    for view in ALL_VIEWS {
        for (node, line) in drawn(view, &snapshot, 220) {
            let Some(node) = node else {
                continue;
            };
            assert_eq!(
                chips(&line),
                expected_chip(&snapshot, &node)
                    .into_iter()
                    .collect::<Vec<_>>(),
                "{view:?} draws {node} with another chip:\n{line}"
            );
            if expected_chip(&snapshot, &node).is_some() {
                seen.entry(node).or_default().push(view);
            }
        }
    }
    // The check means something only if sessions with subagents are drawn
    // in several views.
    for key in [HOSTED, BESIDE] {
        let views = seen
            .get(&session(key))
            .unwrap_or_else(|| panic!("no view draws {key}'s chip"));
        assert!(views.len() >= 2, "{key} is drawn in {views:?} only");
    }
    assert_eq!(
        expected_chip(&snapshot, &session(SOLO)),
        None,
        "an exited session shows none"
    );
}

#[test]
fn subagents_change_only_the_session_rows_by_their_chip() {
    let with = with_subagents();
    let without = without_subagents(with.clone());
    for view in ALL_VIEWS {
        for width in [220, 110, 80, 60] {
            let (after, before) = (drawn(view, &with, width), drawn(view, &without, width));
            assert_eq!(after.len(), before.len(), "{view:?} at {width}");
            for ((node, after), (_, before)) in after.iter().zip(&before) {
                let context = format!("{view:?} at {width}:\n{before}\n{after}");
                let Some(node) = node else {
                    assert_eq!(
                        after, before,
                        "a row that is not a session changed: {context}"
                    );
                    continue;
                };
                match expected_chip(&with, node) {
                    // What precedes the chip is as it was; what follows it
                    // may be cut sooner.
                    Some(chip) if after.contains(&format!("  {chip}")) => {
                        let at = after.find(&format!("  {chip}")).unwrap();
                        assert_eq!(after[..at], before[..at.min(before.len())], "{context}");
                    }
                    // The chip gave way, or there is none: the row is as it
                    // was.
                    _ => assert_eq!(after, before, "{context}"),
                }
            }
        }
    }
}

#[test]
fn a_store_change_refills_the_chip_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = super::live::live_world();
    let mut app = listing_with(View::Sessions, &snapshot, overlay_over(&store));
    let selected = app.selection().cloned();
    let chips_of_solo = |app: &mut App| {
        row_lines(app, 220)
            .iter()
            .find(|line| line.contains("Untitled walk"))
            .map(|line| chips(line))
            .expect("the loose session's row")
    };
    assert!(chips_of_solo(&mut app).is_empty(), "no subagent yet");
    store
        .apply_event(&LiveEvent::new(
            "claude-code",
            SOLO,
            now_ms(),
            LiveEventKind::SubagentStarted {
                id: "agent-1".into(),
            },
        ))
        .expect("the store takes the event");
    let rebuilds = app.tree_rebuilds;
    app.poll_live(
        Instant::now() + std::time::Duration::from_secs(5),
        crate::tui::rows::tree_current_unix_ms(),
    );
    assert_eq!(chips_of_solo(&mut app), vec!["+1".to_string()]);
    assert_eq!(app.tree_rebuilds, rebuilds, "in place");
    assert_eq!(app.selection().cloned(), selected, "the cursor stays");
}
