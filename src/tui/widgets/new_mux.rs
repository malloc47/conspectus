//! Bare tmux new-session form overlay (ADR 0095).
//!
//! A minimal two-field modal — name + cwd — that commits a
//! [`Msg::CommitMuxNew`] the runtime turns into a subprocess re-exec of
//! `conspectus mux new`. Up and Down move between the fields; Tab
//! completes a directory in the cwd field.
//!
//! Menu-first principle: attach
//! (`a` / `Enter`) and rename (`R`) are already mux-relevant but
//! polymorphic across node kinds, so today they wouldn't shape a
//! useful mux-specific menu on their own. When a second bare-mux-
//! shape verb lands, the ADR 0095 follow-up notes an `m`-keyed
//! mux action menu will fold New + the future verb + surface
//! attach/rename for discovery. Today discoverability rides on the
//! `?` help overlay and the `n` binding in the KEYBINDINGS table.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::line;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use tuikit::frame::Lines;

use crate::tui::Msg;
use crate::tui::theme::Theme;
use crate::tui::widgets::form::{self, footer_lines, input_row, path_row};
use crate::tui::widgets::path_omnibox::{PathCandidate, PathOmniboxOutcome, PathOmniboxState};
use tuikit::overlay::Outcome;
use tuikit::text_edit::LineEdit;

/// Which field currently holds the cursor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NewMuxField {
    Name,
    Cwd,
}

impl NewMuxField {
    fn toggle(self) -> Self {
        match self {
            Self::Name => Self::Cwd,
            Self::Cwd => Self::Name,
        }
    }
}

/// Overlay state: two text inputs and which one has focus. Constructor
/// takes the pre-seeded defaults from the caller (runtime derives them
/// from the selected row).
#[derive(Debug, Clone)]
pub struct NewMuxFormState {
    name: LineEdit,
    cwd: PathOmniboxState,
    focus: NewMuxField,
}

impl NewMuxFormState {
    /// New form with `name` / `cwd` pre-populated. Focus starts on the
    /// name field so operators can type a session name immediately;
    /// the cwd default is usually derived from the selected row and
    /// good as-is.
    pub fn new(name: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            name: LineEdit::new(name),
            cwd: PathOmniboxState::new(cwd),
            focus: NewMuxField::Name,
        }
    }

    /// Offer these directories when completing the cwd field.
    #[must_use]
    pub fn with_cwd_candidates(mut self, candidates: Vec<PathCandidate>) -> Self {
        self.cwd.set_known_candidates(candidates);
        self
    }

    pub fn name_value(&self) -> &str {
        self.name.value()
    }

    pub fn cwd_value(&self) -> &str {
        self.cwd.value()
    }

    pub fn focus(&self) -> NewMuxField {
        self.focus
    }

    /// Try to commit. `Enter` on the name field advances focus to cwd
    /// rather than committing (mirrors other two-field forms); `Enter`
    /// on cwd commits when both fields are non-empty. An empty field
    /// keeps the form open on that field and a toast names it. The cwd
    /// is committed with `~` expanded, since `mux new` takes it as given.
    fn try_commit(&mut self) -> Outcome<Msg> {
        let missing = |field: &str| {
            Outcome::Stay(Msg::Toast(tuikit::feedback::Feedback::warning(format!(
                "mux: {field} is required"
            ))))
        };
        match self.focus {
            NewMuxField::Name => {
                if self.name.value().trim().is_empty() {
                    return missing("name");
                }
                self.focus = NewMuxField::Cwd;
                Outcome::Consumed
            }
            NewMuxField::Cwd => {
                let name = self.name.value().trim().to_string();
                let cwd = self.cwd.expanded_value();
                if name.is_empty() {
                    self.focus = NewMuxField::Name;
                    return missing("name");
                }
                if cwd.is_empty() {
                    return missing("cwd");
                }
                Outcome::Commit(Msg::CommitMuxNew { name, cwd })
            }
        }
    }
}

impl NewMuxFormState {
    /// One key: edit, move between the fields, submit, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        // Global chords first so the line editor can't steal them.
        use crate::tui::keybindings::overlay::NEW_MUX_KEYS;
        use tuikit::keys::{FormKey, resolve};
        match resolve(NEW_MUX_KEYS, key) {
            Some(FormKey::Abort | FormKey::Cancel) => return Outcome::Close,
            Some(FormKey::Up | FormKey::Down | FormKey::Back) => {
                self.focus = self.focus.toggle();
                return Outcome::Consumed;
            }
            Some(FormKey::Tab) => {
                if self.focus == NewMuxField::Cwd
                    && matches!(
                        self.cwd.handle_key(key),
                        PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed
                    )
                {
                    return Outcome::Consumed;
                }
                self.focus = self.focus.toggle();
                return Outcome::Consumed;
            }
            Some(FormKey::Submit) => return self.try_commit(),
            Some(FormKey::OptionPrev | FormKey::OptionNext | FormKey::Inherit) | None => {}
        }
        match self.focus {
            NewMuxField::Cwd => {
                self.cwd.handle_key(key);
            }
            NewMuxField::Name => {
                self.name.handle(key);
            }
        }
        Outcome::Consumed
    }
}

/// Bordered popup rendering: the name and cwd rows and a key hint.
pub struct NewMuxFormWidget<'a> {
    state: &'a NewMuxFormState,
    theme: &'a Theme,
}

impl<'a> NewMuxFormWidget<'a> {
    pub fn new(state: &'a NewMuxFormState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for NewMuxFormWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frame = form::modal_frame(self.theme, "New tmux session");
        let inner_width = usize::from(frame.inner_width(area));
        let cols = form::Cols::new(inner_width);
        let name_focused = self.state.focus == NewMuxField::Name;
        let cwd_focused = self.state.focus == NewMuxField::Cwd;
        let mut lines: Vec<Line<'static>> = vec![
            input_row("name", &self.state.name, name_focused, cols, self.theme),
            path_row("cwd", &self.state.cwd, cwd_focused, cols, self.theme),
            line![""],
        ];
        lines.extend(footer_lines(
            &tuikit::keys::footer(crate::tui::keybindings::overlay::NEW_MUX_KEYS),
            inner_width,
            self.theme,
        ));
        let cursor = usize::from(cwd_focused);
        frame.render(area, buf, Lines::new(lines).following(cursor..cursor + 1));
    }
}

#[cfg(test)]
#[path = "new_mux_tests.rs"]
mod tests;
