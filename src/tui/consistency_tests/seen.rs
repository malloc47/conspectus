//! A finished turn the operator has not seen reads as one they have, only
//! heavier: in every view that lists its session or its mux, the mark (in
//! Pinned, the word) keeps its shape and its color and is bold, as are the
//! row's own words (a session's key and label, a mux's name), and nothing
//! else about the row changes. Every listing of one entity agrees,
//! and looking at a session with no mux (resting the cursor on one of its
//! rows) turns them all plain at once.

use std::time::Instant;

use ratatui::style::Modifier;

use super::live::{HOSTED, SOLO, listing_with, mux, running, session, with_table};
use super::*;
use crate::model::live::{LiveDetail, LiveState};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::RowKind;
use crate::tui::rows::seen::SEEN_REST;

/// `app` with every foldable row open, so every row shows.
fn unfolded(mut app: App) -> App {
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
    app
}

/// The cell where `needle` starts on the first line of `app` that
/// contains `key` and is not the cursor row (which the highlight draws
/// bold), with that line's text.
fn cell_of(app: &mut App, key: &str, needle: &str) -> (ratatui::buffer::Cell, String) {
    let buffer = buffer_of(app, 220, 60);
    let lines = tuikit::testkit::buffer::rows(&buffer);
    let y = lines
        .iter()
        .position(|line| {
            line.contains(key)
                && line.contains(needle)
                && !line
                    .trim_start_matches('│')
                    .starts_with(tuikit::glyphs::marks::POINTER)
        })
        .unwrap_or_else(|| panic!("no line with {key} and {needle}:\n{}", lines.join("\n")));
    let line = &lines[y];
    let (before, _) = line.split_once(needle).expect("the needle");
    let x = u16::try_from(tuikit::text::width(before)).expect("a column");
    let cell = buffer[(x, u16::try_from(y).expect("a row"))].clone();
    (cell, line.clone())
}

/// Whether `row` stands for the session `key`, whatever its kind.
fn stands_for(row: &crate::tui::rows::Row, key: &str) -> bool {
    match &row.kind {
        RowKind::AgentSession(row) => row.primary_node == session(key),
        // A pin is named for what it points at, its session's key cut
        // to its first word.
        RowKind::Mixed(mixed) => !mixed.label.is_empty() && key.starts_with(mixed.label.as_str()),
        _ => false,
    }
}

/// The row of `app` standing for the session `key`.
fn row_of(app: &App, key: &str) -> RowId {
    app.tree()
        .rows
        .iter()
        .find(|row| stands_for(row, key))
        .unwrap_or_else(|| panic!("no row for {key}"))
        .id
        .clone()
}

/// Put the cursor on the first row that `avoid` does not name, so the
/// highlight (which is bold) is not on the row a test reads.
fn park_away(app: &mut App, avoid: impl Fn(&crate::tui::rows::Row) -> bool) {
    let away = app
        .tree()
        .rows
        .iter()
        .find(|row| !avoid(row))
        .map(|row| row.id.clone())
        .expect("another row");
    app.set_selection(away);
}

#[test]
fn an_unseen_ready_mark_is_bold_and_a_seen_one_plain_with_the_same_shape() {
    // The loose session finished a turn and has no mux.
    let snapshot = with_table();
    let theme = crate::tui::Theme::default();
    // The Search view lists the session alone, always under the cursor,
    // whose highlight is bold; its rows carry the same judgment (below).
    for (view, needle) in [
        (View::Sessions, crate::tui::icons::live::READY),
        (View::Pinned, "ready"),
    ] {
        let mut app = unfolded(listing_with(view, &snapshot, LiveOverlay::off()));
        park_away(&mut app, |row| stands_for(row, SOLO));
        let (unseen, before) = cell_of(&mut app, "solo", needle);
        assert!(
            unseen.modifier.contains(Modifier::BOLD),
            "{view:?}: an unseen turn is bold:\n{before}"
        );
        assert_eq!(unseen.fg, theme.success, "{view:?}: in the ready color");

        // Rest the cursor on the session's row long enough to look at it,
        // then move it away again.
        app.set_selection(row_of(&app, SOLO));
        let start = Instant::now();
        app.note_cursor_rest(start);
        app.note_cursor_rest(start + SEEN_REST);
        park_away(&mut app, |row| stands_for(row, SOLO));
        let (seen, after) = cell_of(&mut app, "solo", needle);
        assert!(
            !seen.modifier.contains(Modifier::BOLD),
            "{view:?}: a seen turn is plain:\n{after}"
        );
        assert_eq!(seen.fg, unseen.fg, "{view:?}: the color stays");
        assert_eq!(seen.symbol(), unseen.symbol(), "{view:?}: the shape stays");
        assert_eq!(after, before, "{view:?}: nothing else on the row changes");
    }
}

