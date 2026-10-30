//! The priority line: a line of segments, each with a priority, that drop
//! in order to fit a width.
//!
//! This is a layer 1 control. A summary line or a status line shows
//! several short things at once (counts, settings, hints, status chips),
//! each worth showing for itself, in an order that reads well rather than
//! in order of importance. On a narrow screen the least important have to
//! go first, wherever they stand, and the line has to say that something
//! went instead of being cut mid-word at the edge.
//!
//! Each [`Segment`] is a few styled spans with a rank (its priority) or
//! none. The line gives way by [`text::elide_filled`], with its mark at
//! the end:
//!
//! - every segment shows while they all fit;
//! - otherwise segments give way lowest rank first (of two with one rank,
//!   the later first), and the line ends with [`marks::MORE`] (` …`);
//! - then the room is filled back: a segment that gave way returns,
//!   highest rank first, when the line still fits with it, so a narrow
//!   segment of low rank shows where a wide one of higher rank did not
//!   fit (segments of one rank return in order, the earlier first);
//! - a segment with no rank never gives way; when those alone do not fit,
//!   the mark goes and the last of them is cut with `…`.
//!
//! Segments are joined by the line's separator ([`marks::SPACED_SEPARATOR`]
//! by default); a segment may have a gap of its own drawn before it
//! instead (a wider one before a group of chips, a narrower one inside
//! it). The line never takes more cells than its room.
//!
//! The host keeps what the segments say and how important each is: their
//! words, their chips' meanings, and their ranks. Parts of a line that
//! must never move (a lead that names the focus, chips that stay at the
//! end) are spans the host draws around the line, giving the line the
//! cells they leave.
//!
//! ```
//! use ratatui::style::{Modifier, Style};
//! use ratatui::text::Span;
//! use tuikit::chip::{StatusChip, Tone};
//! use tuikit::glyphs::marks;
//! use tuikit::priority_line::{PriorityLine, Segment};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let bold = Style::default().add_modifier(Modifier::BOLD);
//! let line = PriorityLine::new(&theme)
//!     .separator(Span::styled(" · ", bold))
//!     .segment(Segment::new(Span::styled("synced 2m ago", bold)).rank(3))
//!     .segment(Segment::new(Span::styled("12 items", bold)))
//!     .segment(Segment::new(Span::styled("3 shared", bold)).rank(2))
//!     .segment(
//!         Segment::new(vec![
//!             StatusChip::new(marks::WARNING, Tone::Warning).bold().span(&theme),
//!             Span::raw(" 2"),
//!         ])
//!         .rank(1)
//!         .gap("  ·  "),
//!     );
//! let text = |room| line.line(room).to_string();
//! assert_eq!(text(80), "synced 2m ago · 12 items · 3 shared  ·  ▲ 2");
//! assert_eq!(text(40), "synced 2m ago · 12 items · 3 shared …");
//! assert_eq!(text(30), "synced 2m ago · 12 items …");
//! assert_eq!(text(21), "12 items · 3 shared …");
//! // `3 shared` no longer fits beside `12 items`, but the warning does.
//! assert_eq!(text(20), "12 items  ·  ▲ 2 …");
//! assert_eq!(text(12), "12 items …");
//! assert_eq!(text(5), "12 i…");
//! ```

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::text::{self, Elided, Item, MarkAt, Piece};
use crate::theme::Theme;

/// One segment of a [`PriorityLine`]: a few styled spans, a rank, and an
/// optional gap of its own.
///
/// ```
/// use ratatui::text::Span;
/// use tuikit::priority_line::Segment;
///
/// let quit = Segment::new("q quit").rank(8);
/// assert_eq!((quit.width(), quit.rank_of()), (6, Some(8)));
/// let count = Segment::new(vec![Span::raw("3"), Span::raw(" left")]);
/// assert_eq!((count.width(), count.rank_of()), (6, None));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segment {
    spans: Vec<Span<'static>>,
    rank: Option<u32>,
    gap: Option<Span<'static>>,
}

impl Segment {
    /// A segment drawing `spans` (a span, a string, or a list of spans),
    /// which never gives way until [`rank`](Self::rank) gives it one.
    pub fn new(spans: impl Into<Line<'static>>) -> Self {
        Self {
            spans: spans.into().spans,
            rank: None,
            gap: None,
        }
    }

