//! Multi-select list overlay primitive (ADR 0031, F8-006).
//!
//! Thin shim over [`ratatui_cheese::multi_select`] (H-WIDG-002). The
//! upstream crate owns the cursor + selection state machine and the
//! per-row rendering; this module preserves the in-tree
//! [`MultiSelectOutcome`] + [`MultiSelectState::handle_key`] contract
//! that `widgets/controls.rs`'s sub-editor dispatch expects, so the
//! swap stays a single-file change at the API boundary. The bordered
//! centered modal + buffer-clear remain in this module because
//! they're UI integration code, not widget rendering.
//!
//! Theme bridge: [`cheese_styles_from_theme`] maps the `[tui.theme]`
//! keys (ADR 0032) onto the upstream
//! [`ratatui_cheese::multi_select::MultiSelectStyles`] surface so the
//! sub-editor honors operator overrides. Callers pass a `&Theme` to
//! [`MultiSelectWidget::theme`] at construction; with no theme the
//! widget falls back to the upstream dark palette.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::macros::line;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui_cheese::multi_select::{
    MultiSelect as CheeseMultiSelect, MultiSelectOption,
    MultiSelectState as CheeseMultiSelectState, MultiSelectStyles as CheeseMultiSelectStyles,
};
use tui_popup::KnownSize;

use crate::tui::Theme;

/// Bridge the project's [`Theme`] (ADR 0032) onto upstream
/// [`CheeseMultiSelectStyles`]. The bordered modal title is rendered
/// by this module's [`MultiSelectWidget`], not by the upstream
/// widget, so the `title` and `description` slots stay defaulted —
/// they are unreachable in our composition. The remaining slots map:
///
/// - `cursor` → `panel_focus_accent` so the `>` indicator pops.
/// - `checked` → `panel_focus_accent` + BOLD so checked items read
///   as the operator's selection.
/// - `unchecked` → unset so labels render as regular text.
/// - `disabled` → `placeholder` modifier so disabled rows dim
///   uniformly (no disabled options today; future-proofs the swap).
/// - `validation_error` / `validation_success` → `error` / `success`
///   colors so future limit/required validators inherit the project
///   palette.
fn cheese_styles_from_theme(theme: &Theme) -> CheeseMultiSelectStyles {
    CheeseMultiSelectStyles {
        title: Style::default(),
        description: Style::default(),
        cursor: Style::default().fg(theme.panel_focus_accent),
        checked: Style::default()
            .fg(theme.panel_focus_accent)
            .add_modifier(Modifier::BOLD),
        unchecked: Style::default(),
        disabled: Style::default().add_modifier(theme.placeholder),
        validation_error: Style::default().fg(theme.error),
        validation_success: Style::default().fg(theme.success),
    }
}

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

/// Pure state for a multi-select list. Wraps
/// [`ratatui_cheese::multi_select::MultiSelectState`] so the in-tree
/// callers keep their handle_key/outcome contract while the cursor
/// movement, wrapping, and toggle semantics come from upstream.
#[derive(Debug)]
pub struct MultiSelectState {
    title: String,
    item_count: usize,
    inner: CheeseMultiSelectState,
}

impl Clone for MultiSelectState {
    fn clone(&self) -> Self {
        // CheeseMultiSelectState holds a `Box<dyn Fn>` validator that
        // blocks #[derive(Clone)]. The in-tree shim does not use
        // validators, so cloning rebuilds a fresh state with the same
        // cursor + selection footprint. Focus state is always `true`
        // for the open sub-editor surface so it's set unconditionally
        // alongside the inner reconstruction.
        let mut inner = CheeseMultiSelectState::new(self.item_count);
        inner.set_cursor(self.inner.cursor());
        for idx in self.inner.selected_indices() {
            inner.set_selected(idx, true);
        }
        inner.set_focused(self.inner.focused());
        Self {
            title: self.title.clone(),
            item_count: self.item_count,
            inner,
        }
    }
}

