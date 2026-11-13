//! Fitting a snippet to a cell budget without losing its match.

use std::ops::Range;

use super::cut::truncate_end;
use super::measure::{Cluster, clusters, width};
use crate::glyphs::marks;

/// A snippet cut to fit, and where its match sits in the cut text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    /// The text to show, with the ellipsis mark on each side that was
    /// cut.
    pub text: String,
    /// The byte range of the match inside [`Snippet::text`], or `None`
    /// when the match was empty, out of range, or not on character
    /// boundaries, or when nothing of it is left.
    pub highlight: Option<Range<usize>>,
}

/// `text` cut to at most `budget` cells around the match at byte range
/// `highlight`, so the match stays on screen.
///
/// Every character in `drop` is removed first, and the match moves with
/// the text (pass `&['`']` to drop Markdown code ticks from a one-line
/// preview, `&[]` to keep everything). Then, when the text is wider than
/// the budget:
///
/// - the cut keeps the match and spends the room left on context,
///   favoring the text after the match (two thirds of the spare cells);
/// - each cut side ends in the ellipsis mark, inside the budget;
/// - a cut that would split a word moves to the word's edge toward the
///   match, even when the only edge on that side is the one beside the
///   match and the side shows nothing: a cut word reads worse than none;
/// - a token is the exception: when the only edge is the one beside the
///   match, a token there keeps the part that fits, cut between
///   graphemes. A token is a stretch between blanks that holds a wide
///   character, is wider than 24 cells (a hash, a long id), or has a
///   `/`, `\`, `_`, `.`, or `:` with a character before it and two
///   letters after it (a path, an address, a code name; not `e.g.`,
///   `3.14`, or `10:30`);
/// - words end at blanks and beside wide characters: Chinese and
///   Japanese text puts no spaces between its words, so a cut may fall
///   between any two of its characters;
/// - a match wider than the budget shows as much of its start as fits,
///   followed by the mark;
/// - without a valid match, the text is cut at its end.
///
/// The match is widened to whole graphemes, so a highlight never
/// separates a letter from its accent.
///
/// ```
/// use tuikit::text::fit_snippet;
///
/// let text = "the cache is rebuilt from the index whenever the schema changes";
/// let at = text.find("index").unwrap();
/// let fitted = fit_snippet(text, at..at + 5, 30, &[]);
/// assert_eq!(fitted.text, "…the index whenever the…");
/// let range = fitted.highlight.unwrap();
/// assert_eq!(&fitted.text[range], "index");
/// ```
pub fn fit_snippet(text: &str, highlight: Range<usize>, budget: usize, drop: &[char]) -> Snippet {
    let (text, highlight) = drop_chars(text, &highlight, drop);
    let highlight = highlight.map(|range| to_graphemes(&text, range));
    if budget == 0 {
        return Snippet {
            text: String::new(),
            highlight: None,
        };
    }
    if width(&text) <= budget {
        return Snippet { text, highlight };
    }
    match highlight {
        Some(highlight) => crop(&text, &highlight, budget),
        None => Snippet {
            text: truncate_end(&text, budget),
            highlight: None,
        },
    }
}

/// `text` without the characters in `drop`, and `highlight` moved to
/// match; `None` when the range is not a valid match in `text`.
fn drop_chars(
    text: &str,
    highlight: &Range<usize>,
    drop: &[char],
) -> (String, Option<Range<usize>>) {
    let valid = highlight.start < highlight.end
        && highlight.end <= text.len()
        && text.is_char_boundary(highlight.start)
        && text.is_char_boundary(highlight.end);
    if drop.is_empty() {
        return (text.to_string(), valid.then(|| highlight.clone()));
    }
    let mut out = String::with_capacity(text.len());
    let (mut start, mut end) = (None, None);
    for (byte, c) in text.char_indices() {
        if valid && byte == highlight.start {
            start = Some(out.len());
        }
        if valid && byte == highlight.end {
            end = Some(out.len());
        }
        if !drop.contains(&c) {
            out.push(c);
        }
    }
    if valid && highlight.end == text.len() {
        end = Some(out.len());
    }
    let range = match (start, end) {
        (Some(start), Some(end)) if start < end => Some(start..end),
        _ => None,
    };
    (out, range)
}

/// `range` widened to the edges of the graphemes it touches.
fn to_graphemes(text: &str, range: Range<usize>) -> Range<usize> {
    let mut start = range.start;
    let mut end = range.end;
    for cluster in clusters(text) {
        if cluster.at < range.start && range.start < cluster.end() {
            start = cluster.at;
        }
        if cluster.at < range.end && range.end < cluster.end() {
            end = cluster.end();
        }
    }
    start..end
}