#[test]
fn an_unread_row_is_bold_in_its_words_and_a_read_one_is_not() {
    // The loose session finished a turn and has no mux. Its key and its
    // label, which no mark's boldness reaches, are bold while the turn is
    // unread, and plain once it is read, in the colors they always had.
    let snapshot = with_table();
    let mut app = unfolded(listing_with(View::Sessions, &snapshot, LiveOverlay::off()));
    let (key, label) = app
        .tree()
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(row) if row.primary_node == session(SOLO) => Some((
                row.session.session_key.chars().take(8).collect::<String>(),
                row.tree_label()
                    .map(|label| label.chars().take(12).collect::<String>())
                    .expect("the session has a label"),
            )),
            _ => None,
        })
        .expect("the session's row");
    park_away(&mut app, |row| stands_for(row, SOLO));
    let unread: Vec<_> = [&key, &label]
        .into_iter()
        .map(|word| cell_of(&mut app, "solo", word))
        .collect();
    for (word, (cell, line)) in [&key, &label].into_iter().zip(&unread) {
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "`{word}` is bold while the turn is unread:\n{line}"
        );
    }

    app.set_selection(row_of(&app, SOLO));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    park_away(&mut app, |row| stands_for(row, SOLO));
    for (word, (before, text)) in [&key, &label].into_iter().zip(&unread) {
        let (after, line) = cell_of(&mut app, "solo", word);
        assert!(
            !after.modifier.contains(Modifier::BOLD),
            "`{word}` is plain once read:\n{line}"
        );
        assert_eq!(after.fg, before.fg, "`{word}`: the color stays");
        assert_eq!(&line, text, "`{word}`: only the weight changes");
    }
}

#[test]
fn an_unread_mux_row_is_bold_in_its_name_and_a_read_one_is_not() {
    let world = |last_attached: Option<i64>| {
        let mut snapshot = with_table();
        snapshot
            .node_live
            .retain(|node, _| *node == session(HOSTED));
        snapshot.pane_live.clear();
        snapshot.node_live.insert(
            session(HOSTED),
            running(LiveState::Ready, Some(LiveDetail::Finished), 0),
        );
        for node in &mut snapshot.nodes {
            if let GraphNode::MuxSession(node) = node
                && node.id == mux("conspectus-main")
            {
                node.last_attached_epoch = last_attached;
            }
        }
        snapshot
    };
    let name = |snapshot: &GraphSnapshot| {
        let mut app = unfolded(listing_with(View::Mux, snapshot, LiveOverlay::off()));
        park_away(
            &mut app,
            |row| matches!(&row.kind, RowKind::MuxSession(row) if row.mux == mux("conspectus-main")),
        );
        cell_of(&mut app, "conspectus-main", "conspectus-main")
    };
    let (unread, before) = name(&world(None));
    assert!(
        unread.modifier.contains(Modifier::BOLD),
        "no client attached since the turn ended:\n{before}"
    );
    let (read, after) = name(&world(Some(NOW)));
    assert!(!read.modifier.contains(Modifier::BOLD), "{after}");
    assert_eq!(read.fg, unread.fg, "the name keeps its color");
    assert_eq!(after, before, "only the weight changes");
}

