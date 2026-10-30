//! The one title form the modal frame and the pane frame share: the
//! title's own spans, with the spaces at its ends dropped, between the
//! spaces the frame adds.

use ratatui::text::{Line, Span};

/// `title` (or a footer, which takes the same form) with one space on
/// each side and the spaces it had at its ends dropped; `None` when it
/// has no text.
pub(super) fn framed(title: Line<'_>) -> Option<Line<'_>> {
    let mut title = trimmed(title);
    if title.spans.is_empty() {
        return None;
    }
    title.spans.insert(0, Span::raw(" "));
    title.spans.push(Span::raw(" "));
    Some(title)
}

/// `title` without its empty spans and without the spaces at its ends,
/// its style and alignment kept. A title of spaces alone has no spans
/// left.
pub(super) fn trimmed(title: Line<'_>) -> Line<'_> {
    let Line {
        style,
        alignment,
        spans,
    } = title;
    let mut spans: Vec<Span<'_>> = spans
        .into_iter()
        .filter(|span| !span.content.is_empty())
        .collect();
    trim_ends(&mut spans);
    Line {
        style,
        alignment,
        spans,
    }
}

/// Drops the spaces at the start of the first spans and at the end of the
/// last ones, removing spans that become empty.
fn trim_ends(spans: &mut Vec<Span<'_>>) {
    while let Some(first) = spans.first_mut() {
        let trimmed = first.content.trim_start();
        if trimmed.is_empty() {
            spans.remove(0);
            continue;
        }
        if trimmed.len() != first.content.len() {
            first.content = trimmed.to_string().into();
        }
        break;
    }
    while let Some(last) = spans.last_mut() {
        let trimmed = last.content.trim_end();
        if trimmed.is_empty() {
            spans.pop();
            continue;
        }
        if trimmed.len() != last.content.len() {
            last.content = trimmed.to_string().into();
        }
        break;
    }
}
