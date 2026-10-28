//! Row rendering shared by the multi-field forms: the mux launch form,
//! the new mux form, and the pin edit form.
//!
//! Every form draws the same kinds of rows: a text value with a cursor
//! and scroll indicators, a path field with a validity mark and a Tab
//! completion hint, the harness choices as a row of chips, a row of
//! launch option checkboxes, and one-of-N choice rows. Sharing them keeps
//! the forms looking and behaving alike.
//!
//! Every row takes the theme: a field's value is drawn in the `field`
//! token, a focused field's in `field_active` with the cursor cell in
//! `field_cursor`, scroll markers and hints in `accent`, notes and
//! footers `muted`, and path marks in `success`, `warning`, and `inert`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;
use tuikit::glyphs::marks;
use tuikit::text;

use crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment};
use crate::tui::Theme;
use crate::tui::widgets::input::TextInputState;
use crate::tui::widgets::path_omnibox::{PathOmniboxState, PathValidation};

pub const LABEL_WIDTH: usize = 11;

/// Cells the harness value gets before the chips.
const VALUE_SLOT_WIDTH: usize = 18;

/// The widths a row is drawn into: the popup's inner width and the
/// label column.
#[derive(Debug, Clone, Copy)]
pub struct Cols {
    pub inner: usize,
    pub label: usize,
}

impl Cols {
    pub fn new(inner: usize) -> Self {
        Self {
            inner,
            label: LABEL_WIDTH,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: usize) -> Self {
        self.label = label;
        self
    }
}
const VALUE_GAP: &str = "  ";
const TEXT_GUTTER: &str = "  ";
const PATH_SUFFIX_WIDTH: usize = 32;

/// Spans drawn after a row's value, and the width they take.
pub struct Suffix {
    pub width: usize,
    pub spans: Vec<Span<'static>>,
}

/// The marker and label that start a row, bold when the cursor is on it.
pub fn prefix(label: &str, active: bool, label_width: usize) -> (String, Style) {
    let marker = if active { "> " } else { "  " };
    let style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    (format!("{marker}{label:label_width$}{VALUE_GAP}"), style)
}

/// A field's value: filled when the field has focus, plain otherwise.
pub fn entry_style(active: bool, theme: &Theme) -> Style {
    if active {
        theme.field_active.into_style()
    } else {
        theme.field.into_style()
    }
}

fn edge_style(indicator: &str, edge: Style) -> Style {
    if indicator.trim().is_empty() {
        Style::default()
    } else {
        edge
    }
}

struct Window {
    left: &'static str,
    before: String,
    cursor_text: String,
    after: String,
    right: &'static str,
    edge: Style,
    cursor_style: Style,
}

/// The part of `value` that fits in `width` cells around the cursor,
/// with `<` and `>` marking what scrolled out of view.
///
/// The result is always exactly `width` cells wide, focused or not: the
/// cell the cursor sits on exists either way, so moving the focus never
/// shifts what follows the value.
fn window(value: &str, cursor: usize, width: usize, active: bool, theme: &Theme) -> Window {
    let width = width.max(1);
    // The scroll markers sit on the field's own fill, in the accent.
    let edge = entry_style(active, theme)
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let cursor_style = if active {
        theme.field_cursor.into_style()
    } else {
        Style::default()
    };
    let value = if value.trim().is_empty() { "-" } else { value };
    // The characters, plus the blank cell a cursor at the end sits on.
    let mut cells: Vec<char> = value.chars().collect();
    let total = cells.len();
    cells.push(' ');
    let cursor = cursor.min(total);

    if width <= 4 {
        let ch = cells[cursor];
        return Window {
            left: "",
            before: String::new(),
            cursor_text: ch.to_string(),
            after: " ".repeat(width.saturating_sub(1)),
            right: "",
            edge,
            cursor_style,
        };
    }

    let content_width = width - 4;
    let start = if cells.len() <= content_width || cursor < content_width {
        0
    } else {
        (cursor + 1 - content_width).min(cells.len() - content_width)
    };
    let end = (start + content_width).min(cells.len());
    let before: String = cells[start..cursor].iter().collect();
    let cursor_text = cells[cursor].to_string();
    let mut after: String = cells[cursor + 1..end].iter().collect();
    let used = before.chars().count() + 1 + after.chars().count();
    after.push_str(&" ".repeat(content_width.saturating_sub(used)));
    Window {
        left: if start > 0 { "< " } else { "  " },
        before,
        cursor_text,
        after,
        right: if end < total { " >" } else { "  " },
        edge,
        cursor_style,
    }
}

/// `value` windowed around `cursor` into exactly `width` cells.
pub fn value_cells(
    value: &str,
    cursor: usize,
    width: usize,
    active: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    window_spans(
        &window(value, cursor, width, active, theme),
        entry_style(active, theme),
    )
}

fn window_spans(w: &Window, value_style: Style) -> Vec<Span<'static>> {
    vec![
        Span::styled(w.left.to_string(), edge_style(w.left, w.edge)),
        Span::styled(w.before.clone(), value_style),
        Span::styled(w.cursor_text.clone(), w.cursor_style),
        Span::styled(w.after.clone(), value_style),
        Span::styled(w.right.to_string(), edge_style(w.right, w.edge)),
    ]
}

