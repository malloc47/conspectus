//! Shared centered-bordered-modal framing primitive.
//!
//! Conspectus has nine overlays (rename, controls, pins menu + 5
//! pin sub-editors, search, help, value, viewer, multi-select
//! sub-editors) that all carry the same shape: compute a centered
//! Rect with width/height caps, repaint the background so dimmed
//! body content doesn't bleed through, draw a bordered Block with
//! a title, and render a body widget into the inner area. This
//! module centralizes that frame via [`tui_popup::Popup`], so every
//! modal's `Widget::render` collapses to four steps:
//!
//! 1. Build a body wrapper (a per-modal struct implementing
//!    [`KnownSize`] + [`Widget`]) that reports the body width/height
//!    matching the in-tree `centered_modal_rect` caps and renders
//!    the existing per-modal body into the inner area.
//! 2. Pass it to [`themed_popup`] together with a `&Theme` and a
//!    title `Line`.
//! 3. Render the returned [`tui_popup::Popup`] against the frame
//!    area; the upstream renderer handles clear + border + title +
//!    body in one pass.
//! 4. Optionally render any sub-editor or extra overlay on top of
//!    the popup after step 3.
//!
//! Theme bridge (ADR 0032): the border + title both inherit
//! `theme.accent` by default. The body style stays
//! untouched so the per-modal body widget retains full control of
//! its own colors. Override knobs for individual modals can be
//! added later if needed; today's nine modals all use the same
//! palette.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Borders, Paragraph, Widget};
use tui_popup::{KnownSize, Popup};

use crate::tui::Theme;

/// Shared centering math for the overlays. Callers compute their
/// own `width` / `height` (which legitimately vary per widget) and
/// hand them to this one helper.
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Build a [`Popup`] pre-configured with Conspectus's framing
/// defaults: `Borders::ALL`, project-accent border + title styles.
/// The `body` must implement [`KnownSize`] + [`ratatui::widgets::Widget`]
/// (the per-modal wrapper structs in this module's siblings do).
pub fn themed_popup<'a, W>(body: W, title: Line<'static>, theme: &'a Theme) -> Popup<'a, W>
where
    W: KnownSize + ratatui::widgets::Widget,
{
    Popup::new(body)
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(theme.accent))
}

/// Reusable popup-body wrapper for the common case of "a `Vec<Line>`
/// rendered as a `Paragraph`." Holds the inner width/height the
/// in-tree `centered_modal_rect` produced so the popup's auto-sizing
/// reproduces the legacy rect. Per-modal bodies that need extra
/// rendering (cursor cells, multi-region layout, stateful upstream
/// widgets) keep their own per-modal wrapper; this is the convenience
/// hook for plain text bodies.
pub struct LinesBody {
    pub lines: Vec<Line<'static>>,
    pub inner_width: usize,
    pub inner_height: usize,
}

impl KnownSize for LinesBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for LinesBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines).render(area, buf);
    }
}

/// Wrap `text` into lines of at most `width` cells, each prefixed with
/// `indent` spaces, so a modal's prose never runs past its frame.
pub fn wrapped_lines(text: &str, indent: usize, width: usize) -> Vec<String> {
    let budget = u16::try_from(width.saturating_sub(indent).max(1)).unwrap_or(u16::MAX);
    crate::viewer::render::word_wrap(text, budget)
        .into_iter()
        .map(|chunk| format!("{}{}", " ".repeat(indent), chunk.trim_end()))
        .collect()
}

/// What a scrolling modal body showed when it was drawn: the rows of text
/// it had and how far that text can scroll. The draw path writes it back
/// into the modal's state, so a page key moves by the rows the operator
/// saw and an offset past the end is taken back before the next key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollFit {
    pub rows: usize,
    pub max_scroll: usize,
}
