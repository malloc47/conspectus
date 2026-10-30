//! The breadcrumb: the steps that led to what a pane shows, as one line
//! that gives way hop by hop.
//!
//! This is a layer 1 control. A pane that lets its reader follow a link
//! from one thing to the next shows how it got there: each hop a glyph in
//! the host's style and a short tag, joined by [`marks::SPACED_CRUMB`]
//! (`a › b › c`). Two hops with the same tag would read alike, so a hop's
//! suffix (`tag·suffix`) is shown only when another hop has its tag.
//!
//! When the chain is wider than its room it gives way by
//! [`text::elide`], with its mark ([`marks::ELLIPSIS`]) in the gap:
//!
//! - the last hop (where the reader came from) never gives way;
//! - the first hop (where the path began) outlasts every middle hop;
//! - the middle hops give way oldest first, and each run of them that
//!   gave way leaves one `…`;
//! - when even the last hop alone does not fit, the marks go and the hop
//!   is cut with `…` to the room.
//!
//! So a chain narrows as `a › b › c › d › e`, `a › … › c › d › e`,
//! `a › … › d › e`, `a › … › e`, `… › e`, `e`, and then `e` cut. The line
//! never takes more cells than its room.
//!
//! The host keeps what a hop is: which glyph and color stand for the kind
//! of thing it was, which tag names it, and which suffix tells two alike
//! apart. It also keeps the rest of the title the chain sits in.
//!
//! ```
//! use ratatui::style::{Color, Style};
//! use ratatui::text::Span;
//! use tuikit::breadcrumb::{Breadcrumb, Hop};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let glyph = |c: &'static str| Span::styled(c, Style::default().fg(Color::Cyan));
//! let hops = [
//!     Hop::new("start").glyph(glyph("*")).suffix("a1"),
//!     Hop::new("middle").glyph(glyph("#")).suffix("c3"),
//!     Hop::new("start").glyph(glyph("*")).suffix("b2"),
//! ];
//! let chain = Breadcrumb::new(&hops, &theme);
//! let text = |room| chain.line(room).map(|line| line.to_string());
//! // Only the two hops named alike show their suffix.
//! assert_eq!(text(80).as_deref(), Some("* start·a1 › # middle › * start·b2"));
//! assert_eq!(text(27).as_deref(), Some("* start·a1 › … › * start·b2"));
//! assert_eq!(text(14).as_deref(), Some("… › * start·b2"));
//! assert_eq!(text(6).as_deref(), Some("* sta…"));
//! assert!(Breadcrumb::new(&[], &theme).line(80).is_none());
//! ```

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::text::{self, Item, MarkAt, Piece};
use crate::theme::Theme;

/// One step of a breadcrumb: an optional glyph, a tag, and a suffix that
/// tells it from another hop with the same tag.
///
/// ```
/// use ratatui::text::Span;
/// use tuikit::breadcrumb::Hop;
///
/// let hop = Hop::new("index").glyph(Span::raw("#")).suffix("7f3a");
/// assert_eq!(hop.tag(), "index");
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hop {
    glyph: Option<Span<'static>>,
    tag: String,
    suffix: String,
}

impl Hop {
    /// A hop named `tag`, with no glyph and no suffix.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            glyph: None,
            tag: tag.into(),
            suffix: String::new(),
        }
    }

    /// Draw `glyph` before the tag, in the glyph's own style, with a
    /// space between. It is usually one cell: a vetted glyph for the kind
    /// of thing the hop was. Its width is measured, so a wider one fits
    /// too.
    pub fn glyph(mut self, glyph: Span<'static>) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Show `suffix` after the tag, joined by [`marks::SEPARATOR`]
    /// (`tag·suffix`), when another hop of the chain has the same tag. An
    /// empty suffix shows nothing.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// The hop's tag.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// The spans the hop draws: its glyph and a space, then its tag, with
    /// the suffix when `repeated`.
    fn spans(&self, repeated: bool, style: Style) -> Vec<Span<'static>> {
        let mut spans = Vec::with_capacity(3);
        if let Some(glyph) = &self.glyph {
            spans.push(glyph.clone());
            spans.push(Span::styled(" ", style));
        }
        let label = if repeated && !self.suffix.is_empty() {
            format!("{}{}{}", self.tag, marks::SEPARATOR, self.suffix)
        } else {
            self.tag.clone()
        };
        spans.push(Span::styled(label, style));
        spans
    }
}

