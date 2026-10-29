//! The host's side of the form row kit, for the forms that still lay
//! their rows out themselves: the mux launch form and the pin edit form.
//! The new mux form is a `tuikit::form::Form`.
//!
//! The rows are the framework's (`tuikit::form`): a text value with a
//! cursor and scroll marks, a path field with its validity mark and Tab
//! hint, the harness choices as a row of chips, checkbox and one-of-N
//! rows, notes and derived lines, the fixed footer, and the body whose
//! footer never scrolls away. What stays here is what only Conspectus
//! knows (a harness's launch options as checkboxes, the argv an empty
//! argv field means, how an argv reads back) and the two layouts the
//! framework's `Form` does not have yet: a checkbox sharing the name row,
//! and the pin edit form's tag column.

use ratatui::text::{Line, Span};
use tuikit::frame::ModalFrame;
use tuikit::selection::{self, Cursor};
use tuikit::text_edit::LineEdit;

use crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment};
use crate::tui::Theme;

pub use tuikit::form::{
    Cols, Tone, checkbox, field_style as entry_style, footer_lines, path_cells, path_row, row,
    text_cells as value_cells, text_row as input_row, toggle_row,
};

/// The label column of the forms whose labels are short.
pub const LABEL_WIDTH: usize = tuikit::form::LABEL_MIN;

/// Spans drawn after a row's value, and the width they take.
pub struct Suffix {
    pub width: usize,
    pub spans: Vec<Span<'static>>,
}

/// The label that starts a row, in the label column, and the gap before
/// the value. The cursor gutter goes before it ([`row`]).
pub fn prefix(label: &str, label_width: usize) -> String {
    tuikit::form::label(label, label_width)
}

/// `line`, a row drawn without the cursor, given the cursor's marker and
/// highlight: for a row whose focus is on a part other than its field.
pub fn with_cursor(mut line: Line<'static>, theme: &Theme) -> Line<'static> {
    if let Some(gutter) = line.spans.first_mut() {
        *gutter = selection::gutter(Cursor::Active);
    }
    selection::highlight(&mut line.spans, Cursor::Active, theme);
    line
}

/// A text value row with `suffix` after the value: the name row and the
/// socket checkbox that shares it.
pub fn value_row(
    label: &str,
    edit: &LineEdit,
    active: bool,
    cols: Cols,
    suffix: Option<Suffix>,
    theme: &Theme,
) -> Line<'static> {
    let suffix_width = suffix.as_ref().map_or(0, |s| s.width);
    let value_width = cols.value().saturating_sub(suffix_width).max(1);
    let mut spans = vec![Span::raw(prefix(label, cols.label))];
    spans.extend(value_cells(edit, value_width, active, theme));
    if let Some(suffix) = suffix {
        spans.extend(suffix.spans);
    }
    row(spans, active, theme)
}

/// The harness field: its value with a chip for every known harness, the
/// selected one highlighted, or a `[custom]` mark for a value that isn't
/// one of them (the framework's choice row).
pub fn harness_row(
    label: &str,
    edit: &LineEdit,
    known: &[String],
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    tuikit::form::choice_row(label, edit, known, active, cols, theme)
}

/// A chip for each known harness that fits in `width` cells.
pub fn harness_chips(
    selected: &str,
    known: &[String],
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    tuikit::form::chips(selected, known, width, theme)
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
    let items: Vec<(String, bool)> = options
        .iter()
        .map(|option| {
            (
                option.label.to_string(),
                argv_contains_fragment(effective_argv, option.argv),
            )
        })
        .collect();
    tuikit::form::check_row(label, &items, None, active, cols, theme)
}

/// One-of-N choices, the chosen one marked: `(•) a ( ) b`.
pub fn choice_row(
    label: &str,
    choices: &[(&str, bool)],
    active: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let options: Vec<String> = choices
        .iter()
        .map(|(text, _)| (*text).to_string())
        .collect();
    let chosen = choices
        .iter()
        .position(|(_, selected)| *selected)
        .unwrap_or(0);
    tuikit::form::one_of_row(label, &options, chosen, active, cols, theme)
}

/// A dimmed, read-only row, such as the pin a form edits.
pub fn note_row(label: &str, value: &str, cols: Cols, theme: &Theme) -> Line<'static> {
    tuikit::form::read_only_row(label, value, Tone::Note, cols, theme)
}

/// A line derived from the fields above it, such as the command the form
/// will run: led in by the dim corner two cells deeper than the labels,
/// with no focus marker, so it reads as output and not as another input;
/// in the warning color when `warn`.
pub fn derived_row(
    label: &str,
    value: &str,
    warn: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let tone = if warn { Tone::Warning } else { Tone::Derived };
    tuikit::form::read_only_row(label, value, tone, cols, theme)
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

/// The frame every form draws in: the form size, with `title`. A form
/// lays its rows out for the frame's
/// [`inner_width`](ModalFrame::inner_width) and draws them as the body,
/// a `tuikit::form::FormBody`, so its footer stays in view.
pub fn modal_frame<'a>(theme: &'a Theme, title: &'a str) -> ModalFrame<'a> {
    ModalFrame::new(theme, tuikit::form::FORM_SIZE).title(title)
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
