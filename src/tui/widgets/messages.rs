//! `!` Messages overlay (ADR 0105): the TUI's log of operation
//! outcomes, newest first, with the selected entry's full record
//! (argv, exit status, stderr, stdout) below the list.
//!
//! Input:
//! - `j` / `k` / `Down` / `Up` select an entry; `g` / `G` jump to the
//!   newest / oldest.
//! - `J` / `K` scroll the detail pane a row; `PageDown` / `PageUp` by the
//!   rows it showed.
//! - `y` copies the selected entry (handled by the runtime, which
//!   owns the clipboard).
//! - `Esc` / `q` / `!` close.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;
use tuikit::glyphs::marks;
use tuikit::scroll::ScrollWindow;

use crate::tui::Theme;
use crate::tui::messages::{LogEntry, LogLevel, MessageLog};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagesOverlayState {
    /// Index into the newest-first entry list.
    pub selected: usize,
    pub detail_scroll: u16,
    /// Rows the detail pane showed at the last draw; a page key moves by
    /// this many.
    pub detail_rows: u16,
}

impl MessagesOverlayState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The selected entry, if the log has any.
    pub fn selected_entry<'a>(&self, log: &'a MessageLog) -> Option<&'a LogEntry> {
        log.newest_first().nth(self.selected)
    }

    fn select(&mut self, index: usize, len: usize) {
        let index = index.min(len.saturating_sub(1));
        if index != self.selected {
            self.selected = index;
            self.detail_scroll = 0;
        }
    }

    fn scroll_detail(&mut self, delta: i32) {
        let next = i32::from(self.detail_scroll).saturating_add(delta).max(0);
        self.detail_scroll = u16::try_from(next).unwrap_or(u16::MAX);
    }

    /// Take in the window the last draw's detail pane showed: its offset
    /// comes back inside the text and page keys move by the rows on
    /// screen.
    pub fn fit(&mut self, window: ScrollWindow) {
        let last = u16::try_from(window.last_offset()).unwrap_or(u16::MAX);
        self.detail_scroll = self.detail_scroll.min(last);
        self.detail_rows = u16::try_from(window.viewport()).unwrap_or(u16::MAX);
    }
}

impl crate::tui::Overlay for MessagesOverlayState {
    type Ctx<'a> = &'a MessageLog;

    fn handle(&mut self, log: &MessageLog, key: KeyEvent) -> crate::tui::OverlayOutcome {
        use crate::tui::OverlayOutcome;
        use crate::tui::keybindings::overlay::{MESSAGES_KEYS, MessagesKey, resolve};
        let len = log.len();
        match resolve(MESSAGES_KEYS, key) {
            Some(MessagesKey::Abort | MessagesKey::Close) => return OverlayOutcome::Close,
            Some(MessagesKey::Next) => self.select(self.selected + 1, len),
            Some(MessagesKey::Prev) => {
                self.select(self.selected.saturating_sub(1), len);
            }
            Some(MessagesKey::First) => self.select(0, len),
            Some(MessagesKey::Last) => self.select(len.saturating_sub(1), len),
            Some(MessagesKey::DetailDown) => self.scroll_detail(1),
            Some(MessagesKey::DetailUp) => self.scroll_detail(-1),
            Some(MessagesKey::DetailPageDown) => {
                self.scroll_detail(i32::from(self.detail_rows.max(1)));
            }
            Some(MessagesKey::DetailPageUp) => {
                self.scroll_detail(-i32::from(self.detail_rows.max(1)));
            }
            // The runtime copies before the key reaches the widget.
            Some(MessagesKey::Copy) | None => {}
        }
        OverlayOutcome::Consumed
    }
}

pub struct MessagesWidget<'a> {
    state: &'a MessagesOverlayState,
    log: &'a MessageLog,
    theme: &'a Theme,
    now: i64,
}

impl<'a> MessagesWidget<'a> {
    pub fn new(
        state: &'a MessagesOverlayState,
        log: &'a MessageLog,
        theme: &'a Theme,
        now: i64,
    ) -> Self {
        Self {
            state,
            log,
            theme,
            now,
        }
    }
}

/// Style for an entry's level glyph and summary.
pub fn level_style(level: LogLevel, theme: &Theme) -> Style {
    match level {
        LogLevel::Info => Style::default().fg(theme.info),
        LogLevel::Warning => Style::default().fg(theme.warning),
        LogLevel::Error => Style::default().fg(theme.error),
    }
}

