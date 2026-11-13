//! The one fit rule for the pieces of a line: which pieces stay when a
//! line is too wide for its room, and where the mark for the rest goes.
//!
//! A path of steps in a title, a strip of tab names, and a status line of
//! hints all hold pieces that can give way one by one, and each says that
//! something went with a mark. [`elide`] is the rule they share. The
//! caller measures each piece in cells and ranks it; the rule says which
//! pieces to keep, where the marks stand, and, when even the pieces that
//! never give way do not fit, how many cells the last of them may take.
//! The caller draws the line from that. [`elide_filled`] is the same rule
//! with a second pass that fills the room back, for a line whose pieces
//! stand alone.

use std::cmp::Reverse;

/// One piece of a line, measured in cells, and when it gives way.
///
/// ```
/// use tuikit::text::Piece;
///
/// assert_eq!(Piece::ranked(5, 2), Piece { width: 5, rank: Some(2), gap: None });
/// assert_eq!(Piece::unranked(5), Piece { width: 5, rank: None, gap: None });
/// assert_eq!(Piece::ranked(5, 2).gap(1).gap, Some(1));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// The cells the piece takes.
    pub width: usize,
    /// `None` keeps the piece whatever the room; otherwise pieces give
    /// way lowest rank first, and of two pieces with one rank, the later
    /// one first.
    pub rank: Option<u32>,
    /// The cells before the piece when an item precedes it, in place of
    /// the line's separator; `None` takes the separator. The first item
    /// of a line has nothing before it.
    pub gap: Option<usize>,
}

impl Piece {
    /// A piece `width` cells wide that gives way at `rank`.
    pub const fn ranked(width: usize, rank: u32) -> Self {
        Self {
            width,
            rank: Some(rank),
            gap: None,
        }
    }

    /// A piece `width` cells wide that never gives way. When nothing else
    /// can go, the last such piece is cut.
    pub const fn unranked(width: usize) -> Self {
        Self {
            width,
            rank: None,
            gap: None,
        }
    }

    /// The piece with `cells` before it in place of the line's separator,
    /// for a line whose pieces are not all joined alike (a wider gap
    /// before a group of pieces, a narrower one inside it).
    #[must_use]
    pub const fn gap(mut self, cells: usize) -> Self {
        self.gap = Some(cells);
        self
    }
}

/// Where the mark for pieces that gave way goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkAt {
    /// In the gap they left, as an item between separators: one mark for
    /// each run of neighboring pieces that gave way (a path of steps).
    Gap,
    /// Once, at the end of the line, after the last piece and with no
    /// separator before it (a strip of names, a status line).
    End,
}

/// What [`elide`] keeps of a line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Elided {
    /// The pieces kept, by index into the pieces, in order.
    pub kept: Vec<usize>,
    /// Positions in [`kept`](Self::kept) before which a mark stands
    /// ([`MarkAt::Gap`]), in order; `kept.len()` is a mark after the last
    /// kept piece.
    pub marks: Vec<usize>,
    /// Whether a mark ends the line ([`MarkAt::End`]).
    pub end_mark: bool,
    /// When even the pieces that never give way do not fit: the cells the
    /// last kept piece may take, for the caller to cut it to with
    /// [`truncate_end`](super::truncate_end) or [`fit_spans`](super::fit_spans).
    /// The marks are dropped then.
    pub cut: Option<usize>,
}

/// One item of an elided line: a kept piece or a mark standing in a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// The piece at this index into the pieces.
    Piece(usize),
    /// A mark for a run of pieces that gave way.
    Mark,
}

