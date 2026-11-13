//! Sorting an age into four steps and tinting each from the theme.

use ratatui::style::Style;

use super::Age;
use crate::theme::Theme;

/// How recent an age is, in four steps from freshest to oldest. Each
/// step styles from the theme token of the same name
/// (`recency_fresh` .. `recency_cold`), so a column of ages carries its
/// freshness in color as well as in text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Recency {
    /// Younger than the scale's `fresh` bound (five minutes by default).
    Fresh,
    /// Younger than the `active` bound (an hour).
    Active,
    /// Younger than the `recent` bound (a day).
    Recent,
    /// As old as the `recent` bound or older.
    Cold,
}

impl Recency {
    /// Every step, freshest first.
    pub const ALL: [Self; 4] = [Self::Fresh, Self::Active, Self::Recent, Self::Cold];

    /// The step's style: the theme's `recency_fresh`, `recency_active`,
    /// `recency_recent`, or `recency_cold`.
    pub fn style(self, theme: &Theme) -> Style {
        match self {
            Self::Fresh => theme.recency_fresh,
            Self::Active => theme.recency_active,
            Self::Recent => theme.recency_recent,
            Self::Cold => theme.recency_cold,
        }
        .into_style()
    }
}

/// Where one [`Recency`] step ends and the next begins: each bound is
/// the first age of the next step, so an age of exactly five minutes is
/// active, not fresh.
///
/// The bounds are public so a host can pass its own; they should grow
/// from `fresh` to `recent`, and a bound out of order only makes the step
/// it closes empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecencyScale {
    /// The first age that is no longer fresh.
    pub fresh: Age,
    /// The first age that is no longer active.
    pub active: Age,
    /// The first age that is cold.
    pub recent: Age,
}

impl RecencyScale {
    /// The step `age` falls in.
    pub fn recency(&self, age: Age) -> Recency {
        if age < self.fresh {
            Recency::Fresh
        } else if age < self.active {
            Recency::Active
        } else if age < self.recent {
            Recency::Recent
        } else {
            Recency::Cold
        }
    }
}

impl Default for RecencyScale {
    /// Fresh under five minutes, active under an hour, recent under a
    /// day, and cold after.
    fn default() -> Self {
        Self {
            fresh: Age::from_secs(5 * 60),
            active: Age::from_secs(60 * 60),
            recent: Age::from_secs(24 * 60 * 60),
        }
    }
}
