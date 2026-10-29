//! The host's side of the form row kit, for the one form that still lays
//! its rows out itself: the pin edit form. The new mux and launch forms
//! are `tuikit::form::Form`s.
//!
//! The rows are the framework's (`tuikit::form`): a text value with a
//! cursor and scroll marks, a path field with its validity mark and Tab
//! hint, the harness choices as chips, checkbox rows, notes, the fixed
//! footer, and the body whose footer never scrolls away. What stays here
//! is what only Conspectus knows (a harness's launch options as
//! checkboxes) and the frame the forms share.

use ratatui::text::{Line, Span};
use tuikit::frame::ModalFrame;

use crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment};
use crate::tui::Theme;

pub use tuikit::form::{Cols, footer_lines, path_cells, row, text_cells as value_cells};

/// The label that starts a row, in the label column, and the gap before
/// the value. The cursor gutter goes before it ([`row`]).
pub fn prefix(label: &str, label_width: usize) -> String {
    tuikit::form::label(label, label_width)
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

/// A dimmed, read-only row, such as the pin a form edits.
pub fn note_row(label: &str, value: &str, cols: Cols, theme: &Theme) -> Line<'static> {
    tuikit::form::read_only_row(label, value, tuikit::form::Tone::Note, cols, theme)
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
