//! Footers generated from a key table, in the one grammar.

use std::fmt;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::table::{Hint, KeyRow, rows};
use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// What joins the segments of a footer. A footer too wide for its row
/// wraps only here, so a key stays beside its verb.
pub const HINT_SEPARATOR: &str = marks::SPACED_SEPARATOR;

/// One segment of a footer: the labels of the rows that share a verb and
/// the verb (`↑/↓ move`), or a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The segment as drawn.
    pub text: String,
    /// Whether any of its keys can act now; a note always can.
    pub available: bool,
}

/// The footer of a key table: one [`Segment`] per verb or note, in the
/// order each first appears, with aliases left out.
///
/// The grammar is the same in every footer: the keys, a space, and a
/// short imperative verb with no "to" (`Esc close`, `Enter pick`,
/// `Space toggle`). Labels that share a verb join with `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Footer {
    segments: Vec<Segment>,
}

impl Footer {
    /// The footer of `table`, included tables spliced in.
    pub fn new<A>(table: &[KeyRow<A>]) -> Self {
        enum Part {
            Keys {
                verb: &'static str,
                labels: Vec<&'static str>,
                available: bool,
            },
            Note(&'static str),
        }
        let mut parts: Vec<Part> = Vec::new();
        for row in rows(table) {
            match row {
                KeyRow::Bind {
                    hint: Hint::Shown { label, verb },
                    available,
                    ..
                } => {
                    let existing = parts
                        .iter_mut()
                        .find(|part| matches!(part, Part::Keys { verb: v, .. } if v == verb));
                    match existing {
                        Some(Part::Keys {
                            labels,
                            available: any,
                            ..
                        }) => {
                            if !labels.contains(label) {
                                labels.push(label);
                            }
                            *any |= *available;
                        }
                        _ => parts.push(Part::Keys {
                            verb,
                            labels: vec![label],
                            available: *available,
                        }),
                    }
                }
                KeyRow::Note(note) => parts.push(Part::Note(note)),
                _ => {}
            }
        }
        let segments = parts
            .into_iter()
            .map(|part| match part {
                Part::Keys {
                    verb,
                    labels,
                    available,
                } => Segment {
                    text: format!("{} {verb}", labels.join("/")),
                    available,
                },
                Part::Note(note) => Segment {
                    text: note.to_string(),
                    available: true,
                },
            })
            .collect();
        Self { segments }
    }

    /// The segments, in order.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether the footer says nothing: every row is an alias.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// The footer as one line of text, segments joined with
    /// [`HINT_SEPARATOR`].
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(HINT_SEPARATOR)
    }

    /// The footer in rows at most `width` cells wide, broken only between
    /// segments (a segment wider than a row is word-wrapped on rows of
    /// its own, with no trailing spaces), as [`text::wrap_at`] breaks
    /// [`Footer::text`]. Available
    /// segments and the separators are unstyled, so they take the style
    /// the caller draws the footer in; an unavailable segment takes
    /// [`unavailable_style`].
    pub fn lines(&self, width: usize, theme: &Theme) -> Vec<Line<'static>> {
        let width = width.max(1);
        let gap = text::width(HINT_SEPARATOR);
        let mut out: Vec<Line<'static>> = Vec::new();
        let mut current: Vec<Span<'static>> = Vec::new();
        let mut used = 0;
        for segment in &self.segments {
            let style = if segment.available {
                Style::default()
            } else {
                unavailable_style(theme)
            };
            let cells = text::width(&segment.text);
            if !current.is_empty() && used + gap + cells <= width {
                current.push(Span::raw(HINT_SEPARATOR));
                current.push(Span::styled(segment.text.clone(), style));
                used += gap + cells;
                continue;
            }
            if !current.is_empty() {
                out.push(Line::from(std::mem::take(&mut current)));
            }
            if cells <= width {
                current.push(Span::styled(segment.text.clone(), style));
                used = cells;
            } else {
                let mut pieces: Vec<String> = text::wrap(&segment.text, width)
                    .into_iter()
                    .map(|piece| piece.trim_end().to_string())
                    .collect();
                let last = pieces.pop().unwrap_or_default();
                out.extend(
                    pieces
                        .into_iter()
                        .map(|piece| Line::from(Span::styled(piece, style))),
                );
                used = text::width(&last);
                current.push(Span::styled(last, style));
            }
        }
        if !current.is_empty() || out.is_empty() {
            out.push(Line::from(current));
        }
        out
    }
}

impl fmt::Display for Footer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

/// The footer of `table` as one line of text: [`Footer::text`].
pub fn footer<A>(table: &[KeyRow<A>]) -> String {
    Footer::new(table).text()
}

/// How a footer or a help page draws an action that cannot run now: the
/// theme's `inert` color, receding with its `muted` modifier, so it
/// stands apart from a footer drawn plain, muted, or inert.
pub fn unavailable_style(theme: &Theme) -> Style {
    Style::default().fg(theme.inert).add_modifier(theme.muted)
}
