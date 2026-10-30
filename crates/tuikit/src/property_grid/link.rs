//! The link row and its tones: where a link leads, and how much it holds.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::MIN_LINK_LABEL;
use super::grid::{Intent, Layout};
use crate::glyphs::marks;
use crate::selection::GUTTER_WIDTH;
use crate::text;
use crate::theme::Theme;

/// The cells an alert row's lead takes: the warning mark and a space.
const ALERT_LEAD: usize = 2;

/// How much a link holds, and so how its row looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    /// A link that holds: the one the program settled on. The label and
    /// meta in the theme's `link` color, bold.
    #[default]
    Primary,
    /// An alternative shown for context: the label and meta in the
    /// `inert` color.
    Quiet,
    /// A link that competes with another or conflicts: the row is led by
    /// [`marks::WARNING`], and the mark, the label, and the meta are in
    /// the `warning` color, bold. The mark is the one cue that survives a
    /// terminal with no color.
    Alert,
    /// A link with no far end the program knows: the whole row recedes
    /// (the `muted` look), and following it leads nowhere.
    Inert,
}

impl Tone {
    /// Every tone, in the order the variants are declared.
    pub const ALL: [Tone; 4] = [Tone::Primary, Tone::Quiet, Tone::Alert, Tone::Inert];

    /// The tone's look from the theme's tokens alone.
    pub fn style(self, theme: &Theme) -> Style {
        let style = Style::default();
        match self {
            Self::Primary => style.fg(theme.link).add_modifier(Modifier::BOLD),
            Self::Quiet => style.fg(theme.inert),
            Self::Alert => style.fg(theme.warning).add_modifier(Modifier::BOLD),
            Self::Inert => style.add_modifier(theme.muted),
        }
    }
}

/// The look of each [`Tone`]: from the theme's tokens ([`Tones::new`]),
/// with any of them replaced by a role the host keeps for links of that
/// kind ([`Tones::with`]).
///
/// ```
/// use ratatui::style::{Color, Style};
/// use tuikit::property_grid::{Tone, Tones};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let tones = Tones::new(&theme).with(Tone::Quiet, Style::default().fg(Color::Gray));
/// assert_eq!(tones.style(Tone::Quiet).fg, Some(Color::Gray));
/// assert_eq!(tones.style(Tone::Primary), Tone::Primary.style(&theme));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tones {
    primary: Style,
    quiet: Style,
    alert: Style,
    inert: Style,
}

impl Tones {
    /// Every tone in its look from `theme`'s tokens.
    pub fn new(theme: &Theme) -> Self {
        Self {
            primary: Tone::Primary.style(theme),
            quiet: Tone::Quiet.style(theme),
            alert: Tone::Alert.style(theme),
            inert: Tone::Inert.style(theme),
        }
    }

    /// `tone` drawn in `style` instead.
    pub fn with(mut self, tone: Tone, style: Style) -> Self {
        *self.slot(tone) = style;
        self
    }

    /// The look of `tone`.
    pub fn style(&self, tone: Tone) -> Style {
        match tone {
            Tone::Primary => self.primary,
            Tone::Quiet => self.quiet,
            Tone::Alert => self.alert,
            Tone::Inert => self.inert,
        }
    }

    fn slot(&mut self, tone: Tone) -> &mut Style {
        match tone {
            Tone::Primary => &mut self.primary,
            Tone::Quiet => &mut self.quiet,
            Tone::Alert => &mut self.alert,
            Tone::Inert => &mut self.inert,
        }
    }
}

