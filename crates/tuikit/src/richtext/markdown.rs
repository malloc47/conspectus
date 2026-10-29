//! Markdown, with its tables and highlighted code, as styled lines.

use ratatui::text::{Line, Span};

use super::table::{BodySegment, render_table, segment_body};
use crate::text;
use crate::theme::Theme;

/// `body`, Markdown, as styled lines no wider than `width` cells: headings,
/// emphasis, lists, block quotes, links, and inline code styled; fenced
/// code highlighted by its language when the highlighter knows it (and
/// shown plain when not); and GFM pipe tables laid out to the width by
/// [`render_table`], their borders in the theme's inert tone.
///
/// Lines wrap by the [wrapping rules](crate::text#wrapping-rules), and a
/// loose list's markers stay beside their items' text. Blank lines between
/// blocks are kept.
///
/// ```
/// use tuikit::richtext::markdown;
/// use tuikit::theme::Theme;
///
/// let body = "# Notes\n\nSome **bold** words that wrap.\n\n| a | b |\n|---|---|\n| 1 | 2 |";
/// let lines = markdown(body, 20, &Theme::default());
/// assert!(lines.iter().all(|line| line.width() <= 20));
/// assert!(lines.iter().any(|line| line.to_string().contains("bold")));
/// ```
pub fn markdown(body: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for segment in segment_body(body) {
        match segment {
            BodySegment::Markdown(src) => {
                let rendered = tui_markdown::from_str(src);
                for line in join_orphan_list_markers(rendered.lines) {
                    out.extend(text::wrap_line(&line, usize::from(width), 0));
                }
            }
            BodySegment::Table(parsed) => out.extend(render_table(&parsed, width, theme)),
        }
    }
    out
}

/// `source`, code in `language` (a fenced block's info string: `rust`,
/// `py`, `toml`), as highlighted lines, one per line of the source and
/// not wrapped. A language the highlighter does not know gives the lines
/// unstyled.
///
/// ```
/// use tuikit::richtext::code;
///
/// let lines = code("fn main() {}\nlet x = 1;", "rust");
/// assert_eq!(lines.len(), 2);
/// assert_eq!(lines[0].to_string(), "fn main() {}");
/// assert!(lines[0].spans.iter().any(|span| span.style.fg.is_some()));
/// ```
pub fn code(source: &str, language: &str) -> Vec<Line<'static>> {
    // The block is read as a fenced block whose fence is longer than any
    // run of backticks in the source, so the source cannot close it.
    let longest = source.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let block = format!("{fence}{language}\n{source}\n{fence}");
    let rendered = tui_markdown::from_str(&block);
    let mut lines = rendered.lines;
    // The renderer draws the opening and closing fences as lines of their
    // own; the code is what lies between them.
    lines.truncate(lines.len().saturating_sub(1));
    lines.into_iter().skip(1).map(owned).collect()
}

/// `line` with its text copied, so it outlives the source it borrowed.
fn owned(line: Line<'_>) -> Line<'static> {
    Line {
        spans: line
            .spans
            .into_iter()
            .map(|span| Span::styled(span.content.into_owned(), span.style))
            .collect(),
        style: line.style,
        alignment: line.alignment,
    }
}

/// Put a list marker back on the line of its item's text.
///
/// For a loose list (items separated by blank lines, which generated text
/// often has) the Markdown renderer emits the `1. ` marker as a line of
/// its own and the item's paragraph on the next. Join the two so the
/// number sits beside its text, as it does in a tight list.
fn join_orphan_list_markers(lines: Vec<Line<'_>>) -> Vec<Line<'_>> {
    let mut out: Vec<Line<'_>> = Vec::with_capacity(lines.len());
    let mut iter = lines.into_iter().peekable();
    while let Some(line) = iter.next() {
        if is_bare_list_marker(&line)
            && let Some(next) = iter.next_if(|next| !next.spans.is_empty())
        {
            let mut joined = line;
            joined.spans.extend(next.spans);
            out.push(joined);
        } else {
            out.push(line);
        }
    }
    out
}

/// A line holding only a list marker such as `1. `, `2) `, or `- `.
fn is_bare_list_marker(line: &Line<'_>) -> bool {
    let [span] = line.spans.as_slice() else {
        return false;
    };
    let text = span.content.trim_start();
    let Some(marker) = text.strip_suffix(' ') else {
        return false;
    };
    matches!(marker, "-" | "*" | "+")
        || marker
            .strip_suffix(['.', ')'])
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
