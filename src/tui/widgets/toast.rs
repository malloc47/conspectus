//! Transient toast widget (T8-040).
//!
//! Surfaces short, non-blocking feedback (`copied: cwd`, `copied: id`,
//! …) at the bottom of the screen for a fixed duration after a copy or
//! similar one-shot action. The toast is a render-only overlay: input
//! continues to flow to the underlying view, and a newer toast simply
//! replaces an older one.
//!
//! Auto-dismiss is driven entirely by `posted_at.elapsed()` at render
//! time. The reducer never inspects the timer; the runtime's normal
//! poll cadence redraws often enough that an expired toast simply
//! doesn't render on the next frame.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::tui::Theme;

/// How long a toast stays visible before it's auto-dismissed.
pub const TOAST_DURATION: Duration = Duration::from_millis(1500);

/// One toast's worth of state: the label to display and the moment it
/// was posted. `posted_at` is set by the runtime (`Cmd` boundary) when
/// the toast is created so the reducer stays pure.
#[derive(Debug, Clone)]
pub struct ToastState {
    pub label: String,
    pub posted_at: Instant,
}

impl ToastState {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            posted_at: Instant::now(),
        }
    }

    /// Whether this toast is past its auto-dismiss window.
    pub fn is_expired(&self) -> bool {
        self.posted_at.elapsed() >= TOAST_DURATION
    }
}

/// Centered one-line toast rendered along the bottom edge of the
/// frame. Clears the cells it occupies so it reads as a distinct
/// overlay over whatever is behind it.
pub struct ToastWidget<'a> {
    state: &'a ToastState,
    theme: &'a Theme,
}

impl<'a> ToastWidget<'a> {
    pub fn new(state: &'a ToastState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for ToastWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.state.is_expired() {
            return;
        }
        let Some(rect) = toast_rect(area, &self.state.label) else {
            return;
        };
        // Clear cells under the toast so we don't blend with content
        // behind it (mirrors the value-modal clear contract).
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(rect);
        block.render(rect, buf);
        let body = Paragraph::new(Line::from(Span::styled(
            self.state.label.clone(),
            Style::default()
                .fg(self.theme.success)
                .add_modifier(Modifier::BOLD),
        )));
        body.render(inner, buf);
    }
}

/// Place a 3-row box (border + 1 line + border) along the bottom edge,
/// centered horizontally. Returns `None` when the available area is
/// too small for a meaningful toast.
fn toast_rect(area: Rect, label: &str) -> Option<Rect> {
    if area.height < 3 || area.width < 6 {
        return None;
    }
    // Inner content width: at least the label's display width, capped
    // by what the surrounding area can host (leave a one-cell margin
    // on each side so the toast doesn't kiss the frame edge).
    let label_width = UnicodeWidthStr::width(label) as u16;
    let max_inner = area.width.saturating_sub(4); // 2 borders + 1 margin each side
    let inner_width = label_width.min(max_inner).max(1);
    let outer_width = inner_width + 2; // add border columns
    let x = area.left() + (area.width.saturating_sub(outer_width)) / 2;
    let y = area.bottom().saturating_sub(3);
    Some(Rect::new(x, y, outer_width, 3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn newly_posted_toast_is_not_expired() {
        let state = ToastState::new("copied: cwd");
        assert!(!state.is_expired());
    }

    #[test]
    fn expiry_kicks_in_after_toast_duration() {
        let state = ToastState {
            label: "copied: cwd".into(),
            // Simulate a toast posted past its window by reaching
            // back in time via `checked_sub` on the Instant.
            posted_at: Instant::now()
                .checked_sub(TOAST_DURATION + Duration::from_millis(50))
                .unwrap_or_else(Instant::now),
        };
        assert!(state.is_expired());
    }

    #[test]
    fn replacement_supersedes_older_toast() {
        // The runtime always replaces the entire ToastState when
        // posting a new toast, so the older posted_at is dropped.
        // This test pins the contract: posting a second toast resets
        // the timer to "now".
        let mut current = ToastState::new("copied: cwd");
        sleep(Duration::from_millis(10));
        let original_posted_at = current.posted_at;
        current = ToastState::new("copied: id");
        assert_eq!(current.label, "copied: id");
        assert!(
            current.posted_at > original_posted_at,
            "replacement toast must reset the timer"
        );
    }

    #[test]
    fn toast_rect_centers_at_bottom_and_fits_label() {
        let area = Rect::new(0, 0, 80, 24);
        let rect = toast_rect(area, "copied: cwd").expect("rect available");
        assert_eq!(rect.height, 3);
        // 11 visible chars + 2 borders = 13 wide.
        assert_eq!(rect.width, 13);
        // Bottom-anchored.
        assert_eq!(rect.bottom(), area.bottom());
        // Horizontally centered: (80 - 13) / 2 = 33.
        assert_eq!(rect.left(), 33);
    }

    #[test]
    fn toast_rect_caps_inner_width_to_available_area() {
        // Very narrow frame: the rect must still fit and leave a
        // 1-cell margin on each side of the border.
        let area = Rect::new(0, 0, 10, 5);
        let rect =
            toast_rect(area, "an extremely long label that overflows").expect("rect available");
        // inner = min(label, area.width - 4) = min(N, 6) = 6
        // outer = 6 + 2 borders = 8
        assert_eq!(rect.width, 8);
    }

    #[test]
    fn toast_rect_refuses_tiny_areas() {
        assert!(toast_rect(Rect::new(0, 0, 5, 3), "label").is_none());
        assert!(toast_rect(Rect::new(0, 0, 80, 2), "label").is_none());
    }

    #[test]
    fn expired_toast_renders_nothing() {
        let state = ToastState {
            label: "copied".into(),
            posted_at: Instant::now()
                .checked_sub(TOAST_DURATION + Duration::from_millis(50))
                .unwrap_or_else(Instant::now),
        };
        let theme = Theme::default();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        ToastWidget::new(&state, &theme).render(area, &mut buf);
        // No cell should carry the success color (toast bypassed).
        let painted = (0..area.width)
            .flat_map(|x| (0..area.height).map(move |y| (x, y)))
            .any(|(x, y)| {
                buf.cell((x, y))
                    .map(|c| c.fg == ratatui::style::Color::Reset || c.symbol() == "─")
                    .unwrap_or(false)
                    && buf
                        .cell((x, y))
                        .map(|c| c.symbol() == "─" || c.symbol() == "│")
                        .unwrap_or(false)
            });
        assert!(!painted, "expired toast must not draw borders");
    }

    #[test]
    fn rendered_toast_paints_borders_and_label() {
        let state = ToastState::new("copied: cwd");
        let theme = Theme::default();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        ToastWidget::new(&state, &theme).render(area, &mut buf);
        // The label string should appear contiguously in the inner
        // row. Scan the row just above the bottom border for the
        // label text.
        let inner_y = area.bottom() - 2;
        let mut found = String::new();
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell((x, inner_y)) {
                found.push_str(cell.symbol());
            }
        }
        assert!(
            found.contains("copied: cwd"),
            "expected label in rendered row, got {found:?}"
        );
    }
}
