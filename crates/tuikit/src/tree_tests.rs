use std::cell::Cell;
use std::collections::BTreeSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;
use crate::scroll::ScrollWindow;
use crate::testkit::buffer::rows;
use crate::theme::Theme;

type Tree = Vec<Row<u32, &'static str>>;

/// A row with `id` at `depth`, its label the payload.
const fn row(id: u32, depth: u16, expandable: bool, label: &'static str) -> Row<u32, &'static str> {
    Row {
        id,
        depth,
        expandable,
        payload: label,
    }
}

/// Two groups of three, the second with a nested group of two:
///
/// ```text
/// 1 a        (0)
///   2 a1     (1)
///   3 a2     (2)
///   4 a3     (3)
/// 5 b        (4)
///   6 b1     (5)
///   7 bb     (6)
///     8 bb1  (7)
///     9 bb2  (8)
///   10 b2    (9)
/// ```
fn groups() -> Tree {
    vec![
        row(1, 0, true, "a"),
        row(2, 1, false, "a1"),
        row(3, 1, false, "a2"),
        row(4, 1, false, "a3"),
        row(5, 0, true, "b"),
        row(6, 1, false, "b1"),
        row(7, 1, true, "bb"),
        row(8, 2, false, "bb1"),
        row(9, 2, false, "bb2"),
        row(10, 1, false, "b2"),
    ]
}

fn open(ids: &[u32]) -> TreeState<u32> {
    let mut state = TreeState::new();
    state.set_expanded(ids.iter().copied().collect());
    state
}

fn shown(state: &TreeState<u32>, tree: &Tree) -> Vec<u32> {
    state.visible(tree).iter().map(|row| row.id).collect()
}

#[test]
fn a_collapsed_row_hides_every_deeper_row_below_it() {
    let tree = groups();
    assert_eq!(shown(&open(&[]), &tree), [1, 5]);
    assert_eq!(shown(&open(&[1]), &tree), [1, 2, 3, 4, 5]);
    assert_eq!(shown(&open(&[5]), &tree), [1, 5, 6, 7, 10]);
    assert_eq!(shown(&open(&[5, 7]), &tree), [1, 5, 6, 7, 8, 9, 10]);
    // A nested row's own fold does not matter while its parent is closed.
    assert_eq!(shown(&open(&[7]), &tree), [1, 5]);
    assert_eq!(
        open(&[1, 5, 7]).visible_indices(&tree),
        (0..10).collect::<Vec<_>>()
    );
}

#[test]
fn a_row_without_children_hides_nothing_even_when_not_expanded() {
    let tree = vec![row(1, 0, false, "a"), row(2, 1, false, "a1")];
    assert_eq!(shown(&open(&[]), &tree), [1, 2]);
}

#[test]
fn the_cursor_steps_through_showing_rows_and_stops_at_the_ends() {
    let tree = groups();
    let mut state = open(&[5]);
    assert_eq!(
        state.step(&tree, 1),
        Some(1),
        "from nothing, a step leaves the first row"
    );
    assert_eq!(state.selected(), Some(&5));
    assert_eq!(state.step(&tree, 10), Some(4));
    assert_eq!(state.selected(), Some(&10));
    assert_eq!(state.step(&tree, 1), Some(4), "it stops at the last row");
    assert_eq!(state.step(&tree, -100), Some(0));
    assert_eq!(state.selected(), Some(&1));
    assert_eq!(state.end(&tree), Some(4));
    assert_eq!(state.home(&tree), Some(0));
}

#[test]
fn a_page_is_the_rows_the_last_frame_showed() {
    let tree = groups();
    let mut state = open(&[1, 5, 7]);
    state.home(&tree);
    assert_eq!(
        state.page_down(&tree),
        Some(1),
        "before a frame a page is a row"
    );
    state.set_viewport(ScrollWindow::new(4, 10));
    assert_eq!(state.page_down(&tree), Some(5));
    assert_eq!(state.page_down(&tree), Some(9), "a page stops at the end");
    assert_eq!(state.page_up(&tree), Some(5));
    assert_eq!(state.page_up(&tree), Some(1));
    assert_eq!(state.page_up(&tree), Some(0));
}

#[test]
fn no_showing_rows_select_nothing() {
    let empty: Tree = Vec::new();
    let mut state = TreeState::<u32>::new();
    assert_eq!(state.step(&empty, 1), None);
    assert_eq!(state.end(&empty), None);
    assert_eq!(state.selected(), None);
    assert_eq!(state.position(), None);
}

/// One item listed under two groups.
fn duplicated() -> Tree {
    vec![
        row(1, 0, true, "a"),
        row(9, 1, false, "shared"),
        row(2, 0, true, "b"),
        row(9, 1, false, "shared"),
        row(3, 1, false, "b1"),
    ]
}

#[test]
fn stepping_from_the_second_copy_of_a_row_moves_on_from_it() {
    let tree = duplicated();
    let mut state = open(&[1, 2]);
    state.home(&tree);
    state.step(&tree, 3);
    assert_eq!((state.selected(), state.cursor(&tree)), (Some(&9), Some(3)));
    assert_eq!(state.step(&tree, 1), Some(4));
    assert_eq!(state.selected(), Some(&3));
}

#[test]
fn a_jump_lands_on_the_first_showing_copy() {
    let tree = duplicated();
    let mut state = open(&[1, 2]);
    state.end(&tree);
    assert!(state.select(&tree, 9));
    assert_eq!(state.position(), None);
    assert_eq!(state.cursor(&tree), Some(1));
    assert!(
        !state.select(&tree, 42),
        "an identity no row has is not selected"
    );
    assert_eq!(state.selected(), Some(&9));
}

#[test]
fn folding_acts_on_the_selected_row_with_children_only() {
    let tree = groups();
    let mut state = open(&[]);
    state.home(&tree);
    assert!(state.expand(&tree));
    assert!(!state.expand(&tree), "already expanded");
    assert!(state.is_expanded(&1));
    assert!(state.collapse(&tree));
    assert!(!state.collapse(&tree), "already collapsed");
    assert!(state.toggle(&tree));
    assert!(state.is_expanded(&1));
    assert!(state.toggle(&tree));
    assert!(!state.is_expanded(&1));

    state.expand(&tree);
    state.step(&tree, 1);
    assert_eq!(state.selected(), Some(&2));
    let folds = state.expanded().clone();
    assert!(!state.expand(&tree) && !state.collapse(&tree) && !state.toggle(&tree));
    assert_eq!(state.expanded(), &folds, "a leaf changes no fold");
}

#[test]
fn revealing_a_row_expands_its_ancestors_only() {
    let tree = groups();
    let mut state = open(&[]);
    state.reveal(&tree, 8);
    assert_eq!(
        state.expanded(),
        &BTreeSet::from([5, 7]),
        "the group and the nested group above it"
    );
    assert!(state.select(&tree, 9));
    assert_eq!(state.cursor(&tree), Some(5));
}

#[test]
fn a_rebuild_keeps_the_cursor_on_its_row() {
    let tree = groups();
    let mut state = open(&[1, 5]);
    state.home(&tree);
    state.step(&tree, 5);
    assert_eq!(state.selected(), Some(&6));
    // Two rows join the first group above it.
    let mut rebuilt = tree.clone();
    rebuilt.insert(1, row(11, 1, false, "a0"));
    rebuilt.insert(1, row(12, 1, false, "a00"));
    state.rebuild(&tree, &rebuilt, None);
    assert_eq!(state.selected(), Some(&6));
    assert_eq!(state.cursor(&rebuilt), Some(7));
}

#[test]
fn a_rebuild_without_the_cursors_row_stays_where_it_was() {
    let tree = groups();
    let mut state = open(&[1, 5]);
    state.end(&tree);
    assert_eq!(
        (state.selected(), state.cursor(&tree)),
        (Some(&10), Some(7))
    );
    let rebuilt: Tree = tree.iter().filter(|row| row.id != 10).cloned().collect();
    state.rebuild(&tree, &rebuilt, None);
    assert_eq!(
        state.selected(),
        Some(&7),
        "the same position, the last row now"
    );
    let short = vec![row(1, 0, true, "a"), row(2, 1, false, "a1")];
    state.rebuild(&rebuilt, &short, None);
    assert_eq!(state.selected(), Some(&2), "fewer rows: the last");
}

#[test]
fn a_first_load_opens_on_the_hint_and_a_later_one_ignores_it() {
    let tree = groups();
    let mut state = open(&[1, 5]);
    state.rebuild(&[], &tree, Some(&6));
    assert_eq!(state.selected(), Some(&6));
    state.rebuild(&tree, &tree, Some(&3));
    assert_eq!(state.selected(), Some(&6), "a selection beats the hint");

    let mut fresh = open(&[]);
    fresh.rebuild(&[], &tree, Some(&6));
    assert_eq!(
        fresh.selected(),
        Some(&1),
        "a hidden hint gives the first row"
    );
}

#[test]
fn a_rebuild_keeps_the_copy_the_cursor_was_on() {
    let tree = duplicated();
    let mut state = open(&[1, 2]);
    state.home(&tree);
    state.step(&tree, 3);
    let before = state.begin_rebuild(&tree);
    assert_eq!((before.selected(), before.position()), (Some(&9), Some(3)));
    assert_eq!(
        state.selected(),
        None,
        "the cursor is off the rows meanwhile"
    );
    state.finish_rebuild(before, &tree, None);
    assert_eq!(state.cursor(&tree), Some(3));
}

#[test]
fn a_rebuild_to_no_rows_selects_nothing() {
    let tree = groups();
    let mut state = open(&[]);
    state.home(&tree);
    state.rebuild(&tree, &[], None);
    assert_eq!((state.selected(), state.position()), (None, None));
}

#[test]
fn the_window_follows_the_cursor_inside_the_showing_rows() {
    let tree = groups();
    let mut state = open(&[1, 5, 7]);
    state.end(&tree);
    let window = state.placed_window(&tree, 4);
    assert_eq!(
        (window.offset(), window.viewport(), window.content()),
        (6, 4, 10)
    );
    state.set_viewport(window);
    // Collapsing the second group leaves fewer rows: the window comes back.
    state.select(&tree, 5);
    state.collapse(&tree);
    let window = state.placed_window(&tree, 4);
    assert_eq!((window.offset(), window.content()), (1, 5));
    state.scroll_to_top();
    assert_eq!(state.window().offset(), 0);
}

// ---- the pane ---------------------------------------------------------------

fn label(row: &Row<u32, &'static str>, cx: &RowCtx<'_>) -> Line<'static> {
    let fold = match (row.expandable, cx.expanded) {
        (false, _) => "  ",
        (true, true) => "- ",
        (true, false) => "+ ",
    };
    Line::from(vec![
        Span::raw("  ".repeat(usize::from(row.depth))),
        Span::raw(fold),
        Span::styled(row.payload, Style::default().add_modifier(Modifier::BOLD)),
    ])
}