#[test]
fn looking_at_a_session_turns_every_listing_of_it_plain() {
    let snapshot = with_table();
    // The Search view lists the session found, the cursor on it.
    let mut app = listing_with(View::Search, &snapshot, LiveOverlay::off());
    let judged = |app: &App| -> Vec<bool> {
        app.tree()
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(row) if row.primary_node == session(SOLO) => {
                    row.live.map(|cell| cell.unseen)
                }
                RowKind::Mixed(mixed) => match mixed.live {
                    crate::tui::rows::MixedLive::Session(cell)
                        if !mixed.label.is_empty() && SOLO.starts_with(&mixed.label) =>
                    {
                        Some(cell.unseen)
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    };
    assert_eq!(judged(&app), [true], "found, and not looked at yet");
    app.set_selection(row_of(&app, SOLO));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    // The memory belongs to the TUI, not the view: every view the
    // operator switches to reads the turn as seen.
    for view in [View::Search, View::Pinned, View::Sessions] {
        app.update(Msg::SwitchView(view));
        let unseen = judged(&app);
        assert!(!unseen.is_empty(), "{view:?} lists the session");
        assert!(
            unseen.iter().all(|unseen| !unseen),
            "{view:?} still reads the turn unseen"
        );
    }
}

#[test]
fn a_mux_whose_ready_agent_is_unseen_draws_its_mark_bold() {
    let theme = crate::tui::Theme::default();
    let world = |last_attached: Option<i64>| {
        let mut snapshot = with_table();
        snapshot
            .node_live
            .retain(|node, _| *node == session(HOSTED));
        snapshot.pane_live.clear();
        snapshot.node_live.insert(
            session(HOSTED),
            running(LiveState::Ready, Some(LiveDetail::Finished), 0),
        );
        for node in &mut snapshot.nodes {
            if let GraphNode::MuxSession(node) = node
                && node.id == mux("conspectus-main")
            {
                node.last_attached_epoch = last_attached;
            }
        }
        snapshot
    };
    let mark = |snapshot: &GraphSnapshot| {
        let mut app = unfolded(listing_with(View::Mux, snapshot, LiveOverlay::off()));
        park_away(
            &mut app,
            |row| matches!(&row.kind, RowKind::MuxSession(row) if row.mux == mux("conspectus-main")),
        );
        // The mux row, not its nested agents: the line with its name and
        // its age.
        let (cell, line) = cell_of(&mut app, "conspectus-main", crate::tui::icons::live::READY);
        (cell, line)
    };
    let (unseen, before) = mark(&world(None));
    assert!(
        unseen.modifier.contains(Modifier::BOLD),
        "no client has attached since the turn ended:\n{before}"
    );
    assert_eq!(unseen.fg, theme.success);
    // A client attached after the turn ended (it ended 90 seconds before
    // the fixture's time).
    let (seen, after) = mark(&world(Some(NOW)));
    assert!(!seen.modifier.contains(Modifier::BOLD), "{after}");
    assert_eq!(seen.symbol(), unseen.symbol());
    assert_eq!(after, before);
}

#[test]
fn every_listing_of_an_entity_carries_one_judgment() {
    // `one_entity_carries_one_live_value_in_every_view` checks every
    // listing carries one value, the judgment included; here the world
    // has one unseen and one seen turn, so both are carried.
    let mut snapshot = with_table();
    snapshot.node_live.insert(
        session(HOSTED),
        running(LiveState::Ready, Some(LiveDetail::Finished), 0),
    );
    for node in &mut snapshot.nodes {
        if let GraphNode::MuxSession(node) = node
            && node.id == mux("conspectus-main")
        {
            node.last_attached_epoch = Some(NOW);
        }
    }
    let mut judged = std::collections::BTreeMap::<NodeId, Vec<bool>>::new();
    for view in ALL_VIEWS {
        let app = listing_with(view, &snapshot, LiveOverlay::off());
        for row in &app.tree().rows {
            if let RowKind::AgentSession(row) = &row.kind
                && let Some(cell) = row.live
                && cell.state == LiveState::Ready
            {
                judged
                    .entry(row.primary_node.clone())
                    .or_default()
                    .push(cell.unseen);
            }
        }
    }
    assert_eq!(
        judged
            .get(&session(HOSTED))
            .map(|all| all.iter().all(|u| !u)),
        Some(true),
        "its mux was attached after it ended: {judged:?}"
    );
    assert_eq!(
        judged.get(&session(SOLO)).map(|all| all.iter().all(|u| *u)),
        Some(true),
        "no mux, never rested on: {judged:?}"
    );
}

/// The toast text among `effects`, if any.
fn toast_text(effects: &[crate::tui::Effect]) -> Option<String> {
    effects.iter().find_map(|effect| match effect {
        crate::tui::Effect::Toast(feedback) => Some(feedback.text.clone()),
        _ => None,
    })
}

/// Whether `effects` ask for the state file to be written.
fn persists(effects: &[crate::tui::Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, crate::tui::Effect::Persist))
}

#[test]
fn marking_read_acts_on_the_entity_in_view_in_every_view_that_lists_it() {
    let snapshot = with_table();
    for view in [View::Sessions, View::Pinned] {
        let mut app = unfolded(listing_with(view, &snapshot, LiveOverlay::off()));
        let others = app.unread_of(None).len();
        let solo = session(SOLO);
        assert_eq!(app.unread_of(Some(&solo)).len(), 1, "{view:?}: unread");
        app.set_selection(row_of(&app, SOLO));
        let effects = app.update(Msg::MarkRead);
        assert_eq!(
            toast_text(&effects).as_deref(),
            Some("marked 1 agent read"),
            "{view:?}"
        );
        assert!(persists(&effects), "{view:?}: the mark is saved");
        assert!(app.unread_of(Some(&solo)).is_empty(), "{view:?}: read");
        assert_eq!(
            app.unread_of(None).len(),
            others - 1,
            "{view:?}: only that agent was read"
        );
        // Nothing left to read there: it says so and saves nothing.
        let effects = app.update(Msg::MarkRead);
        assert_eq!(toast_text(&effects).as_deref(), Some("nothing unread here"));
        assert!(!persists(&effects), "{view:?}: no change, no write");
    }
}

#[test]
fn marking_a_mux_read_reads_the_agents_counted_on_it_and_no_others() {
    let snapshot = with_table();
    let mut app = unfolded(listing_with(View::Mux, &snapshot, LiveOverlay::off()));
    let main = NodeId::MuxSession(mux("conspectus-main"));
    assert_eq!(app.unread_of(Some(&main)).len(), 2, "hosted and beside");
    let before = app.unread_of(None).len();
    app.set_selection(RowId::MuxSession(main.clone()));
    let effects = app.update(Msg::MarkRead);
    assert_eq!(
        toast_text(&effects).as_deref(),
        Some("marked 2 agents read")
    );
    assert!(app.unread_of(Some(&main)).is_empty());
    assert_eq!(
        app.unread_of(None).len(),
        before - 2,
        "the loose session and the other mux's panes stay unread"
    );
}

#[test]
fn marking_read_on_a_row_with_no_agent_says_so_and_changes_nothing() {
    let snapshot = with_table();
    let mut app = unfolded(listing_with(View::Sessions, &snapshot, LiveOverlay::off()));
    let group = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(row.id, RowId::Group(_)))
        .expect("a group row")
        .id
        .clone();
    let before = app.unread_of(None);
    app.set_selection(group);
    let effects = app.update(Msg::MarkRead);
    assert_eq!(
        toast_text(&effects).as_deref(),
        Some("nothing to mark read: this row has no agent")
    );
    assert_eq!(app.unread_of(None), before);
    assert!(!persists(&effects));
}

