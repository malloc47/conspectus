//! Full-value inspection modal.
//!
//! Surfaces the untruncated text of a long detail-pane value (`cwd`,
//! `command`, `url`, `last_message_preview`, …) in a centered modal
//! so the operator can read or copy the full content without forcing
//! it into the main right-panel layout. Reuses the same centered
//! modal frame as the help overlay (`centered_modal_rect`) per the
//! existing modal conventions and avoids new dependencies.
//!
//! Input handling:
//! - `Esc` / `q` / `o` close the modal.
//! - `j` / `k` / `Down` / `Up` scroll the value body by one row.
//! - `PageDown` / `PageUp` / `Space` jump by the rows the body showed.
//! - `g` / `G` jump to the top / bottom.
//!
//! The value is wrapped here rather than by the paragraph, so the draw
//! knows how many rows it has and can keep the offset inside them.
//! - `y` copies the text, or the payload the opener supplied (a rebuild
//!   plan copies its commands as a script rather than its prose); the
//!   runtime does the clipboard write and the toast.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;
use tuikit::scroll::ScrollWindow;

use crate::tui::Theme;
use crate::tui::widgets::help::centered_modal_rect;

/// Pure state for the full-value modal.
#[derive(Debug, Clone)]
pub struct ValueModalState {
    /// Static label rendered in the title bar (`cwd`, `command`, …).
    pub label: String,
    /// The full untruncated value being inspected.
    pub value: String,
    /// Vertical scroll offset in rendered rows.
    pub scroll: u16,
    /// Rows of text the body showed at the last draw; a page key moves
    /// by this many.
    pub page_rows: u16,
    /// What `y` copies when that isn't the value itself.
    pub copy: Option<ValueCopy>,
    /// Draw lines that show a command (`$ …`) in the accent style.
    pub highlight_commands: bool,
}

/// Text a value modal copies to the clipboard, and how to describe it
/// in the toast and the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCopy {
    pub text: String,
    /// Noun phrase for the footer and toast: `3 commands`.
    pub what: String,
}

