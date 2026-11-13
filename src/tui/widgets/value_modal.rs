//! Full-value inspection modal: the framework's pager (`tuikit::pager`)
//! over a long value, framed at a reader's size.
//!
//! Surfaces the untruncated text of a long detail-pane value (`cwd`,
//! `command`, `url`, `last_message_preview`, …) so the operator can read
//! or copy the full content without forcing it into the main right-panel
//! layout, and shows a rebuild plan.
//!
//! The pager owns the reading: scrolling and viewport-sized paging, `g`
//! and `G` (which keeps to the last page), the scrollbar, `Esc`, `q`, and
//! `o` (the key that opened it) closing, and the footer. What is the
//! value's is here: the label as the title, the value wrapped to the
//! frame with a rebuild plan's command lines (`$ …`) in the accent style,
//! and `y`, whose copy hook asks the runtime to copy the text, or the
//! payload the opener supplied (a rebuild plan copies its commands as a
//! script rather than its prose); the runtime does the clipboard write
//! and the toast.

use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::Line;
use tuikit::frame::ModalSize;
use tuikit::keys::KeyRow;
use tuikit::pager::{NoKeys, Pager, PagerContent, PagerEffect, PagerMode};
use tuikit::theme::Theme;

use crate::tui::Msg;
use crate::tui::keybindings::overlay::VALUE_MODAL_KEYS;

/// The value modal: the framework's pager over a value.
pub type ValueModal = Pager<ValueText>;

/// A long value laid out for the modal's width.
#[derive(Debug, Clone)]
pub struct ValueText {
    /// Static label rendered in the title bar (`cwd`, `command`, …).
    pub label: String,
    /// The full untruncated value being inspected.
    pub value: String,
    /// What `y` copies when that isn't the value itself.
    pub copy: Option<ValueCopy>,
    /// Draw lines that show a command (`$ …`) in the accent style.
    pub highlight_commands: bool,
    /// The value wrapped at the last width it was laid out at.
    rows: Vec<Line<'static>>,
    laid_out: Option<u16>,
}

/// Text a value modal copies to the clipboard, and how to describe it
/// in the toast and the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCopy {
    pub text: String,
    /// Noun phrase for the footer and toast: `3 commands`.
    pub what: String,
}

impl ValueText {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            copy: None,
            highlight_commands: false,
            rows: Vec::new(),
            laid_out: None,
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
        let mut value = Self::new(label, text);
        value.highlight_commands = true;
        value.copy = Some(ValueCopy {
            text: script.into(),
            what: format!("{commands} command{}", if commands == 1 { "" } else { "s" }),
        });
        value
    }

    /// What `y` puts on the clipboard, and how to name it.
    pub fn copy_payload(&self) -> (&str, String) {
        match &self.copy {
            Some(copy) => (copy.text.as_str(), copy.what.clone()),
            None => (self.value.as_str(), "text".to_string()),
        }
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

impl PagerContent for ValueText {
    type Message = Msg;
    type Action = NoKeys;
    const KEYS: &'static [KeyRow<NoKeys>] = &[];

    fn layout(&mut self, width: u16, theme: &Theme) -> usize {
        if self.laid_out != Some(width) {
            self.rows = self.wrapped(width, theme);
            self.laid_out = Some(width);
        }
        self.rows.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.rows[range].to_vec()
    }

    fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
        Line::from(self.label.clone())
    }

    fn apply(&mut self, action: NoKeys) -> PagerEffect<Msg> {
        match action {}
    }

    /// `y`: the runtime copies the payload and says what it copied.
    fn copy(&self) -> Option<Msg> {
        let (text, what) = self.copy_payload();
        Some(Msg::Copy {
            text: text.to_string(),
            done: format!("Copied {what}"),
        })
    }
}

/// The modal over `value`, at the top: a reader's frame, the value's
/// keys (`VALUE_MODAL_KEYS`: the pager's, `o` closing as it opened, and
/// `y` copying).
pub fn open(value: ValueText) -> ValueModal {
    Pager::new(value)
        .mode(PagerMode::Modal(ModalSize::WIDE))
        .keys(VALUE_MODAL_KEYS)
}

#[cfg(test)]
#[path = "value_modal_tests.rs"]
mod tests;