fn draw<F>(
    view: TreeView<'_, Row<u32, &'static str>, F>,
    width: u16,
    height: u16,
) -> (Buffer, Option<ScrollWindow>)
where
    F: Fn(&Row<u32, &'static str>, &RowCtx<'_>) -> Line<'static>,
{
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let window = view.render(area, &mut buf);
    (buf, window)
}

#[test]
fn the_pane_draws_its_title_rows_and_scrollbar() {
    let theme = Theme::default();
    let tree = groups();
    let mut state = open(&[1, 5, 7]);
    state.home(&tree);
    let view = TreeView::new(&tree, &state, label, &theme).title(Line::from(" tabs "));
    let (buf, window) = draw(view, 16, 8);
    let text = rows(&buf);
    assert_eq!(text[0], "┌ tabs ────────┐");
    assert!(text[1].starts_with("│- a  "), "{text:?}");
    assert!(text[2].starts_with("│    a1  "), "{text:?}");
    assert_eq!(window, Some(ScrollWindow::new(6, 10)));
    assert!(buf[(3, 1)].modifier.contains(theme.selection_active));
    // The gutter's column, inside its one-row margins, holds the bar.
    assert_eq!(buf[(14, 1)].symbol(), " ");
    assert!((2..6).all(|y| buf[(14, y)].symbol() != " "), "{text:?}");
    assert_eq!(buf[(14, 6)].symbol(), " ");
}

