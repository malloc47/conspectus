//! A choice among a few words, drawn in a title.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::theme::Theme;

/// A choice among a few words in a title: `[ One ] Two`.
///
/// The chosen word is drawn in brackets in the chip look (the theme's
/// `badge` modifier over its `accent`); the others are plain, one space
/// apart. Brackets take the same cells whichever word holds them, so the
/// title keeps its width as the choice changes. The host says which
/// words there are and which is chosen; the title only draws them.
///
/// ```
/// use tuikit::chip::SegmentedTitle;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let line: String = SegmentedTitle::new(&[("Near", false), ("Far", true)])
///     .spans(&theme)
///     .iter()
///     .map(|span| span.content.as_ref())
///     .collect();
/// assert_eq!(line, "Near [ Far ]");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentedTitle<'a> {
    segments: &'a [(&'a str, bool)],
}

impl<'a> SegmentedTitle<'a> {
    /// A title of `segments`, each a word and whether it is chosen.
    pub fn new(segments: &'a [(&'a str, bool)]) -> Self {
        Self { segments }
    }

    /// The style the chosen word draws in: the theme's `badge` modifier
    /// over its `accent`.
    pub fn chosen_look(theme: &Theme) -> Style {
        Style::default().fg(theme.accent).add_modifier(theme.badge)
    }

    /// The title as spans: one per word and one per gap.
    pub fn spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        let chosen = Self::chosen_look(theme);
        let mut spans = Vec::with_capacity(self.segments.len() * 2);
        for (index, (word, on)) in self.segments.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(" "));
            }
            if *on {
                spans.push(Span::styled(format!("[ {word} ]"), chosen));
            } else {
                spans.push(Span::raw((*word).to_string()));
            }
        }
        spans
    }
}