impl MultiSelectState {
    /// Build a state with a known item count. `initially_selected`
    /// is a list of indices that start in the checked state; out-of-
    /// range entries are silently ignored so callers can re-use
    /// stored selections across item-list shape changes.
    pub fn new(title: impl Into<String>, item_count: usize, initially_selected: &[usize]) -> Self {
        let mut inner = CheeseMultiSelectState::new(item_count);
        for idx in initially_selected {
            if *idx < item_count {
                inner.set_selected(*idx, true);
            }
        }
        inner.set_focused(true);
        Self {
            title: title.into(),
            item_count,
            inner,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cursor(&self) -> usize {
        self.inner.cursor()
    }

    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Sorted indices of every currently-checked item.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.inner.selected_indices()
    }

    pub fn is_selected(&self, idx: usize) -> bool {
        self.inner.is_selected(idx)
    }

    /// Dispatch a crossterm key event.
    ///
    /// - `Up` / `k`: move cursor up (wraps via upstream `prev`).
    /// - `Down` / `j`: move cursor down (wraps via upstream `next`).
    /// - `Space`: toggle the item under the cursor.
    /// - `Enter`: confirm; returns the selected indices.
    /// - `Esc` (or `Ctrl-C`): cancel.
    ///
    /// Empty lists confirm with an empty vector and cancel as usual;
    /// upstream `next` / `prev` / `toggle_current` are all no-ops on
    /// an empty option set.
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
                self.inner.prev();
                MultiSelectOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.inner.next();
                MultiSelectOutcome::Continue
            }
            KeyCode::Char(' ') => {
                self.inner.toggle_current(None);
                MultiSelectOutcome::Continue
            }
            _ => MultiSelectOutcome::Continue,
        }
    }
}

/// Centered modal rendering of [`MultiSelectState`] over a slice of
/// items. The items are passed at render time so the state stays
/// generic-free.
pub struct MultiSelectWidget<'a, T: MultiSelectItem> {
    state: &'a MultiSelectState,
    items: &'a [T],
    theme: Option<&'a Theme>,
}

impl<'a, T: MultiSelectItem> MultiSelectWidget<'a, T> {
    pub fn new(state: &'a MultiSelectState, items: &'a [T]) -> Self {
        Self {
            state,
            items,
            theme: None,
        }
    }

    /// Honor operator `[tui.theme]` overrides (ADR 0032). Without
    /// this the widget renders with the upstream dark palette.
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = Some(theme);
        self
    }
}

impl<T: MultiSelectItem> Widget for MultiSelectWidget<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`. Theme glue
        // applies when set; falls back to upstream defaults when
        // unset so call sites that don't pass a theme still render.
        let modal = centered_modal_rect(area, self.items.len());
        let title = line![self.state.title().to_string()];
        let body = MultiSelectBody {
            state: self.state,
            items: self.items,
            theme: self.theme,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        if let Some(theme) = self.theme {
            let popup = crate::tui::widgets::popup_frame::themed_popup(body, title, theme);
            Widget::render(popup, area, buf);
        } else {
            let popup = tui_popup::Popup::new(body).title(title);
            Widget::render(popup, area, buf);
        }
    }
}

/// Body wrapper that bridges the ratatui-cheese `MultiSelect`
/// upstream widget into a popup body. Sizing follows the cap dims
/// the in-tree `centered_modal_rect` computed.
struct MultiSelectBody<'a, T: MultiSelectItem> {
    state: &'a MultiSelectState,
    items: &'a [T],
    theme: Option<&'a Theme>,
    inner_width: usize,
    inner_height: usize,
}

impl<T: MultiSelectItem> KnownSize for MultiSelectBody<'_, T> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl<T: MultiSelectItem> Widget for MultiSelectBody<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Bridge our `[T: MultiSelectItem]` slice into upstream
        // `MultiSelectOption`s. The Vec lives for the duration of
        // this call so the borrow into the widget is valid.
        let options: Vec<MultiSelectOption<'_>> = self
            .items
            .iter()
            .map(|item| MultiSelectOption::new(item.label()))
            .collect();

        // The upstream widget renders via &mut state. We're behind a
        // shared borrow, so build a mirror that reflects cursor +
        // selections and pass that.
        let mut mirror = CheeseMultiSelectState::new(self.state.item_count);
        mirror.set_cursor(self.state.cursor());
        for idx in self.state.selected_indices() {
            mirror.set_selected(idx, true);
        }
        mirror.set_focused(true);

        // Title is rendered by the popup frame; pass an empty title
        // to the upstream widget so it doesn't re-stamp a second
        // one inside the inner area.
        let mut widget = CheeseMultiSelect::new("", &options);
        if let Some(theme) = self.theme {
            widget = widget.styles(cheese_styles_from_theme(theme));
        }
        StatefulWidget::render(&widget, area, buf, &mut mirror);
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
    fn item_label_trait_works_for_static_str_and_string() {
        let static_items: &[&'static str] = &["one", "two"];
        assert_eq!(static_items[0].label(), "one");
        let owned: Vec<String> = vec!["a".to_string(), "b".to_string()];
        assert_eq!(owned[1].label(), "b");
    }
}
