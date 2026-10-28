//! The chips Conspectus draws on rows: who runs a session or a pane,
//! and the short notes beside a row's label.
//!
//! Each is a thin adapter over a chip form of `tuikit::chip`, which owns
//! the width rule and the look: a harness or program chip is a filled
//! [`Chip`] whose label is cut or padded to `[tui.theme] badge_width`
//! cells inside one cell of padding on each side, so every chip in a
//! column has the same footprint and the columns after it stay aligned
//! (ADR 0032). What stays here is what each chip means: the harness
//! colors, the program chip's colors, and the words of the notes.

use std::borrow::Cow;

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::chip::{Chip, StatusChip, Tone};

use crate::tui::Theme;

/// Render `label` as the harness chip: the theme's chip look (default
/// `REVERSED | BOLD`) over `theme.harness_color(label)`, so every chip
/// has the same footprint and the columns after it stay aligned. At the
/// default width every registered harness label fits; anything longer (a
/// custom launcher, say) is cut with `…`.
pub fn harness_badge(label: &str, theme: &Theme) -> Span<'static> {
    Chip::new(label)
        .color(theme.harness_color(label))
        .span(theme)
}

/// Render a pane's program name as the program chip, the same width as
/// [`harness_badge`], for mux rows with no linked agent. Every program
/// shares `theme.command_badge()` as the background, under bold
/// `theme.command_badge_text()` (white by default), so these chips stay
/// visually quieter than the per-harness colors. Names longer than the
/// chip are cut with `…`.
pub fn command_badge(command: &str, theme: &Theme) -> Span<'static> {
    let style = Style::default()
        .fg(theme.command_badge_text())
        .bg(theme.command_badge())
        .add_modifier(Modifier::BOLD);
    Chip::new(command).style(style).span(theme)
}

/// The chip of a mux that runs no agent and no known program: `no
/// agent`, muted, in a chip's width so the age after it lines up with
/// the rows that have a chip.
pub fn no_agent_chip(theme: &Theme) -> Span<'static> {
    Chip::new("no agent")
        .style(Style::default().add_modifier(theme.muted))
        .span(theme)
}

/// The note after a row whose session a content search matched: `1 hit`
/// or `N hits`, receding, two cells after what precedes it.
pub fn hits_chip(count: u32, theme: &Theme) -> Span<'static> {
    let label = if count == 1 {
        "1 hit".to_string()
    } else {
        format!("{count} hits")
    };
    StatusChip::new(label, Tone::Inert).pad(2, 0).span(theme)
}

/// The note after a session's label when the mux it last ran in is gone:
/// `was in <mux>`, or `(mux stopped)` when the label already names that
/// mux. Receding, two cells after the label.
pub fn was_in_chip(mux: &str, named_after_it: bool, theme: &Theme) -> Span<'static> {
    let label: Cow<'static, str> = if named_after_it {
        Cow::Borrowed("(mux stopped)")
    } else {
        Cow::Owned(format!("was in {}", tuikit::text::truncate_end(mux, 20)))
    };
    StatusChip::new(label, Tone::Inert).pad(2, 0).span(theme)
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod tests;
