//! Filled-pill badge primitive for compact identity chips.
//!
//! Per ADR 0032 and the styling overhaul plan, harness labels are
//! rendered as REVERSED+BOLD pills against the harness color so they
//! read as visually-distinct chips in dense lists. The same primitive
//! backs header chips (Phase 5), the detail-pane identity row
//! (Phase 6), and group-row summaries (Phase 7).
//!
//! Layout contract: `harness_badge` consumes
//! `label.chars().count() + 2` visible cells (one space padding on
//! each side). When a caller wants a fixed-width column, follow the
//! badge with the span returned by [`harness_badge_padding`] to fill
//! any remaining cells with unstyled space; the recency / mux
//! indicator columns then stay aligned across sessions with
//! different-length harness labels.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::tui::Theme;

/// Render `label` as a filled badge span. The badge uses
/// `theme.badge` (default `REVERSED | BOLD`) over
/// `theme.harness_color(label)` so it reads as a colored pill
/// against the terminal's default background.
pub fn harness_badge(label: &str, theme: &Theme) -> Span<'static> {
    let style = Style::default()
        .fg(theme.harness_color(label))
        .add_modifier(theme.badge);
    Span::styled(format!(" {label} "), style)
}

/// Visible cell width of [`harness_badge`]'s output. Equal to
/// `label.chars().count() + 2`.
pub fn harness_badge_width(label: &str) -> usize {
    label.chars().count() + 2
}

/// Plain trailing-space span sized to right-pad the badge so its
/// label cell aligns to `target_label_width`. Returns `None` when
/// the label already meets or exceeds the target — callers can omit
/// the padding span entirely in that case.
pub fn harness_badge_padding(label: &str, target_label_width: usize) -> Option<Span<'static>> {
    let actual = label.chars().count();
    if actual >= target_label_width {
        None
    } else {
        Some(Span::raw(" ".repeat(target_label_width - actual)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn harness_badge_uses_theme_color_and_badge_modifier() {
        let theme = Theme::default();
        let span = harness_badge("codex", &theme);
        assert_eq!(span.content, " codex ");
        assert_eq!(span.style.fg, Some(theme.harness_color("codex")));
        assert!(span.style.add_modifier.contains(Modifier::REVERSED));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn harness_badge_unknown_harness_uses_unknown_color() {
        let theme = Theme::default();
        let span = harness_badge("custom", &theme);
        assert_eq!(span.style.fg, Some(theme.harness_unknown));
    }

    #[test]
    fn harness_badge_width_matches_actual_render() {
        assert_eq!(harness_badge_width("claude"), 8);
        assert_eq!(harness_badge_width("opencode"), 10);
        assert_eq!(harness_badge_width(""), 2);
    }

    #[test]
    fn harness_badge_padding_aligns_short_labels() {
        // claude is 6 chars; align to 8 → 2-cell trailing pad.
        let pad = harness_badge_padding("claude", 8).expect("padding needed");
        assert_eq!(pad.content, "  ");
        assert_eq!(pad.style, Style::default(), "padding stays unstyled");
    }

    #[test]
    fn harness_badge_padding_skipped_when_label_meets_target() {
        assert!(harness_badge_padding("opencode", 8).is_none());
        assert!(harness_badge_padding("codex", 5).is_none());
    }

    #[test]
    fn harness_badge_picks_correct_color_per_known_harness() {
        let theme = Theme::default();
        assert_eq!(
            harness_badge("claude", &theme).style.fg,
            Some(Color::Magenta)
        );
        assert_eq!(harness_badge("codex", &theme).style.fg, Some(Color::Cyan));
        assert_eq!(
            harness_badge("opencode", &theme).style.fg,
            Some(Color::Green)
        );
        assert_eq!(harness_badge("aider", &theme).style.fg, Some(Color::Red));
    }
}
