//! Multi-select list overlay primitive (ADR 0031, F8-006).
//!
//! Used by the controls overlay's harness and mux-state sub-editors
//! and intended to host any future "pick zero or more from a fixed
//! set" surface. Pure state machine: callers own the items and the
//! state, dispatch crossterm key events through [`MultiSelectState::handle_key`],
//! and react to the returned [`MultiSelectOutcome`].
//!
//! The widget is generic over an item label type so callers can use
//! `&'static str`, `String`, an enum, or anything that implements
//! [`MultiSelectItem`]. The state stores the selection by index,
//! keeping the type parameter to the public surface and out of the
//! state itself — that way it can sit inside the App without
//! infecting the reducer signature.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// Anything that can label itself in the multi-select list. Two
/// implementations are provided out of the box — for `&'static str`
/// and for `String`. Callers with enum-shaped item lists can
/// implement this trait directly to control display formatting.
pub trait MultiSelectItem {
    fn label(&self) -> &str;
}

impl MultiSelectItem for &'static str {
    fn label(&self) -> &str {
        self
    }
}

impl MultiSelectItem for String {
    fn label(&self) -> &str {
        self.as_str()
    }
}

/// What the host should do after passing a key event through the
/// overlay. `Continue` keeps the overlay open; `Confirm` carries
/// back the indices of every selected item; `Cancel` closes the
/// overlay without committing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiSelectOutcome {
    Continue,
    Confirm(Vec<usize>),
    Cancel,
}

/// Pure state for a multi-select list: cursor index, selected-set,
/// total item count, and the modal title. The state knows nothing
/// about how the items are formatted — callers pass the same item
/// slice in at every render and `handle_key` call so the widget can
/// stay generic at the API boundary without paying for type
/// parameters in storage.
#[derive(Debug, Clone)]
pub struct MultiSelectState {
    title: String,
    item_count: usize,
    cursor: usize,
    selected: Vec<bool>,
}

impl MultiSelectState {
    /// Build a state with a known item count. `initially_selected`
    /// is a list of indices that start in the checked state; out-of-
    /// range entries are silently ignored so callers can re-use
    /// stored selections across item-list shape changes.
    pub fn new(title: impl Into<String>, item_count: usize, initially_selected: &[usize]) -> Self {
        let mut selected = vec![false; item_count];
        for idx in initially_selected {
            if let Some(slot) = selected.get_mut(*idx) {
                *slot = true;
            }
        }
        Self {
            title: title.into(),
            item_count,
            cursor: 0,
            selected,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Sorted indices of every currently-checked item.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected
            .iter()
            .enumerate()
            .filter_map(|(idx, on)| on.then_some(idx))
            .collect()
    }

    pub fn is_selected(&self, idx: usize) -> bool {
        self.selected.get(idx).copied().unwrap_or(false)
    }

    /// Dispatch a crossterm key event.
    ///
    /// - `Up` / `k`: move cursor up (wraps).
    /// - `Down` / `j`: move cursor down (wraps).
    /// - `Space`: toggle the item under the cursor.
    /// - `Enter`: confirm; returns the selected indices.
    /// - `Esc` (or `Ctrl-C`): cancel.
    ///
    /// Empty lists confirm with an empty vector and cancel as usual;
    /// they cannot toggle.
    pub fn handle_key(&mut self, event: KeyEvent) -> MultiSelectOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return MultiSelectOutcome::Cancel;
        }
        match event.code {
            KeyCode::Enter => MultiSelectOutcome::Confirm(self.selected_indices()),
            KeyCode::Esc => MultiSelectOutcome::Cancel,
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_cursor(-1);
                MultiSelectOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_cursor(1);
                MultiSelectOutcome::Continue
            }
            KeyCode::Char(' ') => {
                self.toggle_at_cursor();
                MultiSelectOutcome::Continue
            }
            _ => MultiSelectOutcome::Continue,
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.item_count == 0 {
            self.cursor = 0;
            return;
        }
        let len = self.item_count as i32;
        let mut next = self.cursor as i32 + delta;
        // Wrap (-1 → len-1, len → 0).
        next = ((next % len) + len) % len;
        self.cursor = next as usize;
    }

    fn toggle_at_cursor(&mut self) {
        if let Some(slot) = self.selected.get_mut(self.cursor) {
            *slot = !*slot;
        }
    }
}

/// Centered modal rendering of [`MultiSelectState`] over a slice of
/// items. The items are passed at render time so the state stays
/// generic-free.
pub struct MultiSelectWidget<'a, T: MultiSelectItem> {
    state: &'a MultiSelectState,
    items: &'a [T],
}

impl<'a, T: MultiSelectItem> MultiSelectWidget<'a, T> {
    pub fn new(state: &'a MultiSelectState, items: &'a [T]) -> Self {
        Self { state, items }
    }
}

impl<T: MultiSelectItem> Widget for MultiSelectWidget<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area, self.items.len());
        // Repaint the modal background so dimmed body content doesn't
        // bleed through.
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(self.state.title().to_string()));
        let inner = block.inner(modal);
        block.render(modal, buf);

        // Render each item as `[x] label` or `[ ] label`; cursor row
        // gets a reverse-video highlight so it's obvious where space
        // applies.
        let inner_width = inner.width as usize;
        let visible_rows = inner.height as usize;
        let scroll = compute_scroll(self.state.cursor(), visible_rows, self.items.len());
        for (row_idx, item_idx) in
            (scroll..(scroll + visible_rows).min(self.items.len())).enumerate()
        {
            let checked = if self.state.is_selected(item_idx) {
                "[x]"
            } else {
                "[ ]"
            };
            let mut text = format!("{checked} {}", self.items[item_idx].label());
            if text.chars().count() > inner_width {
                text = text.chars().take(inner_width).collect();
            }
            let para = Paragraph::new(Line::from(text));
            let row_area = Rect {
                x: inner.x,
                y: inner.y + row_idx as u16,
                width: inner.width,
                height: 1,
            };
            para.render(row_area, buf);
            if item_idx == self.state.cursor() {
                for x in row_area.left()..row_area.right() {
                    if let Some(cell) = buf.cell_mut((x, row_area.y)) {
                        cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
                    }
                }
            }
        }
    }
}