    /// Give way at `rank`: the lower the rank, the sooner the segment
    /// goes when the line is too wide; of two with one rank, the later
    /// goes first.
    #[must_use]
    pub fn rank(mut self, rank: u32) -> Self {
        self.rank = Some(rank);
        self
    }

    /// Draw `gap` before the segment, in place of the line's separator,
    /// when another item precedes it.
    #[must_use]
    pub fn gap(mut self, gap: impl Into<Span<'static>>) -> Self {
        self.gap = Some(gap.into());
        self
    }

    /// The segment's rank, `None` when it never gives way.
    pub fn rank_of(&self) -> Option<u32> {
        self.rank
    }

    /// The cells the segment's spans take.
    pub fn width(&self) -> usize {
        text::spans_width(&self.spans)
    }

    /// The segment as a piece of the fit rule.
    fn piece(&self) -> Piece {
        let piece = match self.rank {
            Some(rank) => Piece::ranked(self.width(), rank),
            None => Piece::unranked(self.width()),
        };
        match &self.gap {
            Some(gap) => piece.gap(gap.width()),
            None => piece,
        }
    }
}

/// A line of [`Segment`]s that give way by rank to fit a room, ending in
/// a mark when some gave way.
///
/// See the [module documentation](self) for the rule.
#[derive(Clone, Debug, PartialEq)]
pub struct PriorityLine {
    segments: Vec<Segment>,
    separator: Span<'static>,
    mark: Span<'static>,
}

impl PriorityLine {
    /// An empty line whose segments are joined by
    /// [`marks::SPACED_SEPARATOR`] and whose mark is [`marks::MORE`], both
    /// in the theme's `muted` look.
    pub fn new(theme: &Theme) -> Self {
        let muted = Style::default().add_modifier(theme.muted);
        Self {
            segments: Vec::new(),
            separator: Span::styled(marks::SPACED_SEPARATOR, muted),
            mark: Span::styled(marks::MORE, muted),
        }
    }

    /// Join the segments with `separator` instead.
    #[must_use]
    pub fn separator(mut self, separator: impl Into<Span<'static>>) -> Self {
        self.separator = separator.into();
        self
    }

    /// End a line that left segments out with `mark` instead, spacing
    /// included.
    #[must_use]
    pub fn mark(mut self, mark: impl Into<Span<'static>>) -> Self {
        self.mark = mark.into();
        self
    }

    /// The line with `segment` after the others.
    #[must_use]
    pub fn segment(mut self, segment: Segment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Add `segment` after the others.
    pub fn push(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    /// The segments, in order.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether the line has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Which segments show in `room` cells: the kept segments by index,
    /// whether the line ends with its mark, and the cells the last kept
    /// segment is cut to when the segments that never give way do not
    /// fit. A host that maps a click on the line to a segment reads the
    /// kept segments' places from this.
    pub fn fit(&self, room: usize) -> Elided {
        let pieces: Vec<Piece> = self.segments.iter().map(Segment::piece).collect();
        text::elide_filled(
            &pieces,
            self.separator.width(),
            self.mark.width(),
            room,
            MarkAt::End,
        )
    }

    /// The line's spans in at most `room` cells.
    pub fn spans(&self, room: usize) -> Vec<Span<'static>> {
        let fit = self.fit(room);
        let last = fit.kept.last().copied();
        let mut spans = Vec::new();
        for (position, item) in fit.items().into_iter().enumerate() {
            // An end mark is never an item, so every item is a segment.
            let Item::Piece(index) = item else { continue };
            let segment = &self.segments[index];
            if position > 0 {
                spans.push(
                    segment
                        .gap
                        .clone()
                        .unwrap_or_else(|| self.separator.clone()),
                );
            }
            match fit.cut {
                Some(cells) if Some(index) == last => {
                    spans.extend(text::fit_spans(segment.spans.clone(), cells));
                }
                _ => spans.extend(segment.spans.iter().cloned()),
            }
        }
        if fit.end_mark {
            spans.push(self.mark.clone());
        }
        spans
    }

    /// The line in at most `room` cells; an empty line with no segments.
    pub fn line(&self, room: usize) -> Line<'static> {
        Line::from(self.spans(room))
    }
}

#[cfg(test)]
#[path = "priority_line_tests.rs"]
mod tests;