#[test]
fn rows_that_fit_leave_the_gutter_to_the_rows() {
    let theme = Theme::default();
    let tree = groups();
    let state = open(&[]);
    let (buf, window) = draw(TreeView::new(&tree, &state, label, &theme), 10, 6);
    let text = rows(&buf);
    assert_eq!(text[1], "│+ a     │");
    assert_eq!(text[2], "│+ b     │");
    assert_eq!(window.map(|window| window.overflows()), Some(false));
}

#[test]
fn the_header_and_the_footer_take_rows_only_when_the_pane_has_room() {
    let theme = Theme::default();
    let tree = groups();
    let state = open(&[1]);
    let warning = Style::default().fg(theme.warning);
    let view = || {
        TreeView::new(&tree, &state, label, &theme)
            .header(Line::from("heading"))
            .footer(
                vec![Line::from("note one"), Line::from("note two")],
                warning,
            )
    };
    let (buf, window) = draw(view(), 14, 9);
    let text = rows(&buf);
    assert_eq!(text[1], "│heading     │");
    assert_eq!(text[2], "│- a         │");
    assert_eq!(text[6], "│note one    │");
    assert_eq!(text[7], "│note two    │");
    assert_eq!(buf[(1, 6)].fg, theme.warning);
    assert_eq!(window.map(|window| window.viewport()), Some(4));

    // Four rows inside the border: no banner; three: no header either.
    let text = rows(&draw(view(), 14, 6).0);
    assert_eq!(text[1], "│heading     │");
    assert_eq!(text[4], "│    a2      │");
    let text = rows(&draw(view(), 14, 5).0);
    assert_eq!(text[1], "│- a         │");
}