impl Elided {
    /// The items of the line in order, the kept pieces and the marks in
    /// their gaps; a separator goes between each two (a piece's own gap
    /// before a piece that has one). The end mark
    /// ([`end_mark`](Self::end_mark)) is not an item: it follows the last
    /// one with no separator.
    ///
    /// ```
    /// use tuikit::text::{Item, MarkAt, Piece, elide};
    ///
    /// let pieces = [Piece::ranked(4, 3), Piece::ranked(4, 1), Piece::unranked(4)];
    /// // `aaaa › … › cccc` is 15 cells.
    /// let line = elide(&pieces, 3, 1, 15, MarkAt::Gap);
    /// assert_eq!(line.items(), [Item::Piece(0), Item::Mark, Item::Piece(2)]);
    /// ```
    pub fn items(&self) -> Vec<Item> {
        let mut marks = self.marks.iter().copied().peekable();
        let mut out = Vec::with_capacity(self.kept.len() + self.marks.len());
        for (position, &index) in self.kept.iter().enumerate() {
            if marks.next_if_eq(&position).is_some() {
                out.push(Item::Mark);
            }
            out.push(Item::Piece(index));
        }
        if marks.next_if_eq(&self.kept.len()).is_some() {
            out.push(Item::Mark);
        }
        out
    }

    /// The cells the line takes when drawn from `pieces` with `separator`
    /// cells between each two items (a piece's own [`gap`](Piece::gap)
    /// before a piece that has one) and `mark` cells for each mark, the
    /// cut piece counted at the cells it may take.
    pub fn width(&self, pieces: &[Piece], separator: usize, mark: usize) -> usize {
        let last = self.kept.last().copied();
        let mut cells = 0;
        for (position, item) in self.items().into_iter().enumerate() {
            let (own, before) = match item {
                Item::Piece(index) => {
                    let piece = pieces[index];
                    let own = match self.cut {
                        Some(cut) if Some(index) == last => cut,
                        _ => piece.width,
                    };
                    (own, piece.gap.unwrap_or(separator))
                }
                Item::Mark => (mark, separator),
            };
            cells += own;
            if position > 0 {
                cells += before;
            }
        }
        cells + if self.end_mark { mark } else { 0 }
    }
}

/// Fit `pieces` into `room` cells: keep what fits by rank and mark what
/// gave way.
///
/// `separator` is the cells between each two items of the line (a piece
/// with a [`gap`](Piece::gap) of its own has that before it instead), and
/// `mark` the cells of one mark (with any spacing it carries, for
/// [`MarkAt::End`]). Widths are terminal cells (measure them with
/// [`width`](super::width) or [`spans_width`](super::spans_width)).
///
/// The rule: start from every piece. While the line (the kept pieces, a
/// separator between each two items, and a mark for each run that gave
/// way at [`MarkAt::Gap`], or one at the end at [`MarkAt::End`]) is wider
/// than the room and a piece with a rank is still kept, the
/// lowest-ranked one gives way (of two with one rank, the later). When
/// only unranked pieces are left and the line still does not fit, the
/// marks go, and if the pieces are still too wide, the last of them is
/// cut to the cells the others leave ([`Elided::cut`]).
///
/// The line then fits the room, unless more than one piece is unranked and
/// those before the last already fill it.
///
/// ```
/// use tuikit::text::{MarkAt, Piece, elide};
///
/// // A strip of names: the first never gives way, the rest go from the
/// // end, and a two-cell mark (` …`) ends a shortened strip.
/// let names = [Piece::unranked(5), Piece::ranked(5, 2), Piece::ranked(5, 1)];
/// let strip = elide(&names, 3, 2, 15, MarkAt::End);
/// assert_eq!(strip.kept, [0, 1]);
/// assert!(strip.end_mark);
/// assert_eq!(strip.width(&names, 3, 2), 15);
///
/// // Too narrow for the one name that stays: it is cut, with no mark.
/// let strip = elide(&names, 3, 2, 3, MarkAt::End);
/// assert_eq!((strip.kept, strip.end_mark, strip.cut), (vec![0], false, Some(3)));
/// ```
pub fn elide(pieces: &[Piece], separator: usize, mark: usize, room: usize, at: MarkAt) -> Elided {
    let mut shown = vec![true; pieces.len()];
    let mut order: Vec<usize> = (0..pieces.len())
        .filter(|&index| pieces[index].rank.is_some())
        .collect();
    order.sort_by_key(|&index| (pieces[index].rank, Reverse(index)));
    let mut order = order.into_iter();
    loop {
        let line = layout(&shown, at);
        if line.width(pieces, separator, mark) <= room {
            return line;
        }
        match order.next() {
            Some(index) => shown[index] = false,
            None => break,
        }
    }
    // Only unranked pieces are left: the marks go, then the last is cut.
    let kept: Vec<usize> = (0..pieces.len()).filter(|&index| shown[index]).collect();
    let mut line = Elided {
        kept,
        ..Elided::default()
    };
    let width = line.width(pieces, separator, mark);
    if let Some(&last) = line.kept.last()
        && width > room
    {
        let others = width - pieces[last].width;
        line.cut = Some(room.saturating_sub(others));
    }
    line
}