/// A text value row, with an optional suffix after the value.
pub fn value_row(
    label: &str,
    value: &str,
    value_cursor: usize,
    active: bool,
    cols: Cols,
    suffix: Option<Suffix>,
    theme: &Theme,
) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let suffix_width = suffix.as_ref().map_or(0, |s| s.width);
    let value_width = inner_width
        .saturating_sub(text::width(&prefix))
        .saturating_sub(suffix_width)
        .max(1);
    let w = window(value, value_cursor, value_width, active, theme);
    let mut spans = vec![Span::styled(prefix, label_style)];
    spans.extend(window_spans(&w, entry_style(active, theme)));
    if let Some(suffix) = suffix {
        spans.extend(suffix.spans);
    }
    Line::from(spans)
}

pub fn input_row(
    label: &str,
    input: &TextInputState,
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    value_row(
        label,
        input.value(),
        input.cursor(),
        active,
        cols,
        None,
        theme,
    )
}

/// A text row followed by a dimmed note, such as where an inherited
/// value came from.
pub fn tagged_input_row(
    label: &str,
    input: &TextInputState,
    tag: &str,
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let text = format!(" {tag}");
    let width = tuikit::text::width(&text);
    value_row(
        label,
        input.value(),
        input.cursor(),
        active,
        cols,
        Some(Suffix {
            width,
            spans: vec![Span::styled(
                text,
                Style::default().add_modifier(theme.muted),
            )],
        }),
        theme,
    )
}

/// A path field: the value, a mark for whether it exists, and the
/// remainder Tab would complete.
pub fn path_row(
    label: &str,
    omnibox: &PathOmniboxState,
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    value_row(
        label,
        omnibox.value(),
        omnibox.cursor(),
        active,
        cols,
        Some(path_suffix(omnibox, active, theme)),
        theme,
    )
}

/// The validity mark of a path value: `√` when it exists, `!` when it is
/// missing, `?` when it is empty.
fn path_symbol(omnibox: &PathOmniboxState, theme: &Theme) -> (&'static str, Style) {
    match omnibox.validation() {
        PathValidation::Empty => ("?", Style::default().fg(theme.inert)),
        PathValidation::Exists => (marks::OK, Style::default().fg(theme.success)),
        PathValidation::Missing => ("!", Style::default().fg(theme.warning)),
    }
}

