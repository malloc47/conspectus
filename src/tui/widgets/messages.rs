//! `!` Messages overlay (ADR 0105): the TUI's log of operation
//! outcomes, newest first, with the selected entry's full record
//! (argv, exit status, stderr, stdout) below the list.
//!
//! Input:
//! - `j` / `k` / `Down` / `Up` select an entry; `g` / `G` jump to the
//!   newest / oldest.
//! - `J` / `K` / `PageDown` / `PageUp` scroll the detail pane.
//! - `y` copies the selected entry (handled by the runtime, which
//!   owns the clipboard).
//! - `Esc` / `q` / `!` close.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use tui_popup::KnownSize;

use crate::tui::Theme;
use crate::tui::messages::{LogEntry, LogLevel, MessageLog};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagesOverlayState {
    /// Index into the newest-first entry list.
    pub selected: usize,
    pub detail_scroll: u16,
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
}

impl crate::tui::Overlay for MessagesOverlayState {
    type Ctx<'a> = &'a MessageLog;

    fn handle(&mut self, log: &MessageLog, key: KeyEvent) -> crate::tui::OverlayOutcome {
        use crate::tui::OverlayOutcome;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return OverlayOutcome::Close;
        }
        let len = log.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | '!') => return OverlayOutcome::Close,
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected + 1, len),
            KeyCode::Char('k') | KeyCode::Up => {
                self.select(self.selected.saturating_sub(1), len);
            }
            KeyCode::Char('g') | KeyCode::Home => self.select(0, len),
            KeyCode::Char('G') | KeyCode::End => self.select(len.saturating_sub(1), len),
            KeyCode::Char('J') => self.scroll_detail(1),
            KeyCode::Char('K') => self.scroll_detail(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_detail(8),
            KeyCode::PageUp => self.scroll_detail(-8),
            _ => {}
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
        LogLevel::Info => Style::default().fg(theme.success),
        LogLevel::Warning => Style::default().fg(theme.warning),
        LogLevel::Error => Style::default().fg(theme.error),
    }
}

impl Widget for MessagesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.saturating_sub(4).clamp(40, 110);
        let height = area.height.saturating_sub(2).max(10);
        let modal = crate::tui::widgets::popup_frame::centered_rect(area, width, height);
        let label_style = Style::default()
            .fg(self.theme.panel_focus_accent)
            .add_modifier(Modifier::BOLD);
        let title = line![
            " ",
            span!(label_style; " Messages · {} ", self.log.len()),
            " "
        ];
        let body = MessagesBody {
            widget: &self,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        crate::tui::widgets::popup_frame::themed_popup(body, title, self.theme).render(area, buf);
    }
}

struct MessagesBody<'a, 'w> {
    widget: &'a MessagesWidget<'w>,
    inner_width: usize,
    inner_height: usize,
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
            Style::default().add_modifier(theme.placeholder);
            "j/k select · J/K scroll detail · y copy · Esc close"
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
        let first = state
            .selected
            .saturating_sub(usize::from(list_height).saturating_sub(1));
        let lines: Vec<Line> = log
            .newest_first()
            .enumerate()
            .skip(first)
            .take(usize::from(list_height))
            .map(|(idx, entry)| list_line(entry, idx == state.selected, theme, now))
            .collect();
        Paragraph::new(lines).render(list_area, buf);

        Paragraph::new(Line::from(span!(
            Style::default().add_modifier(theme.divider);
            "{}", "─".repeat(usize::from(area.width))
        )))
        .render(rule_area, buf);

        if let Some(entry) = state.selected_entry(log) {
            Paragraph::new(entry.full_text())
                .wrap(Wrap { trim: false })
                .scroll((state.detail_scroll, 0))
                .render(detail_area, buf);
        }
        Paragraph::new(footer).render(footer_area, buf);
    }
}

fn list_line(entry: &LogEntry, selected: bool, theme: &Theme, now: i64) -> Line<'static> {
    let age = crate::tui::rows::format_recency(Some(now), Some(entry.at_epoch))
        .unwrap_or_else(|| "—".to_string());
    let mut spans = vec![
        Span::styled(
            format!("{} ", entry.level.glyph()),
            level_style(entry.level, theme),
        ),
        Span::styled(
            format!("{age:>4} ago  "),
            Style::default().fg(theme.secondary_text),
        ),
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
