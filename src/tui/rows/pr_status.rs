//! What a pull request's state reads as, in every view.

use ratatui::style::Style;

use crate::tui::Theme;
use crate::tui::icons::state;

use super::words;

/// A pull request's state as the TUI shows it, from the forge's state and
/// draft flag. It decides the state glyph of a PR row, the hue of every
/// PR kind glyph, the status word of the Pinned and History views, the
/// state groups of the PRs view, and the help legend, so they all agree.
///
/// The forge's state beats the draft flag: a closed or merged PR reads
/// closed or merged even if it is still marked draft, and only an open
/// one reads as a draft. A PR with no state, or a state this code does
/// not know, reads as unknown, draft or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrStatus {
    Open,
    Draft,
    Merged,
    Closed,
    Unknown,
}

impl PrStatus {
    /// Every status, in the order the PRs view groups them.
    pub const ALL: [Self; 5] = [
        Self::Open,
        Self::Draft,
        Self::Merged,
        Self::Closed,
        Self::Unknown,
    ];

    /// The status of a PR whose forge state is `state` (any case) and
    /// whose draft flag is `is_draft`.
    pub fn of(state: Option<&str>, is_draft: bool) -> Self {
        let state = state.unwrap_or_default();
        let is = |wanted: &str| state.eq_ignore_ascii_case(wanted);
        if is("open") {
            if is_draft { Self::Draft } else { Self::Open }
        } else if is("merged") {
            Self::Merged
        } else if is("closed") {
            Self::Closed
        } else {
            Self::Unknown
        }
    }

    /// The word the Pinned and History views print for it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Open => words::PR_OPEN,
            Self::Draft => words::PR_DRAFT,
            Self::Merged => words::PR_MERGED,
            Self::Closed => words::PR_CLOSED,
            Self::Unknown => words::UNKNOWN,
        }
    }

    /// The state glyph and the style every PR glyph and status word of
    /// this state is drawn in: one shape per state so it reads without
    /// color, and one hue so the kind glyph, the state glyph, and the
    /// word read as one signal.
    pub fn mark(self, theme: &Theme) -> (&'static str, Style) {
        let hue = |color| Style::default().fg(color);
        match self {
            Self::Open => (state::PR_OPEN, hue(theme.pr_open())),
            Self::Draft => (state::PR_DRAFT, hue(theme.pr_draft())),
            Self::Merged => (state::PR_MERGED, hue(theme.pr_merged())),
            Self::Closed => (state::PR_CLOSED, hue(theme.pr_closed())),
            Self::Unknown => (state::UNKNOWN, Style::default().add_modifier(theme.muted)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_beats_draft_for_every_state_crossed_with_draft() {
        use PrStatus::{Closed, Draft, Merged, Open, Unknown};
        let cases = [
            (Some("open"), false, Open),
            (Some("open"), true, Draft),
            (Some("OPEN"), true, Draft),
            (Some("merged"), false, Merged),
            (Some("merged"), true, Merged),
            (Some("closed"), false, Closed),
            (Some("Closed"), true, Closed),
            (None, false, Unknown),
            (None, true, Unknown),
            (Some(""), true, Unknown),
            (Some("locked"), false, Unknown),
        ];
        for (state, draft, want) in cases {
            assert_eq!(
                PrStatus::of(state, draft),
                want,
                "state {state:?}, draft {draft}"
            );
        }
    }

    #[test]
    fn every_status_has_its_own_word_glyph_and_style() {
        let theme = Theme::default();
        let words: Vec<_> = PrStatus::ALL.map(PrStatus::word).into();
        assert_eq!(words, ["open", "draft", "merged", "closed", "unknown"]);
        let marks = PrStatus::ALL.map(|status| status.mark(&theme));
        for (index, (glyph, style)) in marks.iter().enumerate() {
            for (other_glyph, other_style) in &marks[index + 1..] {
                assert_ne!(glyph, other_glyph);
                assert_ne!(style, other_style);
            }
        }
    }
}
