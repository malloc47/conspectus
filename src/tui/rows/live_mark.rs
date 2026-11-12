//! What an agent's live state reads as, in every view.

use ratatui::style::Style;
use tuikit::chip::Tone;

use crate::model::live::{LiveConfidence, LiveCounts, LiveState};
use crate::tui::Theme;
use crate::tui::icons::live;

use super::{LiveCell, MixedLive, words};

/// The shape a row draws its entity's live state in: one for each thing
/// the operator acts on (waiting, ready, an error), one for work under
/// way, one for evidence that can't be trusted, and one for none. It
/// decides the live cell of session and mux rows, the word of a Pinned
/// or History row, and the help legend, so they all agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LiveMark {
    Waiting,
    Working,
    Ready,
    Error,
    Unknown,
    None,
}

impl LiveMark {
    /// Every mark, in the order the help legend lists them.
    pub const ALL: [Self; 6] = [
        Self::Waiting,
        Self::Working,
        Self::Ready,
        Self::Error,
        Self::Unknown,
        Self::None,
    ];

    /// The mark of a session in `state`. Compacting and starting are work
    /// under way; a session that exited has nothing to show, like one
    /// with no evidence.
    pub fn of_state(state: LiveState) -> Self {
        match state {
            LiveState::Waiting => Self::Waiting,
            LiveState::Working | LiveState::Compacting | LiveState::Starting => Self::Working,
            LiveState::Ready => Self::Ready,
            LiveState::Error => Self::Error,
            LiveState::Unknown => Self::Unknown,
            LiveState::Exited => Self::None,
        }
    }

    /// The one-cell glyph of the mark.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Waiting => live::WAITING,
            Self::Working => live::WORKING,
            Self::Ready => live::READY,
            Self::Error => live::ERROR,
            Self::Unknown => live::UNKNOWN,
            Self::None => live::NONE,
        }
    }

    /// The word a Pinned or History row prints for a session with this
    /// mark, in place of the word for its link to a mux; `None` where the
    /// row keeps that word, because there is nothing live to say.
    pub fn word(self) -> Option<&'static str> {
        match self {
            Self::Waiting => Some(words::WAITING),
            Self::Working => Some(words::WORKING),
            Self::Ready => Some(words::READY),
            Self::Error => Some(words::ERROR),
            Self::Unknown | Self::None => None,
        }
    }

    /// The tone the mark is drawn in: the theme's warning, info, success,
    /// and error colors for the four states, and the muted look for
    /// evidence that can't be trusted and for none. A chip that counts
    /// agents by their mark takes the same tone.
    pub fn tone(self) -> Tone {
        match self {
            Self::Waiting => Tone::Warning,
            Self::Working => Tone::Info,
            Self::Ready => Tone::Success,
            Self::Error => Tone::Error,
            Self::Unknown | Self::None => Tone::Muted,
        }
    }

    /// The style the glyph and the word are drawn in: the mark's
    /// [tone](Self::tone).
    pub fn style(self, theme: &Theme) -> Style {
        self.tone().style(theme)
    }

    /// How many of the agents `counts` counts draw this mark.
    pub fn count_in(self, counts: LiveCounts) -> u32 {
        LiveState::ALL
            .into_iter()
            .filter(|state| Self::of_state(*state) == self)
            .fold(0u32, |total, state| total.saturating_add(counts.get(state)))
    }
}

/// What a row shows of its entity's live state: the mark, and whether
/// the evidence for it has gone quiet (work with no news for longer than
/// the source's quiet window), which draws the same mark dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveSign {
    pub mark: LiveMark,
    pub quiet: bool,
}

impl LiveSign {
    /// No evidence.
    pub const NONE: Self = Self {
        mark: LiveMark::None,
        quiet: false,
    };

    /// A session's sign: its effective state, or none without evidence.
    pub fn of_session(cell: Option<LiveCell>) -> Self {
        cell.map_or(Self::NONE, |cell| Self {
            mark: LiveMark::of_state(cell.state),
            quiet: cell.confidence == LiveConfidence::Quiet,
        })
    }

    /// A mux's sign: the state of its agents that ranks first by the
    /// model's default precedence (waiting, error, ready, working, then
    /// the rest), or none when no agent of it has live state. The counts
    /// say how many agents are in each state, not how fresh each one's
    /// evidence is, so a mux's mark is never drawn quiet; the agents'
    /// own rows are.
    pub fn of_mux(counts: LiveCounts) -> Self {
        counts.dominant().map_or(Self::NONE, |state| Self {
            mark: LiveMark::of_state(state),
            quiet: false,
        })
    }

    /// The sign of a row that can stand for an entity of any kind.
    pub fn of_mixed(live: MixedLive) -> Self {
        match live {
            MixedLive::None => Self::NONE,
            MixedLive::Session(cell) => Self::of_session(Some(cell)),
            MixedLive::Mux(counts) => Self::of_mux(counts),
        }
    }

    /// The mark's glyph.
    pub fn glyph(self) -> &'static str {
        self.mark.glyph()
    }

    /// The mark's word, if it has one ([`LiveMark::word`]).
    pub fn word(self) -> Option<&'static str> {
        self.mark.word()
    }

    /// The mark's style, receding (the theme's muted look) when quiet.
    pub fn style(self, theme: &Theme) -> Style {
        let style = self.mark.style(theme);
        if self.quiet {
            style.add_modifier(theme.muted)
        } else {
            style
        }
    }
}

#[cfg(test)]
#[path = "live_mark_tests.rs"]
mod tests;
