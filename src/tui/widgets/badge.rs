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
use ratatui::style::{Color, Modifier, Style};
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
/// `theme.harness_color(label)` and is exactly [`HARNESS_BADGE_WIDTH`]
/// cells, so every chip has the same footprint and the columns after
/// it stay aligned. Registered harness labels all fit; anything longer
/// (a pin's custom launcher, say) is cut with `…`.
pub fn harness_badge(label: &str, theme: &Theme) -> Span<'static> {
    let style = Style::default()
        .fg(theme.harness_color(label))
        .add_modifier(theme.badge);
    badge_span(label, style)
}

/// Render a pane's program name as a badge the same width as
/// [`harness_badge`], for mux rows with no linked agent. Every program
/// shares `theme.command_badge` as the background, under bold white
/// text, so these chips stay visually quieter than the per-harness
/// colors. Names longer than the badge are cut with `…`.
pub fn command_badge(command: &str, theme: &Theme) -> Span<'static> {
    let style = Style::default()
        .fg(Color::White)
        .bg(theme.command_badge)
        .add_modifier(Modifier::BOLD);
    badge_span(command, style)
}

/// `label` padded or cut (with `…`) to the badge body, inside one
/// styled span so the padding carries the chip background too.
fn badge_span(label: &str, style: Style) -> Span<'static> {
    let label = if label.chars().count() > MAX_HARNESS_LABEL_LEN {
        let kept: String = label.chars().take(MAX_HARNESS_LABEL_LEN - 1).collect();
        format!("{kept}…")
    } else {
        label.to_string()
    };
    span!(style; " {label:<MAX_HARNESS_LABEL_LEN$} ")
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
    fn command_badge_matches_harness_badge_width_and_truncates() {
        let theme = Theme::default();
        let short = command_badge("npm", &theme);
        assert_eq!(short.content, " npm      ");
        let long = command_badge("conspectus", &theme);
        assert_eq!(long.content, " conspec… ");
        assert_eq!(long.content.chars().count(), HARNESS_BADGE_WIDTH);
        assert_eq!(long.style.bg, Some(theme.command_badge));
        assert_eq!(long.style.fg, Some(Color::White));
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
    fn harness_badge_truncates_labels_longer_than_the_badge() {
        let theme = Theme::default();
        let span = harness_badge("conspectus", &theme);
        assert_eq!(span.content, " conspec… ");
        assert_eq!(span.content.chars().count(), HARNESS_BADGE_WIDTH);
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