fn path_suffix(omnibox: &PathOmniboxState, focused: bool, theme: &Theme) -> Suffix {
    let (symbol, style) = path_symbol(omnibox, theme);
    let mut spans = vec![Span::raw(" "), Span::styled(symbol, style)];
    let used = 1 + text::width(symbol);
    let remaining = PATH_SUFFIX_WIDTH.saturating_sub(used);
    if remaining > 1 {
        if let Some(hint) = completion_hint(omnibox, focused) {
            let hint = text::truncate_end(&hint, remaining.saturating_sub(1));
            let used = used + 1 + text::width(&hint);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(hint, Style::default().fg(theme.accent)));
            spans.push(Span::raw(
                " ".repeat(PATH_SUFFIX_WIDTH.saturating_sub(used)),
            ));
        } else {
            spans.push(Span::raw(" ".repeat(remaining)));
        }
    }
    Suffix {
        width: PATH_SUFFIX_WIDTH,
        spans,
    }
}

/// The validity mark after a path value: two cells, a space and `√`
/// (exists), `!` (missing) or `?` (empty).
pub fn path_mark(omnibox: &PathOmniboxState, theme: &Theme) -> Vec<Span<'static>> {
    let (symbol, style) = path_symbol(omnibox, theme);
    vec![Span::raw(" "), Span::styled(symbol, style)]
}

pub fn completion_hint(omnibox: &PathOmniboxState, focused: bool) -> Option<String> {
    if !focused {
        return None;
    }
    let value = omnibox.value().trim();
    omnibox
        .suggestions()
        .into_iter()
        .find(|suggestion| suggestion.path.as_str() != value)
        .and_then(|suggestion| completion_remainder(value, &suggestion.path))
        .map(|remainder| format!("Tab: {remainder}"))
}

/// What typing `suggestion` would add to `typed`.
pub fn completion_remainder(typed: &str, suggestion: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Some(suggestion.to_string());
    }
    suggestion
        .strip_prefix(typed)
        .filter(|remainder| !remainder.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let typed_basename = typed.rsplit('/').next().unwrap_or(typed);
            let suggestion_basename = suggestion.rsplit('/').next().unwrap_or(suggestion);
            suggestion_basename
                .strip_prefix(typed_basename)
                .filter(|remainder| !remainder.is_empty())
                .map(str::to_string)
        })
}

/// The harness field: its value with a chip for every known harness,
/// the selected one highlighted, or a `[custom]` mark for a value that
/// isn't one of them.
pub fn harness_row(
    label: &str,
    input: &TextInputState,
    known: &[String],
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let available = inner_width.saturating_sub(text::width(&prefix));
    let value = input.value().trim();
    if !value.is_empty() && !known.iter().any(|k| k == value) {
        let suffix = " [custom]";
        let value_width = available.saturating_sub(suffix.len()).max(1);
        let w = window(input.value(), input.cursor(), value_width, active, theme);
        let mut spans = vec![Span::styled(prefix, label_style)];
        spans.extend(window_spans(&w, entry_style(active, theme)));
        spans.push(Span::styled(suffix, Style::default().fg(theme.warning)));
        return Line::from(spans);
    }
    let value_width = VALUE_SLOT_WIDTH.min(available).max(1);
    let chips_budget = available.saturating_sub(value_width).min(48);
    let w = window(input.value(), input.cursor(), value_width, active, theme);
    let shown = w.left.chars().count()
        + w.before.chars().count()
        + w.cursor_text.chars().count()
        + w.after.chars().count()
        + w.right.chars().count();
    let padding = value_width.saturating_sub(shown);
    let style = entry_style(active, theme);
    let mut spans = vec![Span::styled(prefix, label_style)];
    spans.extend(window_spans(&w, style));
    if padding > 0 {
        spans.push(Span::styled(" ".repeat(padding), style));
    }
    spans.extend(harness_chips(value, known, chips_budget, theme));
    Line::from(spans)
}

