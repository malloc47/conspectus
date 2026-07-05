//! Filled-pill badge primitive for compact identity chips.
//!
//! Per ADR 0032 and the styling overhaul plan, harness labels are
//! rendered as REVERSED+BOLD pills against the harness color so they
//! read as visually-distinct chips in dense lists. The same primitive
//! backs header chips (Phase 5), the detail-pane identity row
//! (Phase 6), and group-row summaries (Phase 7).
//!
//! Layout contract: `harness_badge` always renders to
//! [`HARNESS_BADGE_WIDTH`] visible cells. Short labels are
//! right-padded *inside* the styled span so the entire badge — the
//! padding cells included — carries the badge color and modifier.
//! This matches the chip look in modern AI-agent dashboards where
//! every pill is the same width regardless of label length, and it
//! keeps the recency / mux indicator columns aligned downstream.

use ratatui::macros::span;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::tui::Theme;

/// Longest harness label conspectus emits today (`opencode`, 8
/// chars). Drives the fixed badge width below.
const MAX_HARNESS_LABEL_LEN: usize = 8;

/// Total cell width of every harness badge (1 leading space + the
/// max label width + 1 trailing space).
pub const HARNESS_BADGE_WIDTH: usize = MAX_HARNESS_LABEL_LEN + 2;

/// Render `label` as a fixed-width filled badge. The badge uses
/// `theme.badge` (default `REVERSED | BOLD`) over
/// `theme.harness_color(label)` and is padded to
/// [`HARNESS_BADGE_WIDTH`] cells so every chip has the same visual
/// footprint. Labels longer than the configured max are rendered as-is
/// (they'll widen the chip), which is harmless because the column
/// math downstream measures the actual span width.
pub fn harness_badge(label: &str, theme: &Theme) -> Span<'static> {
    let style = Style::default()
        .fg(theme.harness_color(label))
        .add_modifier(theme.badge);
    let body_width = label.chars().count().max(MAX_HARNESS_LABEL_LEN);
    span!(style; " {label:<body_width$} ")
}

/// Visible cell width of [`harness_badge`]'s output for a given
/// label. Today every known harness fits inside [`HARNESS_BADGE_WIDTH`];
/// the helper still measures from the label so a future longer
/// harness key (e.g. `claude-code-2`) widens gracefully.
pub fn harness_badge_width(label: &str) -> usize {
    label.chars().count().max(MAX_HARNESS_LABEL_LEN) + 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn harness_badge_pads_short_labels_inside_styled_span() {
        // codex is 5 chars; the badge pads to the max-label width
        // (8 cells) inside the styled span so the entire badge —
        // padding included — carries the chip background.
        let theme = Theme::default();
        let span = harness_badge("codex", &theme);
        assert_eq!(span.content, " codex    ");
        assert_eq!(span.content.chars().count(), HARNESS_BADGE_WIDTH);
        assert_eq!(span.style.fg, Some(theme.harness_color("codex")));
        assert!(span.style.add_modifier.contains(Modifier::REVERSED));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn harness_badge_matches_max_label_width_without_padding() {
        let theme = Theme::default();
        let span = harness_badge("opencode", &theme);
        assert_eq!(span.content, " opencode ");
        assert_eq!(span.content.chars().count(), HARNESS_BADGE_WIDTH);
    }

    #[test]
    fn harness_badge_unknown_harness_uses_unknown_color() {
        let theme = Theme::default();
        let span = harness_badge("custom", &theme);
        assert_eq!(span.style.fg, Some(theme.harness_unknown));
    }

    #[test]
    fn harness_badge_width_matches_actual_render() {
        // All known-length harnesses render at HARNESS_BADGE_WIDTH;
        // an over-long label widens the chip past the constant so
        // downstream column math still measures correctly.
        assert_eq!(harness_badge_width("claude"), HARNESS_BADGE_WIDTH);
        assert_eq!(harness_badge_width("codex"), HARNESS_BADGE_WIDTH);
        assert_eq!(harness_badge_width("opencode"), HARNESS_BADGE_WIDTH);
        assert_eq!(harness_badge_width("custom-harness-x"), 16 + 2);
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