impl ValueModalState {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            scroll: 0,
            page_rows: 0,
            copy: None,
            highlight_commands: false,
        }
    }

    /// A rebuild plan: the plan text with its commands highlighted, and
    /// `y` copying the commands as a script.
    pub fn plan(
        label: impl Into<String>,
        text: impl Into<String>,
        script: impl Into<String>,
        commands: usize,
    ) -> Self {
        let mut state = Self::new(label, text);
        state.highlight_commands = true;
        state.copy = Some(ValueCopy {
            text: script.into(),
            what: format!("{commands} command{}", if commands == 1 { "" } else { "s" }),
        });
        state
    }

    /// What `y` puts on the clipboard, and how to name it.
    pub fn copy_payload(&self) -> (&str, String) {
        match &self.copy {
            Some(copy) => (copy.text.as_str(), copy.what.clone()),
            None => (self.value.as_str(), "text".to_string()),
        }
    }

    /// Dispatch a crossterm key event. Returns
    /// [`ValueModalOutcome::Close`] when the modal should close,
    /// otherwise [`ValueModalOutcome::Continue`].
    pub fn handle_key(&mut self, event: KeyEvent) -> ValueModalOutcome {
        use crate::tui::keybindings::overlay::{PagerKey, VALUE_MODAL_KEYS, resolve};
        match resolve(VALUE_MODAL_KEYS, event) {
            Some(PagerKey::Abort | PagerKey::Close) => ValueModalOutcome::Close,
            Some(PagerKey::Copy) => ValueModalOutcome::Continue,
            Some(PagerKey::Down) => {
                self.scroll_by(1);
                ValueModalOutcome::Continue
            }
            Some(PagerKey::Up) => {
                self.scroll_by(-1);
                ValueModalOutcome::Continue
            }
            Some(PagerKey::PageDown) => {
                self.scroll_by(i32::from(self.page_rows.max(1)));
                ValueModalOutcome::Continue
            }
            Some(PagerKey::PageUp) => {
                self.scroll_by(-i32::from(self.page_rows.max(1)));
                ValueModalOutcome::Continue
            }
            Some(PagerKey::Top) => {
                self.scroll = 0;
                ValueModalOutcome::Continue
            }
            Some(PagerKey::Bottom) => {
                self.scroll = u16::MAX;
                ValueModalOutcome::Continue
            }
            None => ValueModalOutcome::Continue,
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        let current = i32::from(self.scroll);
        let next = current.saturating_add(delta).max(0);
        self.scroll = u16::try_from(next.min(i32::from(u16::MAX))).unwrap_or(u16::MAX);
    }

    /// Take in the window the last draw showed: the offset comes back
    /// inside the text (so `G`, or scrolling past the end, is undone by
    /// the next key up) and page keys move by the rows on screen.
    pub fn fit(&mut self, window: ScrollWindow) {
        let last = u16::try_from(window.last_offset()).unwrap_or(u16::MAX);
        self.scroll = self.scroll.min(last);
        self.page_rows = u16::try_from(window.viewport()).unwrap_or(u16::MAX);
    }

    /// The value wrapped to `width` cells; a command line keeps the
    /// command style on every row it wraps onto.
    fn wrapped(&self, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        let command_style = Style::default().fg(theme.accent);
        self.value
            .lines()
            .flat_map(|line| {
                let style = if self.highlight_commands && line.trim_start().starts_with("$ ") {
                    command_style
                } else {
                    Style::default()
                };
                tuikit::text::wrap(line, usize::from(width))
                    .into_iter()
                    .map(move |row| Line::styled(row.trim_end().to_string(), style))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueModalOutcome {
    Continue,
    Close,
}

impl crate::tui::Overlay for ValueModalState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> crate::tui::OverlayOutcome {
        match self.handle_key(key) {
            ValueModalOutcome::Continue => crate::tui::OverlayOutcome::Consumed,
            ValueModalOutcome::Close => crate::tui::OverlayOutcome::Close,
        }
    }
}

/// Centered modal that renders [`ValueModalState`]. Uses the same
/// `centered_modal_rect` frame as the help overlay so all modal
/// surfaces line up consistently.
pub struct ValueModalWidget<'a> {
    state: &'a ValueModalState,
    theme: &'a Theme,
}

impl<'a> ValueModalWidget<'a> {
    pub fn new(state: &'a ValueModalState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl ValueModalWidget<'_> {
    /// Draw the modal and report the window its body showed, for
    /// [`ValueModalState::fit`].
    pub fn draw(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        // Framing through `tui_popup::Popup`.
        let modal = centered_modal_rect(area);
        let label_style = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let title = line![" ", span!(label_style; " {} ", self.state.label), " "];
        let mut fit = ScrollWindow::default();
        let body = ValueModalBody {
            state: self.state,
            theme: self.theme,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
            fit: &mut fit,
        };
        let popup = crate::tui::widgets::popup_frame::themed_popup(body, title, self.theme);
        popup.render(area, buf);
        fit
    }
}

impl Widget for ValueModalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.draw(area, buf);
    }
}

/// Body wrapper for `tui_popup::Popup`. Renders the wrapped value
/// text with vertical scroll; sizing follows the cap dims so the
/// popup auto-sizing reproduces the in-tree rect.
struct ValueModalBody<'a> {
    state: &'a ValueModalState,
    theme: &'a Theme,
    inner_width: usize,
    inner_height: usize,
    /// Where the body reports the window it drew.
    fit: &'a mut ScrollWindow,
}

impl KnownSize for ValueModalBody<'_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for ValueModalBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use ratatui::layout::{Constraint, Layout};
        let [text_area, footer_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
        let lines = self.state.wrapped(text_area.width, self.theme);
        // An offset past the end (`G`, or paging on) shows the last rows.
        let window = ScrollWindow::new(usize::from(text_area.height), lines.len())
            .at(usize::from(self.state.scroll));
        *self.fit = window;
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(text_area, buf);
        let hint = crate::tui::keybindings::overlay::footer(
            crate::tui::keybindings::overlay::VALUE_MODAL_KEYS,
        );
        Paragraph::new(hint)
            .style(Style::default().fg(self.theme.inert))
            .render(footer_area, buf);
    }
}

#[cfg(test)]
#[path = "value_modal_tests.rs"]
mod tests;
