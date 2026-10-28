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

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::form::{
    self, ScrollLinesBody, input_row, path_row, scroll_offset_for_cursor, wrap_footer,
};
use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::path_omnibox::{PathCandidate, PathOmniboxOutcome, PathOmniboxState};

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
    name: TextInputState,
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
            name: TextInputState::new("name", name),
            cwd: PathOmniboxState::new("cwd", cwd),
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
    fn try_commit(&mut self) -> OverlayOutcome {
        let missing = |field: &str| {
            OverlayOutcome::CommitAndStay(Box::new(Msg::Toast(format!("mux: {field} is required"))))
        };
        match self.focus {
            NewMuxField::Name => {
                if self.name.value().trim().is_empty() {
                    return missing("name");
                }
                self.focus = NewMuxField::Cwd;
                OverlayOutcome::Consumed
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
                OverlayOutcome::Commit(Box::new(Msg::CommitMuxNew { name, cwd }))
            }
        }
    }
}

impl Overlay for NewMuxFormState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        // Global chords first so the line editor can't steal them.
        use crate::tui::keybindings::overlay::{FormKey, NEW_MUX_KEYS, resolve};
        match resolve(NEW_MUX_KEYS, key) {
            Some(FormKey::Abort | FormKey::Cancel) => return OverlayOutcome::Close,
            Some(FormKey::Up | FormKey::Down | FormKey::Back) => {
                self.focus = self.focus.toggle();
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Tab) => {
                if self.focus == NewMuxField::Cwd
                    && matches!(
                        self.cwd.handle_key(key),
                        PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed
                    )
                {
                    return OverlayOutcome::Consumed;
                }
                self.focus = self.focus.toggle();
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Submit) => return self.try_commit(),
            Some(FormKey::OptionPrev | FormKey::OptionNext | FormKey::Inherit) | None => {}
        }
        match self.focus {
            NewMuxField::Cwd => {
                self.cwd.handle_key(key);
                OverlayOutcome::Consumed
            }
            NewMuxField::Name => match self.name.handle_key(key) {
                InputOutcome::Continue => OverlayOutcome::Consumed,
                InputOutcome::Cancel => OverlayOutcome::Close,
                // Enter is intercepted above; kept for safety.
                InputOutcome::Confirm(_) => self.try_commit(),
            },
        }
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
        let modal_width = 76;
        let inner_width =
            usize::from(std::cmp::min(modal_width, area.width.saturating_sub(4)).max(44)) - 2;
        let cols = form::Cols::new(inner_width);
        let name_focused = self.state.focus == NewMuxField::Name;
        let cwd_focused = self.state.focus == NewMuxField::Cwd;
        let mut lines: Vec<Line<'static>> = vec![
            input_row("name", &self.state.name, name_focused, cols, self.theme),
            path_row("cwd", &self.state.cwd, cwd_focused, cols, self.theme),
            line![""],
        ];
        lines.extend(wrap_footer(
            &crate::tui::keybindings::overlay::footer(
                crate::tui::keybindings::overlay::NEW_MUX_KEYS,
            ),
            inner_width,
            self.theme,
        ));
        let modal = form::modal_rect(area, modal_width, lines.len(), 6);
        let inner_height = usize::from(modal.height.saturating_sub(2));
        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                usize::from(cwd_focused),
                inner_height,
                lines.len(),
            ),
            lines,
            inner_width: usize::from(modal.width.saturating_sub(2)),
            inner_height,
        };
        crate::tui::widgets::popup_frame::themed_popup(
            body,
            line![" New tmux session "],
            self.theme,
        )
        .render(area, buf);
    }
}

#[cfg(test)]
#[path = "new_mux_tests.rs"]
mod tests;