#[test]
fn clearing_all_unread_reads_every_agent_in_every_view_and_saves_once() {
    let snapshot = with_table();
    for view in [View::Sessions, View::Mux, View::Pinned] {
        let mut app = unfolded(listing_with(view, &snapshot, LiveOverlay::off()));
        let unread = app.unread_of(None).len();
        assert!(unread >= 4, "{view:?}: sessions and panes wait: {unread}");
        let effects = app.update(Msg::ClearUnread);
        assert_eq!(
            toast_text(&effects),
            Some(format!("cleared {unread} unread agents")),
            "{view:?}"
        );
        assert!(persists(&effects), "{view:?}");
        assert!(app.unread_of(None).is_empty(), "{view:?}: every agent read");
        assert!(
            app.build_persisted_state().read_marks.len() >= 3,
            "{view:?}: the sessions' marks are in the state file's payload"
        );
        let effects = app.update(Msg::ClearUnread);
        assert_eq!(toast_text(&effects).as_deref(), Some("nothing unread"));
        assert!(!persists(&effects), "{view:?}");
    }
}

#[test]
fn the_row_menu_offers_mark_read_and_clear_all_unread_only_while_they_apply() {
    use crate::tui::actions::row_menu_for;
    let snapshot = with_table();
    let mut app = unfolded(listing_with(View::Sessions, &snapshot, LiveOverlay::off()));
    let labels = |app: &App| -> Vec<String> {
        row_menu_for(app)
            .map(|menu| {
                menu.entries()
                    .iter()
                    .map(|entry| entry.label.clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    app.set_selection(row_of(&app, SOLO));
    let before = labels(&app);
    assert!(before.contains(&"Mark read".to_string()), "{before:?}");
    assert!(
        before.contains(&"Clear all unread".to_string()),
        "{before:?}"
    );
    // The menu's shortcuts are the key table's.
    let menu = row_menu_for(&app).expect("a menu");
    let shortcut = |label: &str| {
        menu.entries()
            .iter()
            .find(|entry| entry.label == label)
            .and_then(|entry| entry.shortcut)
    };
    assert_eq!(
        shortcut("Mark read").map(|key| tuikit::keys::key_label(&key)),
        Some(crate::tui::keybindings::key_name("read.mark"))
    );
    assert_eq!(
        shortcut("Clear all unread").map(|key| tuikit::keys::key_label(&key)),
        Some(crate::tui::keybindings::key_name("read.clear"))
    );
    app.update(Msg::MarkRead);
    let after = labels(&app);
    assert!(!after.contains(&"Mark read".to_string()), "read: {after:?}");
    assert!(
        after.contains(&"Clear all unread".to_string()),
        "others remain"
    );
    app.update(Msg::ClearUnread);
    assert!(!labels(&app).contains(&"Clear all unread".to_string()));
}

#[test]
fn an_unread_wait_and_an_unread_error_draw_their_marks_bold_in_the_sessions_and_mux_views() {
    // The same weight as a finished turn's, on the other two stops: the
    // shape and the color stay, and attaching to the mux plains them.
    for (state, detail, glyph) in [
        (
            LiveState::Waiting,
            LiveDetail::Permission,
            crate::tui::icons::live::WAITING,
        ),
        (
            LiveState::Error,
            LiveDetail::Failed,
            crate::tui::icons::live::ERROR,
        ),
    ] {
        let world = |last_attached: Option<i64>| {
            let mut snapshot = with_table();
            snapshot
                .node_live
                .retain(|node, _| *node == session(HOSTED));
            snapshot.pane_live.clear();
            snapshot
                .node_live
                .insert(session(HOSTED), running(state, Some(detail), 0));
            for node in &mut snapshot.nodes {
                if let GraphNode::MuxSession(node) = node
                    && node.id == mux("conspectus-main")
                {
                    node.last_attached_epoch = last_attached;
                }
            }
            snapshot
        };
        let mark = |snapshot: &GraphSnapshot, view: View, key: &str| {
            let mut app = unfolded(listing_with(view, snapshot, LiveOverlay::off()));
            park_away(&mut app, |row| {
                matches!(&row.kind, RowKind::MuxSession(row) if row.mux == mux("conspectus-main"))
                    || stands_for(row, HOSTED)
            });
            cell_of(&mut app, key, glyph)
        };
        for (view, key) in [(View::Mux, "conspectus-main"), (View::Sessions, "e44d01cb")] {
            let (unread, before) = mark(&world(None), view, key);
            assert!(
                unread.modifier.contains(Modifier::BOLD),
                "{state:?} in {view:?}: nothing has read the stop:\n{before}"
            );
            let (read, after) = mark(&world(Some(NOW)), view, key);
            assert!(
                !read.modifier.contains(Modifier::BOLD),
                "{state:?} in {view:?}: a client attached since:\n{after}"
            );
            assert_eq!(read.fg, unread.fg, "{state:?} in {view:?}: the color stays");
            assert_eq!(read.symbol(), unread.symbol(), "{state:?} in {view:?}");
            assert_eq!(after, before, "{state:?} in {view:?}: nothing else changes");
        }
    }
}
