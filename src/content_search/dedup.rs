//! Dropping pane matches that a session already reports.
//!
//! A pane that runs an agent mirrors that agent's transcript, so the same
//! sentence would surface twice: once from the session and once from the
//! pane. The rule (ADR 0149): a pane line is a duplicate when its
//! normalized text is contained in the normalized body of any turn of an
//! attributed session that matches the query. Every turn kind counts,
//! not only the kinds being searched, so a pane never surfaces text the
//! session search deliberately withheld (tool output is on screen in the
//! pane but off by default in the search). A line shorter than
//! [`SHORT_LINE_CHARS`] is judged together with its neighbors, since a
//! few words on their own match too easily.

use crate::viewer::model::TranscriptDocument;
use crate::viewer::search::find_ci;

/// Lines with fewer characters than this are compared with their
/// neighbors joined.
pub const SHORT_LINE_CHARS: usize = 12;

/// Fold text the way the matcher does and make it comparable across a
/// pane and a transcript: ASCII letters lowercase, box-drawing and block
/// characters removed, and runs of whitespace collapsed to one space.
pub fn normalize_for_compare(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for c in text.chars() {
        if is_border(c) {
            // A border stands between words on a pane line, so it
            // separates them like a space does.
            pending_space = true;
            continue;
        }
        if c.is_whitespace() || c.is_control() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Box drawing (U+2500..U+257F) and block elements (U+2580..U+259F):
/// the borders and fills terminal UIs draw around text.
fn is_border(c: char) -> bool {
    ('\u{2500}'..='\u{259f}').contains(&c)
}

/// The normalized bodies of every turn in `document` that contains
/// `needle`, whatever the turn's kind. These are what a pane line is
/// compared against.
pub fn matching_bodies(document: &TranscriptDocument, needle: &str) -> Vec<String> {
    let folded = normalize_for_compare(needle);
    let has_space = needle.chars().any(char::is_whitespace);
    document
        .turns
        .iter()
        .filter(|turn| {
            if has_space {
                normalize_for_compare(&turn.body).contains(&folded)
            } else {
                find_ci(&turn.body, needle).is_some()
            }
        })
        .map(|turn| normalize_for_compare(&turn.body))
        .collect()
}

/// Whether line `index` of a pane duplicates one of the session `bodies`.
/// `lines` are the pane's lines, already normalized.
pub fn is_duplicate(lines: &[String], index: usize, bodies: &[String]) -> bool {
    let Some(line) = lines.get(index) else {
        return false;
    };
    let target = if line.chars().count() < SHORT_LINE_CHARS {
        let mut parts = Vec::with_capacity(3);
        if let Some(previous) = index.checked_sub(1).and_then(|i| lines.get(i))
            && !previous.is_empty()
        {
            parts.push(previous.as_str());
        }
        parts.push(line.as_str());
        if let Some(next) = lines.get(index + 1)
            && !next.is_empty()
        {
            parts.push(next.as_str());
        }
        parts.join(" ")
    } else {
        line.clone()
    };
    if target.is_empty() {
        return false;
    }
    bodies.iter().any(|body| body.contains(&target))
}

/// Split `matched` (indexes into `lines`) into the matches to keep and
/// the number dropped as duplicates of `bodies`.
pub fn drop_duplicates(
    lines: &[String],
    matched: &[usize],
    bodies: &[String],
) -> (Vec<usize>, usize) {
    if bodies.is_empty() {
        return (matched.to_vec(), 0);
    }
    let mut kept = Vec::with_capacity(matched.len());
    let mut dropped = 0;
    for &index in matched {
        if is_duplicate(lines, index, bodies) {
            dropped += 1;
        } else {
            kept.push(index);
        }
    }
    (kept, dropped)
}
