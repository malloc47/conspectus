//! Bare tmux new-session form overlay (ADR 0095).
//!
//! A two-field form, name and cwd, on the framework's `Form`: it commits
//! a [`Msg::CommitMuxNew`] the runtime turns into a subprocess re-exec of
//! `conspectus mux new`. `Enter` submits from either field, and a field
//! left empty says so on the form's error line. Up and Down (and Tab and
//! S-Tab) move between the fields; Tab completes a directory in the cwd
//! field first.
//!
//! Menu-first principle: attach
//! (`a` / `Enter`) and rename (`R`) are already mux-relevant but
//! polymorphic across node kinds, so today they wouldn't shape a
//! useful mux-specific menu on their own. When a second bare-mux-
//! shape verb lands, the ADR 0095 follow-up notes an `m`-keyed
//! mux action menu will fold New + the future verb + surface
//! attach/rename for discovery. Today discoverability rides on the
//! `?` help overlay and the `n` binding in the program's keymap.

use tuikit::form::{Field, Form, Submit};
use tuikit::path_field::{Candidate, PathField};

use crate::tui::Msg;
use crate::tui::keybindings::overlay::NEW_MUX_KEYS;

/// The form's title.
const TITLE: &str = "New tmux session";
/// The name field: its id and its label.
pub const NAME: &str = "name";
/// The cwd field: its id and its label.
pub const CWD: &str = "cwd";

/// The new mux form with `name` and `cwd` filled in, offering
/// `candidates` when the cwd field completes. The focus starts on the
/// name, so the operator can type one at once; the cwd is usually the
/// selected row's and good as it is.
pub fn form(
    name: impl Into<String>,
    cwd: impl Into<String>,
    candidates: Vec<Candidate>,
) -> Form<Msg> {
    Form::new(TITLE, NEW_MUX_KEYS, |values| {
        // `mux new` takes the directory as given, so `~` is expanded here.
        Submit::Commit(Msg::CommitMuxNew {
            name: values.text(NAME).trim().to_string(),
            cwd: values
                .path(CWD)
                .map(PathField::expanded_value)
                .unwrap_or_default(),
        })
    })
    .field(Field::text(NAME, NAME, name).validate(required(NAME)))
    .field(
        Field::path(
            CWD,
            CWD,
            crate::tui::paths::directory_field(cwd, candidates),
        )
        .validate(required(CWD)),
    )
}

/// Refuses an empty `field`, naming it.
fn required(field: &'static str) -> impl Fn(&str) -> Result<(), String> + Send + Sync {
    move |text: &str| {
        if text.trim().is_empty() {
            Err(format!("mux: {field} is required"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "new_mux_tests.rs"]
mod tests;
