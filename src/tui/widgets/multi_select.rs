//! Multi-select list overlay primitive (ADR 0031).
//!
//! Thin shim over [`ratatui_cheese::multi_select`]. The
//! upstream crate owns the cursor + selection state machine and the
//! per-row rendering; this module preserves the in-tree
//! [`MultiSelectOutcome`] + [`MultiSelectState::handle_key`] contract
//! that `widgets/controls.rs`'s sub-editor dispatch expects, so the
//! swap stays a single-file change at the API boundary. The list draws
//! inside the framework's compact modal frame.
//!
//! Theme bridge: `cheese_styles_from_theme` maps the `[tui.theme]`
//! keys (ADR 0032) onto the upstream
//! [`ratatui_cheese::multi_select::MultiSelectStyles`] surface so the
//! sub-editor honors operator overrides; [`MultiSelectWidget::new`]
//! takes the `&Theme`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui_cheese::multi_select::{
    MultiSelect as CheeseMultiSelect, MultiSelectOption,
    MultiSelectState as CheeseMultiSelectState, MultiSelectStyles as CheeseMultiSelectStyles,
};
use tuikit::frame::{ModalBody, ModalFrame, ModalSize};
use tuikit::scroll::ScrollWindow;

use crate::tui::Theme;

/// Bridge the project's [`Theme`] (ADR 0032) onto upstream
/// [`CheeseMultiSelectStyles`]. The bordered modal title is rendered
/// by this module's [`MultiSelectWidget`], not by the upstream
/// widget, so the `title` and `description` slots stay defaulted —
/// they are unreachable in our composition. The remaining slots map:
///
/// - `cursor` → `accent` so the `>` indicator pops.
/// - `checked` → `accent` + BOLD so checked items read
///   as the operator's selection.
/// - `unchecked` → unset so labels render as regular text.
/// - `disabled` → the `muted` modifier so disabled rows dim
///   uniformly (no disabled options today; future-proofs the swap).
/// - `validation_error` / `validation_success` → `error` / `success`
///   colors so future limit/required validators inherit the project
///   palette.
fn cheese_styles_from_theme(theme: &Theme) -> CheeseMultiSelectStyles {
    CheeseMultiSelectStyles {
        title: Style::default(),
        description: Style::default(),
        cursor: Style::default().fg(theme.accent),
        checked: Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
        unchecked: Style::default(),
        disabled: Style::default().add_modifier(theme.muted),
        validation_error: Style::default().fg(theme.error),
        validation_success: Style::default().fg(theme.success),
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
        use crate::tui::keybindings::overlay::{MULTI_SELECT_KEYS, MultiSelectKey, resolve};
        match resolve(MULTI_SELECT_KEYS, event) {
            Some(MultiSelectKey::Abort | MultiSelectKey::Cancel) => MultiSelectOutcome::Cancel,
            Some(MultiSelectKey::Confirm) => MultiSelectOutcome::Confirm(self.selected_indices()),
            Some(MultiSelectKey::Up) => {
                self.inner.prev();
                MultiSelectOutcome::Continue
            }
            Some(MultiSelectKey::Down) => {
                self.inner.next();
                MultiSelectOutcome::Continue
            }
            Some(MultiSelectKey::Toggle) => {
                self.inner.toggle_current(None);
                MultiSelectOutcome::Continue
            }
            None => MultiSelectOutcome::Continue,
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

/// The upstream `MultiSelect` widget drawn as the frame's body, one row
/// per item, scrolled to keep the cursor in view.
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
        let scroll = window.offset();

        // Bridge our `[T: AsRef<str>]` slice into upstream
        // `MultiSelectOption`s. The Vec lives for the duration of
        // this call so the borrow into the widget is valid.
        let options: Vec<MultiSelectOption<'_>> = self
            .items
            .get(window.visible())
            .unwrap_or(&[])
            .iter()
            .map(|item| MultiSelectOption::new(item.as_ref()))
            .collect();

        // The upstream widget renders via &mut state. We're behind a
        // shared borrow, so build a mirror that reflects cursor +
        // selections and pass that.
        let mut mirror = CheeseMultiSelectState::new(options.len());
        mirror.set_cursor(self.state.cursor().saturating_sub(scroll));
        for idx in self.state.selected_indices() {
            if let Some(visible_idx) = idx.checked_sub(scroll)
                && visible_idx < options.len()
            {
                mirror.set_selected(visible_idx, true);
            }
        }
        mirror.set_focused(true);

        // Title is rendered by the popup frame; pass an empty title
        // to the upstream widget so it doesn't re-stamp a second
        // one inside the inner area.
        let widget = CheeseMultiSelect::new("", &options)
            .checked_indicator(tuikit::glyphs::marks::OK)
            .unchecked_indicator(tuikit::glyphs::marks::BULLET)
            .styles(cheese_styles_from_theme(self.theme));
        StatefulWidget::render(&widget, area, buf, &mut mirror);
    }
}

#[cfg(test)]
#[path = "multi_select_tests.rs"]
mod tests;