/// A link from the thing a grid shows to another: an indent, a verb in
/// the grid's verb column, a one-cell glyph, the far end's label, and
/// optional meta, in a [`Tone`].
///
/// The row is laid out in the width the grid is drawn at: the label is
/// cut in the middle to the room the indent, the verb column, and the
/// glyph leave, so both of its ends stay readable and the row never
/// wraps. Meta follows the label (after two blank cells and the
/// separator mark), and the label leaves room for it while that keeps
/// [`MIN_LINK_LABEL`] cells of the label; with less room the label takes
/// the room and the meta runs on past the width, where a host that wraps
/// its lines carries it to the next row, so meta a host asked for is
/// never lost. A row with no glyph draws the no-value mark in its place.
///
/// ```
/// use ratatui::text::Span;
/// use tuikit::property_grid::{Intent, LinkRow, Tone};
///
/// let link = LinkRow::new("stored in", "/srv/cache").glyph(Span::raw("#"));
/// assert_eq!(link.activate(), Some(Intent::Follow));
/// let nowhere = LinkRow::new("stored in", "unknown").tone(Tone::Inert);
/// assert_eq!(nowhere.activate(), None, "an inert link leads nowhere");
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkRow {
    indent: usize,
    verb: String,
    glyph: Option<Span<'static>>,
    label: String,
    meta: Option<String>,
    tone: Tone,
}

impl LinkRow {
    /// A link that `verb` names, to the thing called `label`, in the
    /// primary tone with no indent.
    pub fn new(verb: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            label: label.into(),
            ..Self::default()
        }
    }

    /// `cells` blank cells between the cursor gutter and the verb. An
    /// alert row's warning mark and a space take the first two of them
    /// (an indent under two cells grows to two).
    pub fn indent(mut self, cells: usize) -> Self {
        self.indent = cells;
        self
    }

    /// The glyph before the label, in the style it carries: what kind of
    /// thing the far end is.
    pub fn glyph(mut self, glyph: Span<'static>) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Detail after the label, in the row's tone, that runs past the row's
    /// width rather than cut the label under [`MIN_LINK_LABEL`] cells.
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// How much the link holds.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The verb.
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// The far end's label, whole.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// What `Enter` on the row asks for: following the link, or nothing
    /// for an inert link, which has no far end.
    pub fn activate(&self) -> Option<Intent> {
        (self.tone != Tone::Inert).then_some(Intent::Follow)
    }

    /// The row's spans after the cursor gutter, laid out in `width`
    /// cells counted from the row's start (the gutter included).
    pub(super) fn draw(&self, layout: &Layout<'_>, width: usize) -> Vec<Span<'static>> {
        let style = layout.tones.style(self.tone);
        // An inert row recedes whole; on the others only the far end
        // takes the tone, so the verbs read alike down the list.
        let plain = if self.tone == Tone::Inert {
            style
        } else {
            Style::default()
        };
        let mut spans = Vec::new();
        let mut indent = self.indent;
        if self.tone == Tone::Alert {
            spans.push(Span::styled(format!("{} ", marks::WARNING), style));
            indent = indent.saturating_sub(ALERT_LEAD);
        }
        spans.push(Span::styled(
            format!(
                "{}{} ",
                " ".repeat(indent),
                text::pad(&self.verb, layout.verb_width)
            ),
            plain,
        ));
        let glyph = self
            .glyph
            .clone()
            .unwrap_or_else(|| Span::styled(marks::NO_VALUE, style));
        let fixed = GUTTER_WIDTH + text::spans_width(&spans) + text::width(&glyph.content) + 1;
        spans.push(glyph);
        spans.push(Span::styled(" ", plain));
        let meta = self
            .meta
            .as_ref()
            .map(|meta| format!("  {} {meta}", marks::SEPARATOR));
        let room = width.saturating_sub(fixed);
        let beside_meta = room.saturating_sub(meta.as_deref().map_or(0, text::width));
        let budget = if beside_meta >= MIN_LINK_LABEL {
            beside_meta
        } else {
            room.max(1)
        };
        spans.push(Span::styled(
            text::truncate_middle(&self.label, budget),
            style,
        ));
        if let Some(meta) = meta {
            spans.push(Span::styled(meta, style));
        }
        spans
    }
}