impl MessagesWidget<'_> {
    /// Draw the overlay and report the window its detail pane showed, for
    /// [`MessagesOverlayState::fit`].
    pub fn draw(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let width = area.width.saturating_sub(4).clamp(40, 110);
        let height = area.height.saturating_sub(2).max(10);
        let modal = crate::tui::widgets::popup_frame::centered_rect(area, width, height);
        let label_style = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let title = line![
            " ",
            span!(label_style; " Messages · {} ", self.log.len()),
            " "
        ];
        let mut fit = ScrollWindow::default();
        let body = MessagesBody {
            widget: &self,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
            fit: &mut fit,
        };
        crate::tui::widgets::popup_frame::themed_popup(body, title, self.theme).render(area, buf);
        fit
    }
}

impl Widget for MessagesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.draw(area, buf);
    }
}

struct MessagesBody<'a, 'w> {
    widget: &'a MessagesWidget<'w>,
    inner_width: usize,
    inner_height: usize,
    /// Where the body reports the window its detail pane drew.
    fit: &'a mut ScrollWindow,
}

impl KnownSize for MessagesBody<'_, '_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for MessagesBody<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let MessagesWidget {
            state,
            log,
            theme,
            now,
        } = *self.widget;
        let footer = line![span!(
            Style::default().add_modifier(theme.muted);
            "{}",
            crate::tui::keybindings::overlay::footer(crate::tui::keybindings::overlay::MESSAGES_KEYS)
        )];
        if log.is_empty() {
            Paragraph::new(vec![
                Line::raw("No messages yet. Launches, attaches, renames and"),
                Line::raw("failures from this session are listed here."),
                Line::raw(""),
                footer,
            ])
            .render(area, buf);
            return;
        }

        // List gets up to 40% of the height, the detail the rest.
        let list_height = (log.len() as u16).min((area.height * 2 / 5).max(3));
        let list_area = Rect {
            height: list_height,
            ..area
        };
        let rule_area = Rect {
            y: area.y + list_height,
            height: 1,
            ..area
        };
        let detail_area = Rect {
            y: rule_area.y + 1,
            height: area.height.saturating_sub(list_height + 2),
            ..area
        };
        let footer_area = Rect {
            y: area.y + area.height.saturating_sub(1),
            height: 1,
            ..area
        };

        // Keep the selection inside the list window.
        let list = ScrollWindow::new(usize::from(list_height), log.len())
            .following(state.selected..state.selected + 1);
        let lines: Vec<Line> = log
            .newest_first()
            .enumerate()
            .skip(list.offset())
            .take(list.viewport())
            .map(|(idx, entry)| list_line(entry, idx == state.selected, theme, now))
            .collect();
        Paragraph::new(lines).render(list_area, buf);

        Paragraph::new(Line::from(span!(
            Style::default().add_modifier(theme.muted);
            "{}", marks::RULE.repeat(usize::from(area.width))
        )))
        .render(rule_area, buf);

        if let Some(entry) = state.selected_entry(log) {
            // Wrapped here so the offset can be kept inside the text: paging
            // past the end shows the last rows.
            let lines: Vec<Line> = entry
                .full_text()
                .lines()
                .flat_map(|line| tuikit::text::wrap(line, usize::from(detail_area.width)))
                .map(|row| Line::from(row.trim_end().to_string()))
                .collect();
            let window = ScrollWindow::new(usize::from(detail_area.height), lines.len())
                .at(usize::from(state.detail_scroll));
            *self.fit = window;
            Paragraph::new(lines)
                .scroll(window.paragraph_scroll())
                .render(detail_area, buf);
        }
        Paragraph::new(footer).render(footer_area, buf);
    }
}

fn list_line(entry: &LogEntry, selected: bool, theme: &Theme, now: i64) -> Line<'static> {
    let age = crate::tui::rows::format_recency(Some(now), Some(entry.at_epoch))
        .unwrap_or_else(|| marks::NO_VALUE.to_string());
    let mut spans = vec![
        Span::styled(
            format!("{} ", entry.level.glyph()),
            level_style(entry.level, theme),
        ),
        Span::styled(format!("{age:>4} ago  "), Style::default().fg(theme.inert)),
        Span::raw(entry.summary_with_repeats()),
    ];
    if selected {
        for span in &mut spans {
            span.style = span.style.add_modifier(theme.selection_active);
        }
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