/// Fit `pieces` into `room` cells by [`elide`], then fill the room back.
///
/// `elide` gives way strictly by rank, so once a wide piece has gone, a
/// narrower one of lower rank that would fit beside the kept pieces has
/// gone too. A line whose pieces stand alone, each worth showing for
/// itself whatever else shows (the hints and settings of a status line),
/// wants every cell used: after the rule, each piece that gave way comes
/// back while the line still fits with it, highest rank first. Pieces of
/// one rank come back in order, the earlier first, and once one of them
/// does not fit the later ones stay out, so the pieces kept of each rank
/// are always its first ones, as `elide` leaves them. The marks follow
/// what is kept.
///
/// A line whose pieces read in sequence (the steps of a path, a strip of
/// names) uses `elide` alone: filling would show a later piece after a
/// gap the mark does not place.
///
/// When `elide` had to drop its marks or cut the last piece, nothing
/// comes back.
///
/// ```
/// use tuikit::text::{MarkAt, Piece, elide, elide_filled};
///
/// // A wide piece that matters more, and a narrow one that matters less.
/// let pieces = [Piece::unranked(4), Piece::ranked(10, 2), Piece::ranked(2, 1)];
/// // Strictly, the narrow piece gives way first and then the wide one,
/// // which leaves `aaaa …`; the narrow one fits back: `aaaa · cc …`.
/// let strict = elide(&pieces, 3, 2, 12, MarkAt::End);
/// assert_eq!(strict.kept, [0]);
/// let filled = elide_filled(&pieces, 3, 2, 12, MarkAt::End);
/// assert_eq!(filled.kept, [0, 2]);
/// assert!(filled.end_mark);
/// assert_eq!(filled.width(&pieces, 3, 2), 11);
/// ```
pub fn elide_filled(
    pieces: &[Piece],
    separator: usize,
    mark: usize,
    room: usize,
    at: MarkAt,
) -> Elided {
    let mut line = elide(pieces, separator, mark, room, at);
    let marked = !line.marks.is_empty() || line.end_mark;
    if line.kept.len() == pieces.len() || !marked {
        return line;
    }
    let mut shown = vec![false; pieces.len()];
    for &index in &line.kept {
        shown[index] = true;
    }
    // Only ranked pieces give way, so every piece out has a rank.
    let mut out: Vec<usize> = (0..pieces.len()).filter(|&index| !shown[index]).collect();
    out.sort_by_key(|&index| (Reverse(pieces[index].rank), index));
    let mut closed = None;
    for index in out {
        let rank = pieces[index].rank;
        if closed == Some(rank) {
            continue;
        }
        shown[index] = true;
        let trial = layout(&shown, at);
        if trial.width(pieces, separator, mark) <= room {
            line = trial;
        } else {
            shown[index] = false;
            closed = Some(rank);
        }
    }
    line
}

/// The line that keeps the `shown` pieces, with its marks placed `at`.
fn layout(shown: &[bool], at: MarkAt) -> Elided {
    let mut line = Elided::default();
    let mut in_run = false;
    for (index, &on) in shown.iter().enumerate() {
        if on {
            line.kept.push(index);
            in_run = false;
            continue;
        }
        match at {
            MarkAt::Gap if !in_run => line.marks.push(line.kept.len()),
            MarkAt::Gap => {}
            MarkAt::End => line.end_mark = true,
        }
        in_run = true;
    }
    line
}

#[cfg(test)]
#[path = "elide_tests.rs"]
mod tests;
