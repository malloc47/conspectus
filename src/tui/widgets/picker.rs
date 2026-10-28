//! A pick-one list with type-to-filter, for choosing among known
//! candidates instead of typing a name: where a pin should point next
//! is the first user.
//!
//! The query line filters the list as it is typed (every word must
//! appear in the label or detail, case-insensitively). Arrow keys and
//! `Ctrl-N`/`Ctrl-P` move the cursor, `Enter` picks, `Esc` cancels; the
//! keys come from the search overlay's table so the two lists behave
//! alike.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::frame::{ModalBody, ModalFrame, ModalSize};
use tuikit::scroll::ScrollWindow;
use tuikit::selection::{self, Cursor, Ends};

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::input::TextInputState;

/// One candidate: what is shown, and what choosing it commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    pub label: String,
    /// Context shown dim after the label: where it lives, its state.
    pub detail: String,
    /// What the commit carries: a node id or a history record id.
    pub value: String,
}

/// What choosing an item does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerPurpose {
    /// Point `pin` at the chosen entity.
    Retarget { pin: String },
    /// Show the chosen remembered search in the Search view; each item's
    /// value is its position, oldest first.
    SearchHistory,
}

#[derive(Debug, Clone)]
pub struct PickerState {
    title: String,
    purpose: PickerPurpose,
    items: Vec<PickerItem>,
    query: TextInputState,
    /// Indices into `items` that match the query, in list order.
    visible: Vec<usize>,
    cursor: usize,
}

impl PickerState {
    pub fn new(title: impl Into<String>, purpose: PickerPurpose, items: Vec<PickerItem>) -> Self {
        let visible = (0..items.len()).collect();
        Self {
            title: title.into(),
            purpose,
            items,
            query: TextInputState::new(" filter ", String::new()),
            visible,
            cursor: 0,
        }
    }

    /// Start with the cursor on the item whose value is `value`.
    #[must_use]
    pub fn at_value(mut self, value: &str) -> Self {
        if let Some(position) = self.items.iter().position(|item| item.value == value) {
            self.cursor = position;
        }
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn query(&self) -> &str {
        self.query.value()
    }

    pub fn total(&self) -> usize {
        self.items.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The items matching the query.
    pub fn visible_items(&self) -> Vec<&PickerItem> {
        self.visible.iter().map(|&i| &self.items[i]).collect()
    }

    /// The item the cursor is on.
    pub fn current(&self) -> Option<&PickerItem> {
        self.visible.get(self.cursor).map(|&i| &self.items[i])
    }

    fn refilter(&mut self) {
        let words: Vec<String> = self
            .query
            .value()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        self.visible = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                let haystack = format!("{} {}", item.label, item.detail).to_lowercase();
                words.iter().all(|word| haystack.contains(word))
            })
            .map(|(index, _)| index)
            .collect();
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
    }

    fn commit(&self, item: &PickerItem) -> Msg {
        match &self.purpose {
            PickerPurpose::Retarget { pin } => Msg::CommitRetarget {
                pin: pin.clone(),
                target: item.value.clone(),
            },
            PickerPurpose::SearchHistory => {
                Msg::ShowSearchHistory(item.value.parse().unwrap_or_default())
            }
        }
    }
}

impl Overlay for PickerState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        use crate::tui::keybindings::overlay::{PICKER_KEYS, SearchKey, resolve};
        match resolve(PICKER_KEYS, key) {
            Some(SearchKey::Abort | SearchKey::Cancel) => OverlayOutcome::Close,
            Some(SearchKey::Pick) => match self.current() {
                Some(item) => OverlayOutcome::Commit(Box::new(self.commit(item))),
                None => OverlayOutcome::Close,
            },
            Some(SearchKey::Down) => {
                self.cursor = Ends::Wrap.step(self.cursor, self.visible.len(), 1);
                OverlayOutcome::Consumed
            }
            Some(SearchKey::Up) => {
                self.cursor = Ends::Wrap.step(self.cursor, self.visible.len(), -1);
                OverlayOutcome::Consumed
            }
            // The picker has no scopes; its table binds none of these.
            Some(_) | None => {
                let _ = self.query.handle_key(key);
                self.refilter();
                OverlayOutcome::Consumed
            }
        }
    }
}

pub struct PickerWidget<'a> {
    state: &'a PickerState,
    theme: &'a Theme,
}

impl<'a> PickerWidget<'a> {
    pub fn new(state: &'a PickerState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = format!(
            "{} ({}/{})",
            self.state.title,
            self.state.visible.len(),
            self.state.total()
        );
        let body = PickerBody {
            state: self.state,
            theme: self.theme,
        };
        // A long list: the frame keeps its size while the filter narrows it.
        ModalFrame::new(self.theme, ModalSize::WIDE)
            .title(title)
            .render(area, buf, body);
    }
}

/// The query row over the filtered list.
struct PickerBody<'a> {
    state: &'a PickerState,
    theme: &'a Theme,
}

impl ModalBody for PickerBody<'_> {
    type Output = ();

    /// The list fills its frame, so it is laid out in `render`.
    fn measure(&mut self, _width: u16) -> u16 {
        0
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        let query = line![
            span!(Style::default().fg(self.theme.accent); "/"),
            self.state.query().to_string(),
        ];
        Paragraph::new(query).render(Rect { height: 1, ..area }, buf);

        let list = Rect {
            y: area.y.saturating_add(1),
            height: area.height.saturating_sub(1),
            ..area
        };
        let items = self.state.visible_items();
        if items.is_empty() {
            let note = if self.state.total() == 0 {
                "(nothing else of this kind to point at)"
            } else {
                "(no matches)"
            };
            Paragraph::new(line![span!(self.theme.muted; "{note}")]).render(list, buf);
            return;
        }
        let cursor = self.state.cursor();
        let window =
            ScrollWindow::new(usize::from(list.height), items.len()).following(cursor..cursor + 1);
        let lines: Vec<Line<'static>> = items
            .iter()
            .enumerate()
            .skip(window.offset())
            .take(window.viewport())
            .map(|(index, item)| {
                let mut spans: Vec<Span<'static>> = vec![Span::raw(item.label.clone())];
                if !item.detail.is_empty() {
                    spans.push(span!(self.theme.muted; "  · {}", item.detail));
                }
                selection::row(spans, Cursor::of(index == cursor, true), self.theme)
            })
            .collect();
        Paragraph::new(lines).render(list, buf);
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
