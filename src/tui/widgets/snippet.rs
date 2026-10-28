//! Fit a search snippet to a cell budget without losing the match.
//!
//! A snippet is a short window of a transcript with the matched bytes
//! marked. When a surface has fewer cells than the snippet is wide, the
//! cut must keep the match on screen: crop around it, favoring the text
//! after the match, and cut on word boundaries so neither end starts or
//! stops mid-word unless a single word fills the room.

use std::ops::Range;

use tuikit::glyphs::marks;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A snippet cut to fit, with the match's byte range in the cut text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fitted {
    pub text: String,
    pub highlight: Option<Range<usize>>,
}

/// `text` cut to at most `budget` cells around the byte range
/// `highlight`, with Markdown code ticks removed (they only add noise
/// to a one-line preview).
pub fn fit(text: &str, highlight: &Range<usize>, budget: usize) -> Fitted {
    let (text, highlight) = strip_ticks(text, highlight);
    if budget == 0 {
        return Fitted {
            text: String::new(),
            highlight: None,
        };
    }
    if UnicodeWidthStr::width(text.as_str()) <= budget {
        return Fitted { text, highlight };
    }
    let Some(highlight) = highlight else {
        return Fitted {
            text: head(&text, budget),
            highlight: None,
        };
    };
    crop(&text, &highlight, budget)
}

/// The text with every backtick removed and the highlight moved to match.
fn strip_ticks(text: &str, highlight: &Range<usize>) -> (String, Option<Range<usize>>) {
    let valid = highlight.start < highlight.end
        && highlight.end <= text.len()
        && text.is_char_boundary(highlight.start)
        && text.is_char_boundary(highlight.end);
    let mut out = String::with_capacity(text.len());
    let (mut start, mut end) = (None, None);
    for (byte, c) in text.char_indices() {
        if valid && byte == highlight.start {
            start = Some(out.len());
        }
        if valid && byte == highlight.end {
            end = Some(out.len());
        }
        if c != '`' {
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

fn head(text: &str, budget: usize) -> String {
    if UnicodeWidthStr::width(text) <= budget {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > budget.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push_str(marks::ELLIPSIS);
    out
}

struct Cell {
    byte: usize,
    c: char,
    width: usize,
}

fn crop(text: &str, highlight: &Range<usize>, budget: usize) -> Fitted {
    let cells: Vec<Cell> = text
        .char_indices()
        .map(|(byte, c)| Cell {
            byte,
            c,
            width: c.width().unwrap_or(0),
        })
        .collect();
    let first = cells.partition_point(|cell| cell.byte < highlight.start);
    let last = cells.partition_point(|cell| cell.byte < highlight.end);
    let match_width: usize = cells[first..last].iter().map(|c| c.width).sum();

    // The match alone fills the room: show as much of it as fits.
    if match_width + 1 >= budget {
        let mut shown = String::new();
        let mut used = 0;
        for cell in &cells[first..last] {
            if used + cell.width > budget.saturating_sub(1) {
                break;
            }
            shown.push(cell.c);
            used += cell.width;
        }
        let range = 0..shown.len();
        shown.push_str(marks::ELLIPSIS);
        return Fitted {
            text: shown,
            highlight: Some(range),
        };
    }

    let before_width: usize = cells[..first].iter().map(|c| c.width).sum();
    let after_width: usize = cells[last..].iter().map(|c| c.width).sum();
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

    // Start and stop on word boundaries when the cut would split a word,
    // unless that would trade away the room next to the match.
    if start > 0
        && !cells[start - 1].c.is_whitespace()
        && !cells[start].c.is_whitespace()
        && let Some(offset) = cells[start..first].iter().position(|c| c.c.is_whitespace())
    {
        start += offset;
    }
    while start < first && cells[start].c.is_whitespace() {
        start += 1;
    }
    if end < cells.len()
        && !cells[end - 1].c.is_whitespace()
        && !cells[end].c.is_whitespace()
        && let Some(offset) = cells[last..end].iter().rposition(|c| c.c.is_whitespace())
    {
        end = last + offset;
    }
    while end > last && cells[end - 1].c.is_whitespace() {
        end -= 1;
    }

    let mut out = String::new();
    if start > 0 {
        out.push_str(marks::ELLIPSIS);
    }
    let lead = out.len();
    let from = cells[start].byte;
    let to = cells.get(end).map_or(text.len(), |c| c.byte);
    out.push_str(&text[from..to]);
    if end < cells.len() {
        out.push_str(marks::ELLIPSIS);
    }
    let begin = lead + (highlight.start - from);
    let finish = lead + (highlight.end - from);
    Fitted {
        text: out,
        highlight: Some(begin..finish),
    }
}

#[cfg(test)]
#[path = "snippet_tests.rs"]
mod tests;