/// Status-bar legend rendered while the overlay is open. Mirrors
/// the locked key map so the operator always sees the active
/// bindings.
pub const STATUS_LEGEND: &str = "Space toggle · Enter confirm · Esc cancel";

/// Centered modal sized to the item list, capped at the available
/// area. Width follows the same 60-column cap as the text-input
/// modal so the two primitives feel like one family. Height grows
/// to fit the items plus a 2-row border (top + bottom).
pub fn centered_modal_rect(area: Rect, item_count: usize) -> Rect {
    let width = std::cmp::min(60, area.width.saturating_sub(4));
    let width = width.max(20);
    let max_height = area.height.saturating_sub(4);
    let desired_height = (item_count as u16).saturating_add(2);
    let height = desired_height.clamp(3, max_height.max(3));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn compute_scroll(cursor: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if cursor >= visible_rows {
        cursor + 1 - visible_rows
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn cursor_starts_at_zero_and_wraps() {
        let mut state = MultiSelectState::new("harness", 3, &[]);
        assert_eq!(state.cursor(), 0);
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Down));
        assert_eq!(state.cursor(), 2);
        state.handle_key(key(KeyCode::Down));
        assert_eq!(state.cursor(), 0, "down wraps");
        state.handle_key(key(KeyCode::Up));
        assert_eq!(state.cursor(), 2, "up wraps");
    }

    #[test]
    fn vim_keys_move_cursor() {
        let mut state = MultiSelectState::new("harness", 3, &[]);
        state.handle_key(key(KeyCode::Char('j')));
        assert_eq!(state.cursor(), 1);
        state.handle_key(key(KeyCode::Char('k')));
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn space_toggles_item_under_cursor() {
        let mut state = MultiSelectState::new("harness", 3, &[]);
        state.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(state.selected_indices(), vec![0]);
        state.handle_key(key(KeyCode::Char(' ')));
        assert!(state.selected_indices().is_empty());
    }

    #[test]
    fn enter_confirms_with_current_selection() {
        let mut state = MultiSelectState::new("harness", 3, &[1]);
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![1]));
    }

    #[test]
    fn esc_cancels_without_confirming() {
        let mut state = MultiSelectState::new("harness", 3, &[1]);
        let outcome = state.handle_key(key(KeyCode::Esc));
        assert_eq!(outcome, MultiSelectOutcome::Cancel);
        assert_eq!(state.selected_indices(), vec![1]);
    }

    #[test]
    fn ctrl_c_cancels() {
        let mut state = MultiSelectState::new("harness", 3, &[]);
        let outcome = state.handle_key(ctrl(KeyCode::Char('c')));
        assert_eq!(outcome, MultiSelectOutcome::Cancel);
    }

    #[test]
    fn initial_selection_round_trips() {
        let state = MultiSelectState::new("harness", 4, &[0, 2]);
        assert!(state.is_selected(0));
        assert!(!state.is_selected(1));
        assert!(state.is_selected(2));
        assert!(!state.is_selected(3));
        assert_eq!(state.selected_indices(), vec![0, 2]);
    }

    #[test]
    fn out_of_range_initial_indices_are_ignored() {
        let state = MultiSelectState::new("harness", 2, &[1, 5, 99]);
        assert_eq!(state.selected_indices(), vec![1]);
    }

    #[test]
    fn empty_list_handles_keys_without_panicking() {
        let mut state = MultiSelectState::new("harness", 0, &[]);
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Up));
        state.handle_key(key(KeyCode::Char(' ')));
        assert_eq!(state.cursor(), 0);
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![]));
    }

    #[test]
    fn unrecognized_keys_are_ignored() {
        let mut state = MultiSelectState::new("harness", 3, &[]);
        let outcome = state.handle_key(key(KeyCode::Tab));
        assert_eq!(outcome, MultiSelectOutcome::Continue);
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn compute_scroll_keeps_cursor_in_view() {
        // 10 items, viewport 3, cursor at 5 → scroll = 3 so cursor
        // lands at row 2 (last visible).
        assert_eq!(compute_scroll(5, 3, 10), 3);
        // Cursor before viewport keeps scroll at zero.
        assert_eq!(compute_scroll(1, 3, 10), 0);
        // Cursor exactly at the bottom edge of the first viewport
        // (idx = viewport-1) does not yet scroll.
        assert_eq!(compute_scroll(2, 3, 10), 0);
        // Empty / small lists don't scroll.
        assert_eq!(compute_scroll(0, 3, 0), 0);
        assert_eq!(compute_scroll(0, 5, 3), 0);
    }

    #[test]
    fn item_label_trait_works_for_static_str_and_string() {
        let static_items: &[&'static str] = &["one", "two"];
        assert_eq!(static_items[0].label(), "one");
        let owned: Vec<String> = vec!["a".to_string(), "b".to_string()];
        assert_eq!(owned[1].label(), "b");
    }
}