/// A chain of hops drawn in a room: the oldest middle hops give way first,
/// and the last hop is cut only when it alone does not fit.
///
/// See the [module documentation](self) for the rule.
#[derive(Clone, Copy, Debug)]
pub struct Breadcrumb<'a> {
    hops: &'a [Hop],
    style: Style,
}

impl<'a> Breadcrumb<'a> {
    /// The chain of `hops`, oldest first, with its tags, separators, and
    /// marks in the theme's `inert` color.
    pub fn new(hops: &'a [Hop], theme: &Theme) -> Self {
        Self {
            hops,
            style: Style::default().fg(theme.inert),
        }
    }

    /// Draw the tags, the separators, and the marks in `style` instead.
    /// A hop's glyph keeps its own style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// The chain in at most `room` cells, or `None` when it has no hops.
    pub fn line(&self, room: usize) -> Option<Line<'static>> {
        if self.hops.is_empty() {
            return None;
        }
        let mut drawn = self.drawn();
        let pieces: Vec<Piece> = drawn
            .iter()
            .enumerate()
            .map(|(index, spans)| {
                let width = text::spans_width(spans);
                match rank(index, self.hops.len()) {
                    Some(rank) => Piece::ranked(width, rank),
                    None => Piece::unranked(width),
                }
            })
            .collect();
        let fit = text::elide(
            &pieces,
            text::width(marks::SPACED_CRUMB),
            text::width(marks::ELLIPSIS),
            room,
            MarkAt::Gap,
        );
        let last = fit.kept.last().copied();
        let mut spans = Vec::new();
        for (position, item) in fit.items().into_iter().enumerate() {
            if position > 0 {
                spans.push(Span::styled(marks::SPACED_CRUMB, self.style));
            }
            match item {
                Item::Mark => spans.push(Span::styled(marks::ELLIPSIS, self.style)),
                Item::Piece(index) => {
                    let hop = std::mem::take(&mut drawn[index]);
                    match fit.cut {
                        Some(cells) if Some(index) == last => spans.extend(self.cut(hop, cells)),
                        _ => spans.extend(hop),
                    }
                }
            }
        }
        Some(Line::from(spans))
    }

    /// Each hop's spans, the suffix shown where tags repeat.
    fn drawn(&self) -> Vec<Vec<Span<'static>>> {
        self.hops
            .iter()
            .map(|hop| hop.spans(self.repeated(hop), self.style))
            .collect()
    }

    /// Whether another hop of the chain has `hop`'s tag.
    fn repeated(&self, hop: &Hop) -> bool {
        self.hops
            .iter()
            .filter(|other| other.tag == hop.tag)
            .nth(1)
            .is_some()
    }

    /// `hop`'s spans cut to `cells`, the mark in the chain's style even
    /// when the cut falls in the glyph.
    fn cut(&self, hop: Vec<Span<'static>>, cells: usize) -> Vec<Span<'static>> {
        let mut cut = text::fit_spans(hop, cells);
        if let Some(mark) = cut.last_mut()
            && mark.content == marks::ELLIPSIS
        {
            mark.style = self.style;
        }
        cut
    }
}

/// When the hop at `index` of `len` gives way: the last never, the first
/// after every middle hop, the middle hops oldest first.
fn rank(index: usize, len: usize) -> Option<u32> {
    if index + 1 == len {
        None
    } else if index == 0 {
        Some(u32::try_from(len).unwrap_or(u32::MAX))
    } else {
        Some(u32::try_from(index).unwrap_or(u32::MAX))
    }
}

#[cfg(test)]
#[path = "breadcrumb_tests.rs"]
mod tests;
