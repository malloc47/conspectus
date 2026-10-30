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
/// The line is never narrower than its chip, its summary, and the
/// closing rule. At a width below that it is wider than asked: a
/// paragraph that draws it clips it at the area's edge, and a wrap folds
/// it onto a second row. Lay it out at the width it is drawn in.
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

    /// The rule `width` cells wide (wider only when the chip, the
    /// summary, and the closing rule alone are wider), in `theme`'s
    /// looks: the rules in `muted`, the chip in `accent` under `badge`,
    /// the summary in `inert`.
    pub fn line(&self, width: usize, theme: &Theme) -> Line<'static> {
        let rule = Style::default().add_modifier(theme.muted);
        let chip = format!(" {} ", self.label);
        let summary = self.summary.as_ref().map(|summary| format!(" {summary} "));
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
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
