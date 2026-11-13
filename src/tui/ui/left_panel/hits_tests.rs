//! The Search view's matching turns, panes, and pane lines are laid out
//! from the search modal's hit columns: they give way by its order (the
//! chip, then the age, then the label narrows, and the snippet keeps 20
//! cells while anything else can give way), are never cut at the pane's
//! edge, and give their chip, name, turn, and age the cells the modal's
//! rows give them at the same width.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::testkit::buffer;

use super::*;
use crate::model::AgentSessionId;
use crate::tui::rows::{HitOrigin, HitRow, PaneHitRow, PaneRow};
use crate::tui::widgets::search::{
    ContentRow, ContentTarget, ContentView, SearchMode, SearchOverlayState, SearchOverlayWidget,
    turn_tag,
};
use crate::viewer::search::{ContentKinds, Grouping};

const NOW: i64 = 1_790_000_000;
const SNIPPET: &str = "we chose the quokka design for the ledger store in the end";

fn session() -> AgentSessionId {
    AgentSessionId::new("claude-code", "/state", "aaaa1111")
}

fn pane_ref() -> crate::content_search::PaneRef {
    crate::content_search::PaneRef {
        mux: crate::content_search::MuxRef {
            backend: "tmux".into(),
            socket: None,
            session: "work".into(),
        },
        pane_id: "%3".into(),
        window_index: "0".into(),
        pane_index: "1".into(),
        command: Some("zsh".into()),
    }
}

/// A matching turn, naming its session as a row of the Messages listing
/// does, or under its session row.
fn hit(flat: bool) -> Row {
    Row {
        id: RowId::Hit {
            session: NodeId::AgentSession(session()),
            turn: 3,
        },
        depth: 0,
        expandable: false,
        kind: RowKind::Hit(HitRow {
            session: session(),
            origin: flat.then(|| HitOrigin {
                label: "Fix the ledger store".into(),
                harness_label: "claude".into(),
            }),
            turn_index: 3,
            role: "you",
            kind: "message",
            age: "1h".into(),
            age_epoch: Some(NOW - 3_600),
            snippet: SNIPPET.into(),
            highlight: 13..19,
        }),
    }
}

/// A pane whose scrollback matched.
fn pane() -> Row {
    Row {
        id: RowId::Pane {
            mux: NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:work")),
            pane: "%3".into(),
        },
        depth: 0,
        expandable: false,
        kind: RowKind::Pane(PaneRow {
            pane: pane_ref(),
            mux_label: "work".into(),
            location: "0.1".into(),
            command: Some("zsh".into()),
            count: 4,
            deduplicated: 0,
            age: "2m".into(),
            age_epoch: Some(NOW - 120),
            line: 17,
            snippet: SNIPPET.into(),
            highlight: 13..19,
        }),
    }
}

/// A matching pane line, naming its pane or under its pane row.
fn pane_hit(flat: bool) -> Row {
    Row {
        id: RowId::PaneHit {
            mux: NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:work")),
            pane: "%3".into(),
            line: 17,
        },
        depth: 0,
        expandable: false,
        kind: RowKind::PaneHit(PaneHitRow {
            pane: pane_ref(),
            origin: flat.then(|| "work 0.1".to_string()),
            line: 17,
            age: "2m".into(),
            age_epoch: Some(NOW - 120),
            snippet: SNIPPET.into(),
            highlight: 13..19,
        }),
    }
}

