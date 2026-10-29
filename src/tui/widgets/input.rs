//! The modal text prompt, per ADR 0030: one line of text in a compact
//! frame titled with what it asks for.
//!
//! The rename prompts, the worktree branch prompt, and the Controls
//! filter editors share it. The editing is the framework's line editor
//! (`tuikit::text_edit::LineEdit`) and the line is drawn by its text
//! field, so the prompt edits and windows text like every other field;
//! what this module adds is the prompt's contract: `Enter` confirms,
//! `Esc` cancels, `Ctrl-C` dismisses, and `Tab` is swallowed.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::frame::{ModalBody, ModalFrame, ModalSize};
use tuikit::text_edit::{LineEdit, TextField};

use crate::tui::Theme;

/// What the host should do after passing a key event through the
/// prompt. `Continue` means the prompt stays open; `Confirm` carries
/// the buffer back to the host as typed; `Cancel` (`Esc`) means the
/// prompt closes without committing, stepping back one level when it
/// sits inside another overlay; `Abort` (`Ctrl-C`) dismisses the whole
/// overlay the prompt is in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputOutcome {
    Continue,
    Confirm(String),
    Cancel,
    Abort,
}

/// Caller-owned state for one text prompt: the line being edited and
/// the title that renders as the modal's border label.
#[derive(Debug, Clone)]
pub struct TextPromptState {
    title: String,
    edit: LineEdit,
}

impl TextPromptState {
    /// A prompt pre-populated with `initial`. The cursor lands at the
    /// end so typing appends naturally: the rename flow wants the caller
    /// to be able to edit an existing alias or harness title verbatim.
    pub fn new(title: impl Into<String>, initial: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            edit: LineEdit::new(initial),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn value(&self) -> &str {
        self.edit.value()
    }

    pub fn cursor(&self) -> usize {
        self.edit.cursor()
    }

    /// The line being edited.
    pub fn edit(&self) -> &LineEdit {
        &self.edit
    }

    /// Pass a key press through the prompt.
    ///
    /// `Enter` returns `Confirm(value)`; `Esc` returns `Cancel`;
    /// `Ctrl-C` returns `Abort`; every other key goes to the line
    /// editor. ADR 0030 locks the surrounding `Tab` focus cycle, so
    /// `Tab` is swallowed here.
    pub fn handle_key(&mut self, event: KeyEvent) -> InputOutcome {
        use tuikit::keys::{TEXT_INPUT_KEYS, TextInputKey, resolve};
        match resolve(TEXT_INPUT_KEYS, event) {
            Some(TextInputKey::Confirm) => InputOutcome::Confirm(self.edit.value().to_string()),
            Some(TextInputKey::Abort) => InputOutcome::Abort,
            Some(TextInputKey::Cancel) => InputOutcome::Cancel,
            Some(TextInputKey::Tab) => InputOutcome::Continue,
            None => {
                self.edit.handle(event);
                InputOutcome::Continue
            }
        }
    }
}

/// How large a prompt is: one row in a compact frame.
pub(crate) const PROMPT_SIZE: ModalSize = ModalSize::COMPACT;

/// Centered modal rendering of [`TextPromptState`]: the field in a
/// compact frame titled with the prompt.
pub struct TextPromptWidget<'a> {
    state: &'a TextPromptState,
    theme: &'a Theme,
}

impl<'a> TextPromptWidget<'a> {
    pub fn new(state: &'a TextPromptState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for TextPromptWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        ModalFrame::new(self.theme, PROMPT_SIZE)
            .title(self.state.title.as_str())
            .render(
                area,
                buf,
                PromptBody {
                    state: self.state,
                    theme: self.theme,
                },
            );
    }
}

/// The prompt's one row: its line as a text field, windowed so the
/// cursor stays in view.
struct PromptBody<'a> {
    state: &'a TextPromptState,
    theme: &'a Theme,
}

impl ModalBody for PromptBody<'_> {
    type Output = ();

    fn measure(&mut self, _width: u16) -> u16 {
        1
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        TextField::new(&self.state.edit).render(area, buf, self.theme);
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
