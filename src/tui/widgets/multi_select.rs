//! Multi-select list overlay primitive (ADR 0031).
//!
//! The checked set lives in [`ratatui_cheese::multi_select`]'s state;
//! this module preserves the in-tree [`MultiSelectOutcome`] +
//! [`MultiSelectState::handle_key`] contract that
//! `widgets/controls.rs`'s sub-editor dispatch expects. The cursor
//! wraps like every short choice list and each row is drawn through
//! [`tuikit::selection`], so the checklist shows its cursor the way
//! every other list does. The list draws inside the framework's compact
//! modal frame and takes the `&Theme` in [`MultiSelectWidget::new`].

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui_cheese::multi_select::MultiSelectState as CheeseMultiSelectState;
use tuikit::frame::{ModalBody, ModalFrame, ModalSize};
use tuikit::glyphs::marks;
use tuikit::scroll::ScrollWindow;
use tuikit::selection::{self, Cursor, Ends};

use crate::tui::Theme;

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
/// [`ratatui_cheese::multi_select::MultiSelectState`], which keeps the
/// cursor and the checked set, so the in-tree callers keep their
/// handle_key/outcome contract.
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
    /// - `Up` / `k` / `Ctrl-P`: move cursor up, wrapping from the first
    ///   item to the last.
    /// - `Down` / `j` / `Ctrl-N`: move cursor down, wrapping from the
    ///   last to the first.
    /// - `Space`: toggle the item under the cursor.
    /// - `Enter`: confirm; returns the selected indices.
    /// - `Esc` (or `Ctrl-C`): cancel.
    ///
    /// Empty lists confirm with an empty vector and cancel as usual;
    /// moving and toggling do nothing on an empty list.
    pub fn handle_key(&mut self, event: KeyEvent) -> MultiSelectOutcome {
        use tuikit::keys::{CHECKLIST_KEYS, ChecklistKey, resolve};
        match resolve(CHECKLIST_KEYS, event) {
            Some(ChecklistKey::Abort | ChecklistKey::Cancel) => MultiSelectOutcome::Cancel,
            Some(ChecklistKey::Confirm) => MultiSelectOutcome::Confirm(self.selected_indices()),
            Some(ChecklistKey::Up) => {
                self.step(-1);
                MultiSelectOutcome::Continue
            }
            Some(ChecklistKey::Down) => {
                self.step(1);
                MultiSelectOutcome::Continue
            }
            Some(ChecklistKey::Toggle) => {
                self.inner.toggle_current(None);
                MultiSelectOutcome::Continue
            }
            // Paging, the ends, and select all or none are the
            // framework checklist's; this list ignores them.
            _ => MultiSelectOutcome::Continue,
        }
    }

    /// A checklist is a short choice list: the cursor wraps.
    fn step(&mut self, delta: i32) {
        if self.item_count > 0 {
            let at = Ends::Wrap.step(self.inner.cursor(), self.item_count, delta);
            self.inner.set_cursor(at);
        }
    }
}

/// How large a checklist is: a compact frame as tall as its items.
pub(crate) const CHECKLIST_SIZE: ModalSize = ModalSize::COMPACT;

/// Centered modal rendering of [`MultiSelectState`] over a slice of
/// items. The items are passed at render time so the state stays
/// generic-free.
pub struct MultiSelectWidget<'a, T: AsRef<str>> {
    state: &'a MultiSelectState,
    items: &'a [T],
    theme: &'a Theme,
}

impl<'a, T: AsRef<str>> MultiSelectWidget<'a, T> {
    pub fn new(state: &'a MultiSelectState, items: &'a [T], theme: &'a Theme) -> Self {
        Self {
            state,
            items,
            theme,
        }
    }
}

impl<T: AsRef<str>> Widget for MultiSelectWidget<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let body = MultiSelectBody {
            state: self.state,
            items: self.items,
            theme: self.theme,
        };
        ModalFrame::new(self.theme, CHECKLIST_SIZE)
            .title(self.state.title())
            .render(area, buf, body);
    }
}

/// The items drawn as the frame's body, one row per item behind the
/// cursor gutter, scrolled to keep the cursor in view.
struct MultiSelectBody<'a, T: AsRef<str>> {
    state: &'a MultiSelectState,
    items: &'a [T],
    theme: &'a Theme,
}

impl<T: AsRef<str>> ModalBody for MultiSelectBody<'_, T> {
    type Output = ();

    fn measure(&mut self, _width: u16) -> u16 {
        u16::try_from(self.items.len()).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        let cursor = self.state.cursor();
        let window = ScrollWindow::new(usize::from(area.height), self.items.len())
            .following(cursor..cursor + 1);
        // The gutter, the check mark, and a space come before the label.
        let label_width = usize::from(area.width).saturating_sub(selection::GUTTER_WIDTH + 2);
        let checked = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let lines: Vec<Line<'_>> = window
            .visible()
            .map(|index| {
                let style = if self.state.is_selected(index) {
                    checked
                } else {
                    Style::default()
                };
                let mark = if self.state.is_selected(index) {
                    marks::OK
                } else {
                    marks::BULLET
                };
                let label = tuikit::text::truncate_end(self.items[index].as_ref(), label_width);
                selection::row(
                    [
                        Span::styled(mark, style),
                        Span::raw(" "),
                        Span::styled(label, style),
                    ],
                    Cursor::of(index == cursor, true),
                    self.theme,
                )
            })
            .collect();
        Paragraph::new(lines).render(area, buf);
    }
}

#[cfg(test)]
#[path = "multi_select_tests.rs"]
mod tests;
