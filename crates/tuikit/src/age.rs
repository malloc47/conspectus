//! Age: how long ago something happened, as compact text and as a tint.
//!
//! This is a layer 1 foundation. A list shows "last active 5m ago" in a
//! cell four wide, a detail field shows `2h ago`, and a status line says
//! a scan ran `30s ago`. When each surface formats its own, one says
//! `just now` where another says `0s`, one stops at hours where another
//! goes on to days, and a row's age reads differently from the same
//! item's age in a panel beside it. Here there is one of each:
//!
//! - **The age**: [`Age`] is a count of seconds, made from two Unix times
//!   with [`Age::between`] (a time in the future, which clock skew
//!   produces, is an age of zero) or [`Age::of`] when either may be
//!   unknown.
//! - **The text**: a [`Ladder`] of [`Unit`]s writes an age in the largest
//!   unit it has reached, rounded down: `45s`, `17m`, `3h`, `12d`.
//!   [`Ladder::DAYS`] is the default and stops at days; [`Ladder::WEEKS`]
//!   goes on to weeks; [`Ladder::new`] takes any units. [`Ladder::ago`]
//!   adds the one suffix prose uses (`5m ago`).
//! - **The tint**: [`Recency`] sorts an age into four steps (fresh,
//!   active, recent, cold) at the bounds a [`RecencyScale`] sets (five
//!   minutes, an hour, a day by default), and styles each step from the
//!   theme's `recency_*` tokens.
//! - **The cell**: [`cell`] draws an age right-aligned in a fixed width
//!   ([`CELL_WIDTH`] by default), tinted by its recency, with the
//!   no-value mark when the age is unknown, so every column of ages
//!   lines up and reads the same.
//!
//! ```
//! use tuikit::age::{Age, Ladder, Recency};
//!
//! let now = 1_000_000;
//! let age = Age::between(now - 3 * 3600 - 59, now);
//! assert_eq!(age.label(), "3h");
//! assert_eq!(age.ago(), "3h ago");
//! assert_eq!(age.recency(), Recency::Recent);
//!
//! // Clock skew is an age of zero; an unknown time has no age.
//! assert_eq!(Age::between(now + 30, now).label(), "0s");
//! assert_eq!(Age::of(None, Some(now)), None);
//!
//! // The default ladder stops at days; the weeks ladder goes on.
//! let old = Age::from_secs(20 * 86_400);
//! assert_eq!(Ladder::DAYS.label(old), "20d");
//! assert_eq!(Ladder::WEEKS.label(old), "2w");
//! ```

mod ladder;
mod recency;

use std::fmt;

use ratatui::style::Style;
use ratatui::text::Span;

use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

pub use ladder::{Ladder, Unit};
pub use recency::{Recency, RecencyScale};

/// How long ago something happened, in whole seconds.
///
/// An age is never negative: [`Age::between`] takes a time in the future
/// as now. Format it with [`Age::label`] or a [`Ladder`], tint it with
/// [`Age::recency`] or a [`RecencyScale`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Age(u64);

impl Age {
    /// An age of `secs` seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs)
    }

    /// The age at `now` of something that happened at `then`, both Unix
    /// times in seconds. A `then` later than `now` (a clock that ran
    /// ahead) is an age of zero, so a fresh item never reads as
    /// negative.
    pub fn between(then: i64, now: i64) -> Self {
        Self(u64::try_from(now.saturating_sub(then)).unwrap_or(0))
    }

    /// [`Age::between`] when both times are known, `None` otherwise.
    pub fn of(then: Option<i64>, now: Option<i64>) -> Option<Self> {
        Some(Self::between(then?, now?))
    }

    /// The age in seconds.
    pub const fn secs(self) -> u64 {
        self.0
    }

    /// The age on the default ladder ([`Ladder::DAYS`]): `45s`, `17m`,
    /// `3h`, `12d`.
    pub fn label(self) -> String {
        Ladder::DAYS.label(self)
    }

    /// The age on the default ladder as prose: `5m ago`.
    pub fn ago(self) -> String {
        Ladder::DAYS.ago(self)
    }

    /// The age's step on the default scale ([`RecencyScale::default`]).
    pub fn recency(self) -> Recency {
        RecencyScale::default().recency(self)
    }
}

impl fmt::Display for Age {
    /// The age on the default ladder, as [`Age::label`] writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// The width of an age cell, in terminal cells: the widest label the
/// default ladder writes for an age under a thousand days (`999d`).
pub const CELL_WIDTH: usize = 4;

/// An age cell: `label` right-aligned in `width` cells (cut with the
/// ellipsis mark if it is wider), tinted by `recency`.
///
/// An unknown age (`label` is `None`) shows the no-value mark. An age
/// whose recency is unknown takes the theme's `muted` modifier instead
/// of a tint. The label is passed in rather than written here so a list
/// can format its ages once, when it builds its rows, and tint them when
/// it draws.
///
/// ```
/// use tuikit::age::{self, Age};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let age = Age::from_secs(90);
/// let cell = age::cell(Some(&age.label()), Some(age.recency()), age::CELL_WIDTH, &theme);
/// assert_eq!(cell.content, "  1m");
/// assert_eq!(cell.style, theme.recency_fresh.into_style());
/// assert_eq!(age::cell(None, None, 4, &theme).content, "   —");
/// ```
pub fn cell(
    label: Option<&str>,
    recency: Option<Recency>,
    width: usize,
    theme: &Theme,
) -> Span<'static> {
    let label = label.unwrap_or(marks::NO_VALUE);
    let style = recency.map_or_else(
        || Style::default().add_modifier(theme.muted),
        |recency| recency.style(theme),
    );
    Span::styled(
        text::pad_start(&text::truncate_end(label, width), width),
        style,
    )
}

#[cfg(test)]
#[path = "age_tests.rs"]
mod tests;