/// `row` laid out alone at `width`, as text.
fn drawn(row: &Row, width: usize) -> String {
    let line = lay_out(
        std::slice::from_ref(row),
        &Theme::default(),
        NOW,
        width,
        None,
    )
    .remove(0);
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn a_hit_naming_its_session_gives_way_in_the_modals_order() {
    let row = hit(true);
    // Wide: the chip, the name (a fifth of the row), the turn, the age, and
    // the whole snippet.
    let wide = drawn(&row, 130);
    assert!(wide.contains(" claude "), "{wide}");
    assert!(wide.contains("Fix the ledger store"), "{wide}");
    assert!(wide.contains("turn 4 · message"), "{wide}");
    assert!(wide.contains("1h"), "{wide}");
    assert!(wide.ends_with(SNIPPET), "{wide}");
    // The chip goes first.
    let narrower = drawn(&row, 65);
    assert!(!narrower.contains(" claude "), "{narrower}");
    assert!(narrower.contains("1h"), "{narrower}");
    // Then the age.
    let narrow = drawn(&row, 56);
    assert!(!narrow.contains("1h"), "{narrow}");
    assert!(narrow.contains("quokka"), "{narrow}");
    // Then the name narrows, cut with `…`, so the snippet keeps 20 cells.
    let narrowest = drawn(&row, 50);
    assert!(narrowest.contains("Fix the l…"), "{narrowest}");
    assert!(narrowest.contains("quokka"), "{narrowest}");
    for width in 0..140 {
        let text = drawn(&row, width);
        assert!(
            tuikit::text::width(&text) <= width,
            "{width}: {text:?} runs past the pane"
        );
    }
}

#[test]
fn panes_and_their_lines_take_the_same_columns() {
    let wide = drawn(&pane(), 120);
    assert!(
        wide.starts_with("  ▣ zsh"),
        "the mux glyph, then the command: {wide}"
    );
    assert!(wide.contains("work 0.1"), "{wide}");
    assert!(wide.contains("4 hits"), "{wide}");
    assert!(wide.contains("2m"), "{wide}");
    let flat = drawn(&pane_hit(true), 120);
    assert!(flat.starts_with("  zsh"), "no kind glyph: {flat}");
    assert!(
        flat.contains("work 0.1") && flat.contains("line 17"),
        "{flat}"
    );
    // A line under its pane row says only where it is.
    let nested = drawn(&pane_hit(false), 120);
    assert!(nested.starts_with("  line 17"), "{nested}");
    assert!(
        !nested.contains("zsh") && !nested.contains("work"),
        "{nested}"
    );
    let nested = drawn(&hit(false), 120);
    assert!(nested.starts_with("  turn 4 · message"), "{nested}");
    for width in 0..140 {
        for row in [pane(), pane_hit(true), pane_hit(false), hit(false)] {
            let text = drawn(&row, width);
            assert!(tuikit::text::width(&text) <= width, "{width}: {text:?}");
        }
    }
}

/// The modal's row for the same hit as [`hit`], drawn on a terminal
/// `columns` wide: its text inside the frame, the cursor gutter included,
/// and the cells it is laid out in (the frame less its reserved scrollbar
/// column).
fn modal_row(columns: u16) -> (String, usize) {
    let theme = Theme::default();
    let content = ContentView {
        rows: vec![ContentRow {
            target: ContentTarget::Session {
                session: session(),
                turn_index: 3,
            },
            label: "Fix the ledger store".into(),
            chip: "claude".into(),
            hits: 2,
            turn: Some(turn_tag(3, "message")),
            age: "1h".into(),
            recency: None,
            snippet: SNIPPET.into(),
            highlight: Some(13..19),
        }],
        scanned: 1,
        total: 1,
        done: true,
        ..ContentView::default()
    };
    let mut state = SearchOverlayState::configured(
        SearchMode::Content,
        Grouping::Flat,
        ContentKinds::default(),
        true,
        "quokka",
    );
    let area = Rect::new(0, 0, columns, 30);
    let bare = Buffer::empty(area);
    let mut framed = Buffer::empty(area);
    SearchOverlayWidget::new(&mut state, &[], &theme)
        .with_content(&content)
        .render(area, &mut framed);
    let rect = buffer::frame_rect(&framed, &bare).expect("the modal's frame");
    let row = buffer::inner_rows(&framed, rect)
        .into_iter()
        .find(|row| row.contains("Fix the"))
        .expect("the hit's row");
    (row, usize::from(rect.width) - 3)
}

#[test]
fn a_hit_reads_as_the_modal_reads_it_at_the_same_width() {
    let gutter = tuikit::selection::GUTTER_WIDTH;
    let after_lead = |text: &str, lead: usize| text.chars().skip(lead).collect::<String>();
    // Both rows start with a two-cell cursor gutter, and the tree's then
    // with its disclosure cell (blank on a hit). Where the modal's source
    // word has given way, a tree whose rows are that cell wider reads as
    // the modal after their leads.
    let (modal, width) = modal_row(60);
    assert!(!modal.contains("session"), "{modal}");
    let view = drawn(&hit(true), width);
    assert_eq!(
        after_lead(&view, 2),
        after_lead(&modal, gutter).trim_end(),
        "at {width} cells and a tree two cells wider"
    );
    // Wider, the modal shows its source word and the tree does not. At one
    // width, the gutters included, the chip, the name, the turn, and the
    // age take the same cells: a share of the row and the turn's steps
    // count the gutter in both.
    let (modal, width) = modal_row(160);
    assert!(modal.contains("session  "), "{modal}");
    let view = drawn(&hit(true), width - gutter);
    let cells = 11 + width / 5 + 2 + 22 + 2 + 4;
    let modal_cells: String = modal.chars().skip(gutter + 8).take(cells).collect();
    let view_cells: String = view.chars().skip(2).take(cells).collect();
    assert_eq!(view_cells, modal_cells, "at {width} cells");
}