fn cells_of(cells: &[Cluster<'_>]) -> usize {
    cells.iter().map(|cell| cell.width).sum()
}

/// Whether a cut just before `cells[at]` falls on a word's edge rather
/// than inside a word: at either end of the text, beside a blank, or
/// beside a wide character. Scripts set in wide characters (Chinese,
/// Japanese) write a sentence with no spaces between its words, and a
/// line may break between any two of their characters, so a cut may too.
fn is_edge(cells: &[Cluster<'_>], at: usize) -> bool {
    match (at.checked_sub(1).and_then(|i| cells.get(i)), cells.get(at)) {
        (Some(before), Some(after)) => {
            before.is_blank() || after.is_blank() || before.is_wide() || after.is_wide()
        }
        _ => true,
    }
}

/// The widest an ordinary word runs, in cells. Words in common use stay
/// within it with the punctuation and quotes around them
/// (`"internationalization",` is 23 cells), while a token with no
/// separator in it is wider: a commit hash is 40 cells, a dashed id 36.
const WORD_CELLS: usize = 24;

/// The separators that join the names of a path, an address, or a code
/// name.
const SEPARATORS: &[&str] = &["/", "\\", "_", ".", ":"];

/// Whether `run`, a stretch of text between blanks, reads as a token
/// rather than a word: it holds a wide character (unspaced Chinese or
/// Japanese, an emoji), it is wider than [`WORD_CELLS`], or it joins
/// names with a separator. A separator joins names when a character
/// comes before it and two letters after it (`docs/notes.md`,
/// `snake_case`, `example.com`, `std::fmt`); abbreviations (`e.g.`),
/// figures (`3.14`, `10:30`, `24/7`), and a word's closing punctuation
/// do not.
fn is_token(run: &[Cluster<'_>]) -> bool {
    let is_letter = |cell: &Cluster<'_>| cell.text.chars().next().is_some_and(char::is_alphabetic);
    let joins_names = (1..run.len()).any(|at| {
        SEPARATORS.contains(&run[at].text)
            && run
                .get(at + 1..at + 3)
                .is_some_and(|next| next.iter().all(is_letter))
    });
    joins_names || run.iter().any(Cluster::is_wide) || cells_of(run) > WORD_CELLS
}

/// The stretch of text between blanks around `cells[at]`.
fn run_around<'a, 'b>(cells: &'b [Cluster<'a>], at: usize) -> &'b [Cluster<'a>] {
    let mut from = at;
    while from > 0 && !cells[from - 1].is_blank() {
        from -= 1;
    }
    let mut to = at;
    while to < cells.len() && !cells[to].is_blank() {
        to += 1;
    }
    &cells[from..to]
}

/// `from` moved forward past blanks, stopping at `limit`.
fn skip_blanks(cells: &[Cluster<'_>], from: usize, limit: usize) -> usize {
    let mut at = from;
    while at < limit && cells[at].is_blank() {
        at += 1;
    }
    at
}

/// `to` moved back past the blanks before it, stopping at `limit`.
fn trim_blanks(cells: &[Cluster<'_>], limit: usize, to: usize) -> usize {
    let mut at = to;
    while at > limit && cells[at - 1].is_blank() {
        at -= 1;
    }
    at
}

fn crop(text: &str, highlight: &Range<usize>, budget: usize) -> Snippet {
    let cells = clusters(text);
    let first = cells.partition_point(|cell| cell.at < highlight.start);
    let last = cells.partition_point(|cell| cell.at < highlight.end);
    let match_width = cells_of(&cells[first..last]);

    // The match alone fills the room: show as much of it as fits.
    if match_width + 1 >= budget {
        let mut end = first;
        let mut used = 0;
        while end < last && used + cells[end].width < budget {
            used += cells[end].width;
            end += 1;
        }
        let shown = if end > first {
            &text[cells[first].at..cells[end - 1].end()]
        } else {
            ""
        };
        return Snippet {
            text: format!("{shown}{}", marks::ELLIPSIS),
            highlight: (!shown.is_empty()).then_some(0..shown.len()),
        };
    }

    let before_width = cells_of(&cells[..first]);
    let after_width = cells_of(&cells[last..]);
    let spare = budget - match_width;
    // Context after the match is worth more than before it.
    let after_want = after_width.min(spare * 2 / 3);
    let before_want = before_width.min(spare - after_want);
    let after_want = after_width.min(spare - before_want);
    let cut_before = before_width > before_want;
    let cut_after = after_width > after_want;
    let before_room = before_want.saturating_sub(usize::from(cut_before));
    let after_room = after_want.saturating_sub(usize::from(cut_after));

    // Walk outward from the match until the room is used.
    let mut start = first;
    let mut used = 0;
    while start > 0 && used + cells[start - 1].width <= before_room {
        used += cells[start - 1].width;
        start -= 1;
    }
    let mut end = last;
    used = 0;
    while end < cells.len() && used + cells[end].width <= after_room {
        used += cells[end].width;
        end += 1;
    }

    // Start and stop on a word's edge when the cut would split a word.
    // When the only edge on that side is the one next to the match, past
    // the blanks there, moving to it gives up all the context on that
    // side: a word goes whole, but a token (a path, an address, unspaced
    // CJK text) keeps the part that fits, the cut staying between its
    // graphemes. A cut with no edge between it and the match stays put.
    if !is_edge(&cells, start)
        && let Some(edge) = (start + 1..=first).find(|&at| is_edge(&cells, at))
    {
        let kept = skip_blanks(&cells, edge, first);
        if kept < first || (edge < first && !is_token(run_around(&cells, start))) {
            start = kept;
        }
    }
    start = skip_blanks(&cells, start, first);
    if !is_edge(&cells, end)
        && let Some(edge) = (last..end).rev().find(|&at| is_edge(&cells, at))
    {
        let kept = trim_blanks(&cells, last, edge);
        if kept > last || (edge > last && !is_token(run_around(&cells, end - 1))) {
            end = kept;
        }
    }
    end = trim_blanks(&cells, last, end);

    let mut out = String::new();
    if start > 0 {
        out.push_str(marks::ELLIPSIS);
    }
    let lead = out.len();
    let from = cells[start].at;
    let to = cells.get(end).map_or(text.len(), |cell| cell.at);
    out.push_str(&text[from..to]);
    if end < cells.len() {
        out.push_str(marks::ELLIPSIS);
    }
    Snippet {
        text: out,
        highlight: Some(lead + (highlight.start - from)..lead + (highlight.end - from)),
    }
}

#[cfg(test)]
#[path = "snippet_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snippet_prose_tests.rs"]
mod prose_tests;