#[test]
fn a_long_footer_takes_three_rows_at_most() {
    let theme = Theme::default();
    let tree = groups();
    let state = open(&[]);
    let lines = vec![Line::from("one two three four five six seven eight")];
    let view = TreeView::new(&tree, &state, label, &theme).footer(lines, Style::default());
    let text = rows(&draw(view, 10, 10).0);
    assert_eq!(text[6], "│one two │");
    assert_eq!(text[7], "│three   │");
    assert_eq!(text[8], "│four    │");
    assert_eq!(text[5], "│        │");
}

#[test]
fn no_showing_rows_draw_the_placeholder_and_report_no_window() {
    let theme = Theme::default();
    let tree: Tree = Vec::new();
    let mut state = TreeState::new();
    state.set_viewport(ScrollWindow::new(5, 20).at(7));
    let view = TreeView::new(&tree, &state, label, &theme).empty("Nothing to show here.");
    let (buf, window) = draw(view, 14, 6);
    assert_eq!(
        window, None,
        "the kept window stays for when rows come back"
    );
    let text = rows(&buf);
    assert_eq!(text[1], "│Nothing to  │");
    assert_eq!(text[2], "│show here.  │");
    assert!(buf[(1, 1)].modifier.contains(theme.muted));
}

#[test]
fn the_unfocused_cursor_row_takes_the_inactive_look() {
    let theme = Theme::default();
    let tree = groups();
    let mut state = open(&[]);
    state.end(&tree);
    let view = TreeView::new(&tree, &state, label, &theme).focused(false);
    let (buf, _) = draw(view, 10, 5);
    // The fold mark of the cursor row carries no bold of its own.
    assert_eq!(buf[(1, 2)].symbol(), "+");
    assert!(buf[(1, 2)].modifier.contains(theme.selection_inactive));
    assert!(!buf[(1, 2)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(1, 1)].modifier.contains(theme.selection_inactive));
}

#[test]
fn row_states_give_rows_the_ghost_and_pending_looks() {
    let theme = Theme::default();
    let tree = groups();
    let state = open(&[]);
    let states = [
        RowState::default(),
        RowState::default(),
        RowState::default(),
        RowState::default(),
        RowState {
            ghost: true,
            pending: true,
            mark: RowMark::On,
        },
    ];
    let marks = Cell::new(0);
    let producer = |row: &Row<u32, &'static str>, cx: &RowCtx<'_>| {
        if cx.mark == RowMark::On {
            marks.set(marks.get() + 1);
        }
        label(row, cx)
    };
    let view = TreeView::new(&tree, &state, producer, &theme).states(&states);
    let (buf, _) = draw(view, 10, 5);
    let ghost = &buf[(3, 2)];
    assert_eq!(ghost.symbol(), "b");
    assert_eq!(ghost.fg, theme.inert);
    assert!(ghost.modifier.contains(Modifier::ITALIC | theme.muted));
    assert!(
        !ghost.modifier.contains(Modifier::BOLD),
        "a ghost drops its bold"
    );
    assert!(buf[(3, 1)].modifier.contains(Modifier::BOLD));
    assert_eq!(marks.get(), 1, "the producer is told the row's mark");
}

#[test]
fn only_the_rows_the_window_shows_are_laid_out() {
    let theme = Theme::default();
    let tree: Tree = (0..200).map(|id| row(id, 0, false, "item")).collect();
    let mut state = TreeState::new();
    state.end(&tree);
    let calls = Cell::new(0);
    let producer = |row: &Row<u32, &'static str>, cx: &RowCtx<'_>| {
        calls.set(calls.get() + 1);
        label(row, cx)
    };
    let (_, window) = draw(TreeView::new(&tree, &state, producer, &theme), 12, 7);
    assert_eq!(calls.get(), 5);
    assert_eq!(window.map(|window| window.visible()), Some(195..200));
}

#[test]
fn the_producer_is_told_the_width_left_by_the_gutter() {
    let theme = Theme::default();
    let tree = groups();
    let widths = Cell::new((usize::MAX, 0));
    let producer = |row: &Row<u32, &'static str>, cx: &RowCtx<'_>| {
        let (low, high) = widths.get();
        widths.set((low.min(cx.width), high.max(cx.width)));
        label(row, cx)
    };
    draw(TreeView::new(&tree, &open(&[]), &producer, &theme), 12, 6);
    assert_eq!(widths.get(), (10, 10), "two rows fit: every column");
    widths.set((usize::MAX, 0));
    draw(
        TreeView::new(&tree, &open(&[1, 5]), &producer, &theme),
        12,
        6,
    );
    assert_eq!(widths.get(), (9, 9), "eight rows overflow: one column less");
}
