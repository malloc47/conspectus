//! The section rule: the line that heads one section of a pane.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// The line that heads a section of a pane: a rule that fills the width,
/// the section's label as a chip, an optional summary beside it, and a
/// short closing rule.
///
/// ```text
/// ──────────────────────── Label  summary ──    the summary after the chip
/// ──────────────────────── summary  Label ──    summary_first
/// ─────────────────────────────────── Label ──  no summary
/// ```
///
/// The chip sits near the right end of every rule, two cells from it, so
/// the labels of the sections stacked down a pane line up. With the
/// summary first, the chip stays in that column however long the summary
/// grows, which keeps the label easy to find down a pane whose counts
/// change. The leading rule takes the cells the chip, the summary, and
/// the closing rule leave, measured in terminal cells, and drops out when
/// they leave none.
///
/// The line is never wider than its width. Below the width of the chip,
/// the summary, and the closing rule, the pieces give way in turn, the
/// closing rule kept: the summary is cut with the ellipsis mark, then
/// leaves (with the spaces around it) once it would show no character
/// before the mark; then the label is cut the same way, and then the chip
/// leaves, and the line is a plain rule. Lay it out at the width it is
/// drawn in.
///
/// ```
/// use tuikit::frame::SectionRule;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let text = |rule: SectionRule| rule.line(30, &theme).to_string();
/// assert_eq!(text(SectionRule::new("Detail")), "──────────────────── Detail ──");
/// assert_eq!(
///     text(SectionRule::new("Links").summary("3 shown")),
///     "──────────── Links  3 shown ──",
/// );
/// assert_eq!(
///     text(SectionRule::new("Links").summary("3 shown").summary_first()),
///     "──────────── 3 shown  Links ──",
/// );
/// // Too narrow for both: the summary gives way first, then the label.
/// let narrow = |width: usize| {
///     SectionRule::new("Links")
///         .summary("3 shown · 1 more")
///         .summary_first()
///         .line(width, &theme)
///         .to_string()
/// };
/// assert_eq!(narrow(20), "─ 3 shown…  Links ──");
/// assert_eq!(narrow(10), "─ Links ──");
/// assert_eq!(narrow(7), " Li… ──");
/// assert_eq!(narrow(3), "───");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionRule {
    label: String,
    summary: Option<String>,
    summary_first: bool,
}

impl SectionRule {
    /// The cells of rule that close the line after the chip and summary.
    pub const CLOSING: usize = 2;

    /// The rule of the section named `label`, with no summary.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            summary: None,
            summary_first: false,
        }
    }

    /// Show `summary` beside the chip (after it, unless
    /// [`summary_first`](Self::summary_first)), with a space on each
    /// side. An empty summary shows nothing.
    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into()).filter(|summary| !summary.is_empty());
        self
    }

    /// Put the summary before the chip, so the chip sits against the
    /// closing rule in the same column whatever the summary says.
    #[must_use]
    pub fn summary_first(mut self) -> Self {
        self.summary_first = true;
        self
    }

    /// The section's label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The rule `width` cells wide, in `theme`'s looks: the rules in
    /// `muted`, the chip in `accent` under `badge`, the summary in
    /// `inert`. Where the chip, the summary, and the closing rule do not
    /// fit, the summary and then the label give way (see
    /// [`SectionRule`]); a width too narrow for even a cut label is a
    /// plain rule.
    pub fn line(&self, width: usize, theme: &Theme) -> Line<'static> {
        let rule = Style::default().add_modifier(theme.muted);
        let Some((label, summary)) = self.fitted(width.saturating_sub(Self::CLOSING)) else {
            if width == 0 {
                return Line::default();
            }
            return Line::from(Span::styled(marks::RULE.repeat(width), rule));
        };
        let chip = format!(" {label} ");
        let summary = summary.map(|summary| format!(" {summary} "));
        let used = text::width(&chip) + summary.as_deref().map_or(0, text::width) + Self::CLOSING;
        let leading = width.saturating_sub(used);
        let chip = Span::styled(
            chip,
            Style::default().fg(theme.accent).add_modifier(theme.badge),
        );
        let summary =
            summary.map(|summary| Span::styled(summary, Style::default().fg(theme.inert)));
        let mut spans = Vec::with_capacity(4);
        if leading > 0 {
            spans.push(Span::styled(marks::RULE.repeat(leading), rule));
        }
        if self.summary_first {
            spans.extend(summary);
            spans.push(chip);
        } else {
            spans.push(chip);
            spans.extend(summary);
        }
        spans.push(Span::styled(marks::RULE.repeat(Self::CLOSING), rule));
        Line::from(spans)
    }

    /// The label and the summary as they fit in `room` cells, the space
    /// the closing rule leaves: whole when they fit; otherwise the summary
    /// cut, or gone, with the label whole; otherwise the label cut and no
    /// summary; `None` when not even a cut label fits.
    fn fitted(&self, room: usize) -> Option<(String, Option<String>)> {
        let chip = text::width(&self.label) + PADDING;
        if chip <= room {
            let left = room - chip;
            let summary = self
                .summary
                .as_deref()
                .and_then(|summary| cut(summary, left.saturating_sub(PADDING)));
            return Some((self.label.clone(), summary));
        }
        cut(&self.label, room.saturating_sub(PADDING)).map(|label| (label, None))
    }
}

/// The cells a chip or a summary takes beyond its text: a space on each
/// side.
const PADDING: usize = 2;

/// `text` in `cells` cells: whole when it fits, cut with the ellipsis
/// mark when a character still shows before it, and `None` when not. A
/// cut that ends at a space drops it, so the mark follows the last word
/// it kept (`3…`, not `3 …`); the rule before the chip takes the cell.
fn cut(text: &str, cells: usize) -> Option<String> {
    if text::width(text) <= cells {
        return Some(text.to_string());
    }
    let cut = text::truncate_end(text, cells);
    match cut.strip_suffix(marks::ELLIPSIS) {
        Some(kept) => {
            let kept = kept.trim_end();
            (!kept.is_empty()).then(|| format!("{kept}{}", marks::ELLIPSIS))
        }
        // Only spaces were past the budget: nothing was lost.
        None => (!cut.is_empty()).then_some(cut),
    }
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