pub fn harness_chips(
    selected: &str,
    known: &[String],
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    if width < 3 {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut remaining = width;
    for key in known {
        let token = format!(" [{key}]");
        let needed = token.chars().count();
        if needed > remaining {
            if remaining >= 2 {
                spans.push(Span::styled(
                    " …",
                    Style::default().add_modifier(theme.muted),
                ));
            }
            break;
        }
        if key == selected {
            spans.push(Span::styled(
                token,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                token,
                Style::default().add_modifier(theme.muted),
            ));
        }
        remaining -= needed;
    }
    spans
}

/// The harness's launch options as checkboxes, checked when the
/// effective argv already carries the option's flags.
pub fn options_row(
    label: &str,
    options: &[HarnessLaunchOption],
    effective_argv: &[String],
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let value = options
        .iter()
        .map(|option| {
            checkbox(
                option.label,
                argv_contains_fragment(effective_argv, option.argv),
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    static_value_row(label, &value, active, cols, theme)
}

/// One-of-N choices: `[x] a  [ ] b`.
pub fn choice_row(
    label: &str,
    choices: &[(&str, bool)],
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let value = choices
        .iter()
        .map(|(text, selected)| checkbox(text, *selected))
        .collect::<Vec<_>>()
        .join(" ");
    static_value_row(label, &value, active, cols, theme)
}

/// A row of text that isn't edited in place (a toggle, a choice).
pub fn static_value_row(
    label: &str,
    value: &str,
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let width = inner_width
        .saturating_sub(text::width(&prefix))
        .saturating_sub(text::width(TEXT_GUTTER));
    Line::from(vec![
        Span::styled(prefix, label_style),
        Span::raw(TEXT_GUTTER),
        Span::styled(text::truncate_end(value, width), entry_style(active, theme)),
    ])
}

/// A dimmed, read-only row, such as the command a form will run.
pub fn note_row(label: &str, value: &str, cols: Cols, theme: &Theme) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, _) = prefix(label, false, cols.label);
    let width = inner_width
        .saturating_sub(text::width(&prefix))
        .saturating_sub(text::width(TEXT_GUTTER))
        .max(1);
    let note = Style::default().add_modifier(theme.muted | Modifier::ITALIC);
    Line::from(vec![
        Span::styled(prefix, note),
        Span::raw(TEXT_GUTTER),
        Span::styled(text::truncate_end(value, width), note),
    ])
}

/// A line derived from the fields above it, such as the command the form
/// will run. It sits two cells in with a dim `└` lead-in, in dim italic,
/// and carries no focus marker, so it reads as output and not as another
/// input. The value starts in the same column as the field values.
pub fn derived_row(
    label: &str,
    value: &str,
    warn: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let dim = Style::default().add_modifier(theme.muted | Modifier::ITALIC);
    let lead = format!(
        "  └ {label:width$}{VALUE_GAP}",
        width = cols.label.saturating_sub(2)
    );
    let width = cols
        .inner
        .saturating_sub(text::width(&lead))
        .saturating_sub(text::width(TEXT_GUTTER))
        .max(1);
    let value_style = if warn {
        Style::default()
            .fg(theme.warning)
            .add_modifier(Modifier::ITALIC)
    } else {
        dim
    };
    Line::from(vec![
        Span::styled(lead, dim),
        Span::raw(TEXT_GUTTER),
        Span::styled(text::truncate_end(value, width), value_style),
    ])
}

pub fn warning_row(label: &str, value: &str, cols: Cols, theme: &Theme) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, _) = prefix(label, false, cols.label);
    let width = inner_width
        .saturating_sub(text::width(&prefix))
        .saturating_sub(text::width(TEXT_GUTTER))
        .max(1);
    Line::from(vec![
        Span::styled(prefix, Style::default().add_modifier(theme.muted)),
        Span::raw(TEXT_GUTTER),
        Span::styled(
            text::truncate_end(value, width),
            Style::default().fg(theme.warning),
        ),
    ])
}

/// A key hint footer as muted lines that fit `width` cells, wrapped only
/// between hints so a key stays beside its verb.
pub fn footer_lines(footer: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let dim = Style::default().add_modifier(theme.muted);
    text::wrap_at(
        footer,
        crate::tui::keybindings::overlay::HINT_SEPARATOR,
        width,
    )
    .into_iter()
    .map(|row| Line::styled(row, dim))
    .collect()
}

