//! The property row: a label in the label column, then the value.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::Cut;
use super::grid::{Intent, Layout};
use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// One property of the thing a grid shows: a label in the grid's label
/// column, an optional glyph, the value, a hint, and an annotation.
///
/// The value is text (one style) or spans the host styled. A text value
/// can be cut to a number of cells ([`PropertyRow::cut`]); the row then
/// draws the cut value, says it was cut with a hint naming the grid's
/// inspect key, and [`PropertyRow::inspect`] hands the whole value to the
/// host. Uncut, a long value is drawn whole and a host that wraps the
/// grid's lines wraps it.
///
/// ```
/// use tuikit::property_grid::{Cut, Intent, PropertyRow};
///
/// let row = PropertyRow::new("path", "/srv/archive/2026/report.txt").cut(Cut::Middle, 15);
/// assert_eq!(row.shown(), "/srv/ar…ort.txt");
/// assert!(row.is_cut());
/// assert_eq!(
///     row.activate(),
///     Some(Intent::Copy { label: "path".into(), text: "/srv/archive/2026/report.txt".into() }),
///     "a copy takes the whole value, not the cut one"
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    label: String,
    value: Value,
    cut: Option<(Cut, usize)>,
    placeholder: bool,
    hint: Option<String>,
    annotation: Option<String>,
    glyph: Option<Span<'static>>,
}

/// What a property row holds as its value.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    /// One run of text in one style.
    Text { text: String, style: Style },
    /// Spans the host styled.
    Spans(Vec<Span<'static>>),
}

impl PropertyRow {
    /// A row whose value is `value` as plain text.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_value(
            label,
            Value::Text {
                text: value.into(),
                style: Style::default(),
            },
        )
    }

    /// A row whose value is `spans`, drawn in the styles they carry. A
    /// cut ([`PropertyRow::cut`]) takes their end off, whatever its kind.
    pub fn spans(label: impl Into<String>, spans: Vec<Span<'static>>) -> Self {
        Self::with_value(label, Value::Spans(spans))
    }

    fn with_value(label: impl Into<String>, value: Value) -> Self {
        Self {
            label: label.into(),
            value,
            cut: None,
            placeholder: false,
            hint: None,
            annotation: None,
            glyph: None,
        }
    }

    /// Draws a text value in `style` (a value given as spans has its
    /// spans patched with it).
    pub fn style(mut self, style: Style) -> Self {
        match &mut self.value {
            Value::Text { style: own, .. } => *own = own.patch(style),
            Value::Spans(spans) => {
                for span in spans {
                    span.style = span.style.patch(style);
                }
            }
        }
        self
    }

    /// Shows at most `cells` of the value, cut `how` with the ellipsis
    /// mark. A value that fits is drawn whole and is not cut.
    pub fn cut(mut self, how: Cut, cells: usize) -> Self {
        self.cut = Some((how, cells));
        self
    }

    /// Marks the value as a stand-in for one the row does not have (the
    /// no-value mark, an "unknown"): it recedes, in the theme's `muted`
    /// look, and there is nothing to copy.
    pub fn placeholder(mut self) -> Self {
        self.placeholder = true;
        self
    }

    /// A note after the value, receding. It takes the place of the
    /// truncated hint a cut value draws.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// A short note at the end of the row that needs attention, in the
    /// theme's `warning` color.
    pub fn annotation(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }

    /// A glyph between the label and the value, in the style it carries:
    /// what kind of thing the value names. It stays beside the label when
    /// a long value wraps.
    pub fn glyph(mut self, glyph: Span<'static>) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// The label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The whole value as text.
    pub fn text(&self) -> String {
        match &self.value {
            Value::Text { text, .. } => text.clone(),
            Value::Spans(spans) => spans.iter().map(|span| span.content.as_ref()).collect(),
        }
    }

    /// The value as the row draws it: cut when [`PropertyRow::cut`] asked
    /// for fewer cells than it takes.
    pub fn shown(&self) -> String {
        self.shown_spans()
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    /// Whether the row draws less than its whole value.
    pub fn is_cut(&self) -> bool {
        self.shown() != self.text()
    }

    /// Whether the row stands in for a value it does not have.
    pub fn is_placeholder(&self) -> bool {
        self.placeholder
    }

    /// What `Enter` on the row asks for: a copy of the whole value, or
    /// nothing for an empty value or a placeholder.
    pub fn activate(&self) -> Option<Intent> {
        let text = self.text();
        (!self.placeholder && !text.is_empty()).then(|| Intent::Copy {
            label: self.label.clone(),
            text,
        })
    }

    /// What the inspect key on the row asks for: the whole value in a
    /// reader when the row shows it cut, else nothing.
    pub fn inspect(&self) -> Option<Intent> {
        self.is_cut().then(|| Intent::Inspect {
            label: self.label.clone(),
            text: self.text(),
        })
    }

    /// The value's spans as drawn, before the placeholder look.
    fn shown_spans(&self) -> Vec<Span<'static>> {
        match (&self.value, self.cut) {
            (Value::Text { text, style }, cut) => {
                let shown = match cut {
                    None => text.clone(),
                    Some((Cut::End, cells)) => text::truncate_end(text, cells),
                    Some((Cut::Middle, cells)) => text::truncate_middle(text, cells),
                    Some((Cut::Clip, cells)) => text::clip(text, cells),
                };
                vec![Span::styled(shown, *style)]
            }
            (Value::Spans(spans), None) => spans.clone(),
            (Value::Spans(spans), Some((_, cells))) => text::fit_spans(spans.clone(), cells),
        }
    }

    /// The row's spans after the cursor gutter.
    pub(super) fn draw(&self, layout: &Layout<'_>, theme: &Theme) -> Vec<Span<'static>> {
        let label_cells = layout.label_width.max(text::width(&self.label) + 1);
        let mut spans = vec![Span::styled(
            text::pad(&self.label, label_cells),
            Style::default().add_modifier(Modifier::BOLD),
        )];
        if let Some(glyph) = &self.glyph {
            spans.push(glyph.clone());
            spans.push(Span::raw(" "));
        }
        let mut value = self.shown_spans();
        if self.placeholder {
            for span in &mut value {
                span.style = span.style.add_modifier(theme.muted);
            }
        }
        spans.extend(value);
        let hint = match (&self.hint, self.is_cut(), layout.inspect_key) {
            (Some(hint), _, _) => Some(hint.clone()),
            (None, true, Some(key)) => Some(format!("(truncated {} {key})", marks::SEPARATOR)),
            (None, true, None) => Some("(truncated)".to_string()),
            (None, false, _) => None,
        };
        if let Some(hint) = hint {
            spans.push(Span::styled(
                format!("  {hint}"),
                Style::default().add_modifier(theme.muted),
            ));
        }
        if let Some(annotation) = &self.annotation {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                annotation.clone(),
                Style::default().fg(theme.warning),
            ));
        }
        spans
    }
}
