//! A short status in a tone: what is off, busy, broken, or worth a note.

use std::borrow::Cow;
use std::time::Duration;

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::glyphs::marks;
use crate::theme::Theme;

/// The tone of a [`StatusChip`] (and of an off [`ToggleChip`](super::ToggleChip)):
/// which theme token it draws in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    /// Recedes: the theme's `muted` modifier over the text color. For
    /// something switched off on purpose.
    Muted,
    /// Recedes further: the `inert` color, dimmed by `muted`. For a note
    /// about the item beside it (a count, where it was) and switches that
    /// are off.
    Inert,
    /// Draws the eye: the `accent` color. For work in progress.
    Accent,
    /// The `success` color: something is on or worked.
    Success,
    /// The `warning` color: something needs attention.
    Warning,
    /// The `error` color: something failed.
    Error,
}

impl Tone {
    /// The tone's style under `theme`.
    pub fn style(self, theme: &Theme) -> Style {
        let style = Style::default();
        match self {
            Self::Muted => style.add_modifier(theme.muted),
            Self::Inert => style.fg(theme.inert).add_modifier(theme.muted),
            Self::Accent => style.fg(theme.accent),
            Self::Success => style.fg(theme.success),
            Self::Warning => style.fg(theme.warning),
            Self::Error => style.fg(theme.error),
        }
    }
}

/// How long each frame of the busy spinner shows.
const SPINNER_FRAME: Duration = Duration::from_millis(120);

/// The spinner frame to draw for work that has run for `elapsed`: one of
/// [`marks::SPINNER`], advancing every 120 milliseconds and starting over
/// after the last. Deriving the frame from the elapsed time keeps the
/// spinner free of state; a host redraws often enough to animate it.
///
/// ```
/// use std::time::Duration;
/// use tuikit::chip::spinner_frame;
/// use tuikit::glyphs::marks;
///
/// assert_eq!(spinner_frame(Duration::ZERO), marks::SPINNER[0]);
/// assert_eq!(spinner_frame(Duration::from_millis(250)), marks::SPINNER[2]);
/// ```
pub fn spinner_frame(elapsed: Duration) -> &'static str {
    let step = elapsed.as_millis() / SPINNER_FRAME.as_millis();
    let index = usize::try_from(step % marks::SPINNER.len() as u128).unwrap_or(0);
    marks::SPINNER[index]
}

/// A short status at its natural width, in a [`Tone`]: `offline`,
/// `▲ 2 unread`, a spinner frame and `saving`.
///
/// A mark or a spinner frame may lead the label, one space before it.
/// [`StatusChip::bold`] adds emphasis. [`StatusChip::pad`] adds blank
/// cells before and after the label in the chip's own style, for a chip
/// that keeps its gap to its neighbors as part of itself.
///
/// ```
/// use std::time::Duration;
/// use tuikit::chip::{StatusChip, Tone};
/// use tuikit::glyphs::marks;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let unread = StatusChip::new("2 unread", Tone::Error).mark(marks::WARNING).bold();
/// assert_eq!(unread.text(), "▲ 2 unread");
/// let busy = StatusChip::busy("saving", Duration::ZERO);
/// assert_eq!(busy.text(), format!("{} saving", marks::SPINNER[0]));
/// let note = StatusChip::new("3 more", Tone::Inert).pad(2, 0).span(&theme);
/// assert_eq!(note.content, "  3 more");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusChip<'a> {
    label: Cow<'a, str>,
    tone: Tone,
    mark: Option<&'static str>,
    bold: bool,
    before: usize,
    after: usize,
}

impl<'a> StatusChip<'a> {
    /// A chip showing `label` in `tone`.
    pub fn new(label: impl Into<Cow<'a, str>>, tone: Tone) -> Self {
        Self {
            label: label.into(),
            tone,
            mark: None,
            bold: false,
            before: 0,
            after: 0,
        }
    }

    /// A chip for work in progress: the spinner frame for `elapsed`
    /// ([`spinner_frame`]) and `label`, bold, in the accent tone.
    pub fn busy(label: impl Into<Cow<'a, str>>, elapsed: Duration) -> Self {
        Self::new(label, Tone::Accent)
            .mark(spinner_frame(elapsed))
            .bold()
    }

    /// Leads the label with `mark` (a glyph from [`marks`]) and a space.
    #[must_use]
    pub fn mark(mut self, mark: &'static str) -> Self {
        self.mark = Some(mark);
        self
    }

    /// Draws the chip bold.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Adds `before` blank cells before the chip and `after` after it, in
    /// the chip's style.
    #[must_use]
    pub fn pad(mut self, before: usize, after: usize) -> Self {
        self.before = before;
        self.after = after;
        self
    }

    /// The chip's tone.
    pub fn tone(&self) -> Tone {
        self.tone
    }

    /// The chip's text: the padding, the mark and a space, and the label.
    pub fn text(&self) -> String {
        let mark = self.mark.map(|mark| format!("{mark} ")).unwrap_or_default();
        format!(
            "{}{mark}{}{}",
            " ".repeat(self.before),
            self.label,
            " ".repeat(self.after)
        )
    }

    /// The style the chip draws in: its tone, bold when asked.
    pub fn look(&self, theme: &Theme) -> Style {
        let style = self.tone.style(theme);
        if self.bold {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    /// The chip as one styled span.
    pub fn span(&self, theme: &Theme) -> Span<'static> {
        Span::styled(self.text(), self.look(theme))
    }
}