/// The argv an empty argv field means for `harness`: a launch profile or
/// `atelier exec` when configured, else the harness default.
pub fn default_argv(
    defaults: Option<&crate::launch_default::LaunchDefaults>,
    harness: &str,
    cwd: &str,
) -> Vec<String> {
    let cwd = cwd.trim();
    let argv = defaults
        .and_then(|d| {
            d.default_argv(
                harness,
                (!cwd.is_empty()).then(|| std::path::Path::new(cwd)),
            )
        })
        .map_or_else(
            || crate::discovery::harness::launch_argv_for(harness),
            |(argv, _)| argv,
        );
    argv.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

pub fn checkbox(label: &str, selected: bool) -> String {
    if selected {
        format!("[x] {label}")
    } else {
        format!("[ ] {label}")
    }
}

/// An argv shown the way a shell would take it back.
pub fn display_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_display_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_display_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '=' | '+')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}

/// A centered popup rect sized for `content_lines` rows.
pub fn modal_rect(area: Rect, width_cap: u16, content_lines: usize, min_height: u16) -> Rect {
    let width = std::cmp::min(width_cap, area.width.saturating_sub(4)).max(44);
    let desired = u16::try_from(content_lines.saturating_add(2)).unwrap_or(u16::MAX);
    let height = desired.clamp(min_height, area.height.max(min_height));
    super::popup_frame::centered_rect(area, width, height)
}

/// Popup body that scrolls so the cursor row stays visible.
pub struct ScrollLinesBody {
    pub lines: Vec<Line<'static>>,
    pub inner_width: usize,
    pub inner_height: usize,
    pub scroll_offset: u16,
}

impl KnownSize for ScrollLinesBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for ScrollLinesBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines)
            .scroll((self.scroll_offset, 0))
            .render(area, buf);
    }
}

/// Popup body in three bands: lines that stay put at the top and bottom,
/// and between them rows that scroll to keep the cursor row visible.
/// The fixed bands keep their place however many rows there are.
pub struct SplitBody {
    pub top: Vec<Line<'static>>,
    pub rows: Vec<Line<'static>>,
    pub bottom: Vec<Line<'static>>,
    pub cursor_row: usize,
    pub inner_width: usize,
    pub inner_height: usize,
}

impl KnownSize for SplitBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for SplitBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = usize::from(area.height);
        let bottom = self.bottom.len().min(height);
        let top = self.top.len().min(height - bottom);
        let rows_height = height - bottom - top;
        let offset = scroll_offset_for_cursor(self.cursor_row, rows_height, self.rows.len());
        let line_at = |y: usize| {
            Rect::new(
                area.x,
                area.y + u16::try_from(y).unwrap_or(0),
                area.width,
                1,
            )
        };
        for (i, line) in self.top.into_iter().take(top).enumerate() {
            Paragraph::new(line).render(line_at(i), buf);
        }
        for (i, line) in self
            .rows
            .into_iter()
            .skip(usize::from(offset))
            .take(rows_height)
            .enumerate()
        {
            Paragraph::new(line).render(line_at(top + i), buf);
        }
        let first = height - bottom;
        for (i, line) in self.bottom.into_iter().take(bottom).enumerate() {
            Paragraph::new(line).render(line_at(first + i), buf);
        }
    }
}

/// How far to scroll so line `cursor_line` is the last one visible.
pub fn scroll_offset_for_cursor(
    cursor_line: usize,
    inner_height: usize,
    content_height: usize,
) -> u16 {
    if inner_height == 0 || cursor_line < inner_height {
        return 0;
    }
    let max_scroll = content_height.saturating_sub(inner_height);
    let offset = cursor_line
        .saturating_sub(inner_height.saturating_sub(1))
        .min(max_scroll);
    u16::try_from(offset).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
