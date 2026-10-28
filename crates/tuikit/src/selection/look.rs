//! The cursor row's look: the marker in its gutter, the highlight, and
//! the ghost and pending looks a row can wear under it.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::theme::Theme;

/// The cells the cursor gutter takes at the start of every row: the
/// pointer mark and a space on the cursor row, two spaces elsewhere.
pub const GUTTER_WIDTH: usize = 2;

/// Whether a list's cursor is on one row, and whether that list has
/// focus: which of the theme's selection pair the row takes.
///
/// ```
/// use tuikit::selection::Cursor;
///
/// assert_eq!(Cursor::of(false, true), Cursor::Off);
/// assert_eq!(Cursor::of(true, true), Cursor::Active);
/// assert_eq!(Cursor::of(true, false), Cursor::Inactive);
/// assert!(Cursor::Inactive.is_on());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cursor {
    /// The cursor is on another row.
    #[default]
    Off,
    /// The cursor row of the list that has focus: the theme's
    /// `selection_active`.
    Active,
    /// The cursor row of a list without focus: the theme's
    /// `selection_inactive`, and still the marker, so the list shows
    /// where its cursor is (house rule 10).
    Inactive,
}

impl Cursor {
    /// The cursor for a row: `on` when the cursor is on it, `focused`
    /// when its list has focus.
    pub const fn of(on: bool, focused: bool) -> Self {
        match (on, focused) {
            (false, _) => Self::Off,
            (true, true) => Self::Active,
            (true, false) => Self::Inactive,
        }
    }

    /// Whether the cursor is on the row, focused or not.
    pub const fn is_on(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// The modifiers the row takes: the theme's `selection_active` or
    /// `selection_inactive`, or none off the cursor.
    pub fn modifier(self, theme: &Theme) -> Modifier {
        match self {
            Self::Off => Modifier::empty(),
            Self::Active => theme.selection_active,
            Self::Inactive => theme.selection_inactive,
        }
    }

    /// [`Cursor::modifier`] as a style that adds those modifiers.
    pub fn style(self, theme: &Theme) -> Style {
        Style::default().add_modifier(self.modifier(theme))
    }
}

/// The cursor gutter a row starts with: the pointer mark and a space on
/// the cursor row, focused or not, and two spaces on every other row, so
/// the text after it stays in its column as the cursor moves. The span
/// is unstyled; [`row`] highlights it with the rest of the row.
pub fn gutter(cursor: Cursor) -> Span<'static> {
    if cursor.is_on() {
        Span::raw(format!("{} ", marks::POINTER))
    } else {
        Span::raw(" ".repeat(GUTTER_WIDTH))
    }
}

/// Whether `style` draws a filled span: a background, or the reversed
/// look a chip takes. Highlighting one would blend its fill into the
/// row's, so the highlight and the ghost look leave its fill alone.
fn is_filled(style: Style) -> bool {
    style.bg.is_some() || style.add_modifier.contains(Modifier::REVERSED)
}

/// Highlights a row: patches the cursor's selection modifiers onto every
/// span (house rule 1). A filled span (a chip, an input field's fill)
/// keeps the look it has on every other row, so it still reads as what
/// it is inside the highlight. Off the cursor nothing changes.
///
/// The highlight covers the spans a row is made of, the gutter included
/// when there is one; a list that wants it to run to the edge pads its
/// last span.
pub fn highlight(spans: &mut [Span<'_>], cursor: Cursor, theme: &Theme) {
    let style = cursor.style(theme);
    if style == Style::default() {
        return;
    }
    for span in spans.iter_mut().filter(|span| !is_filled(span.style)) {
        span.style = span.style.patch(style);
    }
}

/// A list row: the cursor [`gutter`], then `spans`, highlighted from the
/// gutter to the last span when the cursor is on it.
///
/// ```
/// use ratatui::style::{Color, Style};
/// use ratatui::text::Span;
/// use tuikit::selection::{self, Cursor};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let chip = Span::styled(" tag ", Style::default().bg(Color::Blue));
/// let line = selection::row([Span::raw("Item "), chip.clone()], Cursor::Active, &theme);
/// assert_eq!(line.spans.len(), 3);
/// assert_eq!(line.spans[1].style.add_modifier, theme.selection_active);
/// assert_eq!(line.spans[2], chip, "a filled span keeps its look");
/// ```
pub fn row<'a>(
    spans: impl IntoIterator<Item = Span<'a>>,
    cursor: Cursor,
    theme: &Theme,
) -> Line<'a> {
    let mut all: Vec<Span<'a>> = std::iter::once(gutter(cursor)).chain(spans).collect();
    highlight(&mut all, cursor, theme);
    Line::from(all)
}

/// The ghost look: the theme's `inert` color in italics, for a row or a
/// mark that stands for something gone, stopped, or not started yet.
pub fn ghost_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.inert)
        .add_modifier(Modifier::ITALIC)
}

/// Restyles a row's spans in the ghost look. Text takes the `inert`
/// color in italics and loses its bold; a filled span keeps its fill and
/// recedes instead (the `muted` modifier, in italics), so a chip on a
/// ghost row still reads as a chip. Apply it before [`highlight`].
pub fn ghost(spans: &mut [Span<'_>], theme: &Theme) {
    for span in spans {
        span.style = if is_filled(span.style) {
            span.style.add_modifier(theme.muted | Modifier::ITALIC)
        } else {
            span.style
                .fg(theme.inert)
                .remove_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC)
        };
    }
}

/// Restyles a row's spans in the pending look, for a row that keeps its
/// old values while it waits for a change to land: every span recedes
/// (the `muted` modifier) and keeps its colors. Apply it before
/// [`highlight`].
pub fn pending(spans: &mut [Span<'_>], theme: &Theme) {
    for span in spans {
        span.style = span.style.add_modifier(theme.muted);
    }
}
