//! Trees: rows nested by depth, a cursor kept by identity across
//! rebuilds, folds, and the pane that draws them from lines the host lays
//! out.
//!
//! This is a layer 1 control (house rules 1, 2, 10, 12, 13, and 14). A
//! program that lists nested things (groups and their members, items and
//! their parts) shows them as a tree, and every such tree should move,
//! fold, scroll, and keep its place the same way:
//!
//! - **Rows.** A tree is a flat list of rows in display order. Each row
//!   has an identity, a depth, and whether it has children ([`TreeRow`]);
//!   a row's children are the deeper rows that follow it. A host
//!   implements the trait on its own row type or uses [`Row`], which
//!   carries the host's payload beside the three.
//! - **State.** [`TreeState<Id>`](TreeState) keeps what the user did: the
//!   selected row's identity and the position it showed at, the expanded
//!   rows, and the window the last frame drew. It holds no rows; the host
//!   owns them, rebuilds them as its data changes, and passes them in.
//! - **Showing.** A row shows while none of its ancestors is a collapsed
//!   row with children ([`TreeState::visible`]).
//! - **Moving.** The cursor steps through the showing rows and stops at
//!   the first and the last (house rule 2); a page is the rows the last
//!   frame showed, and home and end are the first and the last row (house
//!   rule 12). When an identity is listed twice, the cursor is on the copy
//!   nearest where it was, so stepping from the second copy moves on from
//!   it.
//! - **Folding.** The selected row expands, collapses, or toggles; a row
//!   without children ignores all three. [`TreeState::reveal`] opens the
//!   ancestors of a row a host jumps to.
//! - **Rebuilding.** One routine keeps the cursor across a rebuild of the
//!   rows: on the same identity when it still shows, else where it was,
//!   else on a row the host hints at on its first load, else on the first
//!   row ([`TreeState::begin_rebuild`], [`TreeState::finish_rebuild`]).
//! - **Drawing.** [`TreeView`] draws the tree in a bordered pane: a title
//!   on its border (a strip of tabs, say), an optional header line, the
//!   rows in a window that follows the cursor with the shared scrollbar,
//!   an optional footer banner, and a placeholder when no row shows. The
//!   host's producer lays each row out from the row and its [`RowCtx`];
//!   the tree applies the pending and ghost looks a host's [`RowState`]
//!   asks for and the cursor's highlight from the theme's selection pair,
//!   active with focus and inactive without (house rules 1 and 10). The
//!   draw returns the window it drew, which the host keeps with
//!   [`TreeState::set_viewport`]: nothing is sent from inside the draw.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::style::Modifier;
//! use ratatui::text::{Line, Span};
//! use tuikit::glyphs::marks;
//! use tuikit::text;
//! use tuikit::theme::Theme;
//! use tuikit::tree::{Row, RowCtx, TreeState, TreeView};
//!
//! /// Groups and their items, in display order.
//! fn rows(items: &[&'static str]) -> Vec<Row<String, &'static str>> {
//!     let mut rows = vec![Row { id: "fruit".into(), depth: 0, expandable: true, payload: "fruit" }];
//!     for item in items {
//!         rows.push(Row { id: format!("fruit/{item}"), depth: 1, expandable: false, payload: *item });
//!     }
//!     rows.push(Row { id: "tools".into(), depth: 0, expandable: false, payload: "tools" });
//!     rows
//! }
//!
//! /// The host lays a row out: an indent, a disclosure mark, the label.
//! fn line(row: &Row<String, &'static str>, cx: &RowCtx<'_>) -> Line<'static> {
//!     let disclosure = match (row.expandable, cx.expanded) {
//!         (false, _) => "  ".to_string(),
//!         (true, true) => format!("{} ", marks::EXPANDED),
//!         (true, false) => format!("{} ", marks::COLLAPSED),
//!     };
//!     let label = text::truncate_end(row.payload, cx.width.saturating_sub(4));
//!     Line::from(vec![
//!         Span::raw("  ".repeat(usize::from(row.depth))),
//!         Span::raw(disclosure),
//!         Span::raw(label),
//!     ])
//! }
//!
//! let theme = Theme::default();
//! let mut tree = rows(&["apple", "pear", "plum"]);
//! let mut state = TreeState::new();
//! state.expand_id("fruit".to_string());
//! state.home(&tree);
//! state.step(&tree, 2);
//! assert_eq!(state.selected().map(String::as_str), Some("fruit/pear"));
//!
//! // Draw a frame, and keep the window it drew.
//! let area = Rect::new(0, 0, 24, 5);
//! let mut buf = Buffer::empty(area);
//! let window = TreeView::new(&tree, &state, line, &theme)
//!     .title(Line::from(" fruit "))
//!     .empty("Nothing here.")
//!     .render(area, &mut buf);
//! state.set_viewport(window.expect("rows show"));
//! assert_eq!(state.window().viewport(), 3);
//! assert_eq!(buf[(5, 3)].symbol(), "p");
//! assert!(buf[(5, 3)].modifier.contains(Modifier::REVERSED), "the cursor row is highlighted");
//!
//! // A page moves by the rows the frame showed and stops at the end.
//! state.page_down(&tree);
//! assert_eq!(state.selected().map(String::as_str), Some("tools"));
//! state.step(&tree, -2);
//!
//! // Rebuilt rows keep the cursor on its row, or where it was when the row is gone.
//! let before = state.begin_rebuild(&tree);
//! tree = rows(&["apple", "plum"]);
//! state.finish_rebuild(before, &tree, None);
//! assert_eq!(state.selected().map(String::as_str), Some("fruit/plum"));
//! let before = state.begin_rebuild(&tree);
//! tree = rows(&["fig", "apple", "plum"]);
//! state.finish_rebuild(before, &tree, None);
//! assert_eq!(state.selected().map(String::as_str), Some("fruit/plum"));
//! assert_eq!(state.cursor(&tree), Some(3));
//!
//! // Collapsing the group hides its items.
//! state.home(&tree);
//! state.collapse(&tree);
//! assert_eq!(state.visible(&tree).len(), 2);
//! ```

mod row;
mod state;
mod view;

pub use row::{Row, TreeRow};
pub use state::{Rebuild, TreeState};
pub use view::{RowCtx, RowMark, RowState, TreeView};

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tree_property_tests.rs"]
mod property_tests;
