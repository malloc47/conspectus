//! Chips: short labels set apart from the text around them.
//!
//! This is a layer 1 foundation. A dense list names the kind of thing on
//! each row in a filled pill, a footer shows which switches are on, a
//! title shows which of two choices is chosen, a reader shows how far
//! down it is, and a status line flags what is busy or broken. Each is a
//! chip, and each form has one rule here, so every chip of a form looks
//! and measures the same:
//!
//! - **[`Chip`]**: a filled pill of a fixed width. The label is cut (with
//!   the ellipsis mark) or padded to the theme's `badge_width` in
//!   terminal cells, inside one cell of padding on each side, all in one
//!   style, so the padding carries the fill and every chip in a column
//!   has the same footprint ([`width`]). The look is the theme's `badge`
//!   modifier over a color the caller picks, or a style the caller
//!   supplies.
//! - **[`ToggleChip`]**: a switch and its state, `name·state (key)`, its
//!   state padded to the widest it can show so the chip keeps its width
//!   as it changes; on in the `success` color, off receding.
//! - **[`SegmentedTitle`]**: a choice among a few words in a title, the
//!   chosen one in brackets and the chip look (`[ One ] Two`), so the
//!   title is as wide whichever is chosen.
//! - **[`PositionChip`]**: where a reader is, `[ n/m ]`.
//! - **[`StatusChip`]**: a short status in a [`Tone`] (muted, inert,
//!   accent, info, success, warning, error), optionally led by a mark or
//!   a busy spinner frame ([`spinner_frame`]), at its natural width.
//!
//! What a chip says (which label, which color for which kind of thing,
//! when a switch is on) is the host's; the chip only draws it.
//!
//! ```
//! use ratatui::style::Color;
//! use tuikit::chip::{self, Chip, PositionChip, StatusChip, Tone, ToggleChip};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let pill = Chip::new("alpha").color(Color::Magenta).span(&theme);
//! assert_eq!(pill.content, " alpha    ");
//! assert_eq!(chip::width(&theme), 10);
//! assert_eq!(Chip::new("a long name").span(&theme).content, " a long … ");
//!
//! let toggle = ToggleChip::new("wrap", "on", true).states(&["on", "off"]).key("w");
//! assert_eq!(toggle.text(), "wrap·on  (w)");
//! assert_eq!(PositionChip::new(4, 120).text(), "[ 5/120 ]");
//! assert_eq!(StatusChip::new("offline", Tone::Warning).span(&theme).content, "offline");
//! ```

mod position;
mod segmented;
mod status;
mod toggle;

use std::borrow::Cow;

use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::text;
use crate::theme::{MIN_BADGE_WIDTH, Theme};

pub use position::PositionChip;
pub use segmented::SegmentedTitle;
pub use status::{StatusChip, Tone, spinner_frame};
pub use toggle::ToggleChip;

/// The cells a [`Chip`]'s label takes: the theme's `badge_width`, never
/// less than [`MIN_BADGE_WIDTH`].
pub fn label_width(theme: &Theme) -> usize {
    theme.badge_width.max(MIN_BADGE_WIDTH)
}

/// The cells a [`Chip`] takes on screen: its label width and one cell of
/// padding on each side. A column that holds chips is this wide.
pub fn width(theme: &Theme) -> usize {
    label_width(theme) + 2
}

/// How a chip is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    /// The theme's `badge` modifier, over this color when there is one.
    Badge(Option<Color>),
    /// A style the caller supplied, used as it is.
    Style(Style),
}

/// A fixed-width filled chip: a label cut or padded to a width in cells,
/// inside one cell of padding on each side, drawn as one styled span.
///
/// By default the chip takes the theme's `badge` look (reversed and bold,
/// a filled pill on any terminal) in the terminal's own color;
/// [`Chip::color`] picks the fill's color and [`Chip::style`] replaces the
/// look with a style of the caller's (a background fill, or a muted text
/// chip that keeps the column). The label width is the theme's
/// `badge_width` unless [`Chip::label_width`] sets one.
///
/// ```
/// use ratatui::layout::Alignment;
/// use ratatui::style::{Color, Modifier, Style};
/// use tuikit::chip::Chip;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let filled = Chip::new("beta").color(Color::Cyan).span(&theme);
/// assert_eq!(filled.content, " beta     ");
/// assert_eq!(filled.style.fg, Some(Color::Cyan));
/// assert!(filled.style.add_modifier.contains(Modifier::REVERSED));
///
/// let boxed = Chip::new("job")
///     .style(Style::default().fg(Color::White).bg(Color::Black))
///     .label_width(5)
///     .align(Alignment::Center)
///     .span(&theme);
/// assert_eq!(boxed.content, "  job  ");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip<'a> {
    label: Cow<'a, str>,
    fill: Fill,
    label_width: Option<usize>,
    align: Alignment,
}

impl<'a> Chip<'a> {
    /// A chip showing `label` in the theme's chip look, left-aligned.
    pub fn new(label: impl Into<Cow<'a, str>>) -> Self {
        Self {
            label: label.into(),
            fill: Fill::Badge(None),
            label_width: None,
            align: Alignment::Left,
        }
    }

    /// Fills the chip with `color` under the theme's `badge` modifier (a
    /// reversed foreground, so the color is the fill).
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.fill = Fill::Badge(Some(color));
        self
    }

    /// Draws the chip in `style` instead of the theme's chip look. The
    /// width rule still holds, so a chip in a style of its own lines up
    /// with filled ones.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.fill = Fill::Style(style);
        self
    }

    /// Sets the label's width in cells, overriding the theme's
    /// `badge_width`; never less than [`MIN_BADGE_WIDTH`].
    #[must_use]
    pub fn label_width(mut self, cells: usize) -> Self {
        self.label_width = Some(cells);
        self
    }

    /// Where a label shorter than the width sits: at the start (the
    /// default), centered, or at the end.
    #[must_use]
    pub fn align(mut self, align: Alignment) -> Self {
        self.align = align;
        self
    }

    /// The label, as given.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The style the chip draws in under `theme`.
    pub fn look(&self, theme: &Theme) -> Style {
        match self.fill {
            Fill::Badge(color) => {
                let style = Style::default().add_modifier(theme.badge);
                color.map_or(style, |color| style.fg(color))
            }
            Fill::Style(style) => style,
        }
    }

    /// The chip's text: the label cut with the ellipsis mark or padded to
    /// the label width in cells, inside one space on each side. Always
    /// exactly the label width plus 2 cells wide.
    pub fn text(&self, theme: &Theme) -> String {
        let cells = self
            .label_width
            .map_or_else(|| label_width(theme), |cells| cells.max(MIN_BADGE_WIDTH));
        let cut = text::truncate_end(&self.label, cells);
        let gap = cells.saturating_sub(text::width(&cut));
        let before = match self.align {
            Alignment::Left => 0,
            Alignment::Center => gap / 2,
            Alignment::Right => gap,
        };
        format!(" {}{cut}{} ", " ".repeat(before), " ".repeat(gap - before))
    }

    /// The chip as one styled span, so the padding carries the fill.
    pub fn span(&self, theme: &Theme) -> Span<'static> {
        Span::styled(self.text(theme), self.look(theme))
    }
}

#[cfg(test)]
#[path = "chip_tests.rs"]
mod tests;
