//! Text prompts: one line of text asked for in a small modal, which the
//! host takes, refuses with a reason, or lets the user leave.
//!
//! This is a layer 1 control (house rules 4, 5, 7, and 8). A program asks
//! for one line in several places (a new name, a value for a setting, a
//! filter's text), and each such prompt should confirm, refuse, and close
//! the same way, or one says why it refused and the next does nothing:
//!
//! - **One contract.** [`TextPrompt<M>`](TextPrompt) is an
//!   [`Overlay<M, Cx>`](crate::overlay::Overlay) for any context. `Enter`
//!   hands the text to the host's `on_confirm`, which answers with a
//!   [`PromptResult`]: [`Commit`](PromptResult::Commit) closes the prompt
//!   with the host's message, [`Back`](PromptResult::Back) closes it as
//!   `Esc` would with the host's message (a step that applies its answer
//!   and returns to the overlay under it),
//!   [`Reject`](PromptResult::Reject) keeps it open with the reason on
//!   its error line, and [`Stay`](PromptResult::Stay) keeps it open as it
//!   is. `Esc` closes
//!   it and `Ctrl-C` dismisses it (with any flow it is a step of),
//!   saying nothing; `Tab` is swallowed; every other key edits the line,
//!   `q` included.
//! - **One error line.** A refused confirm shows its reason on one row
//!   under the field, in the theme's `error`, until the next key that
//!   changes the text clears it. On a screen too short for that row the
//!   reason takes the footer's place on the bottom border. No confirm is
//!   refused silently.
//! - **One look.** The prompt is the compact modal frame with the title on
//!   its top border, the field in its one row, and the footer that
//!   [`TEXT_INPUT_KEYS`] generates on its bottom border: three rows, and
//!   four while an error line shows.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::text_prompt::{PromptResult, TextPrompt};
//! use tuikit::theme::Theme;
//!
//! #[derive(Debug, PartialEq)]
//! enum Msg {
//!     Rename(String),
//! }
//!
//! let mut prompt = TextPrompt::new("Rename", "", |text: &str| match text.trim() {
//!     "" => PromptResult::Reject("a name is required".into()),
//!     name => PromptResult::Commit(Msg::Rename(name.to_string())),
//! });
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // An empty name is refused, and the prompt says why.
//! assert_eq!(prompt.handle(press(KeyCode::Enter), &()), Outcome::Consumed);
//! assert_eq!(prompt.error(), Some("a name is required"));
//!
//! // Typing clears the reason; a name commits.
//! prompt.handle(press(KeyCode::Char('q')), &());
//! assert_eq!(prompt.error(), None);
//! let done = prompt.handle(press(KeyCode::Enter), &());
//! assert_eq!(done, Outcome::Commit(Msg::Rename("q".into())));
//!
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 40, 9);
//! let mut buf = Buffer::empty(area);
//! Overlay::<Msg, ()>::render(&mut prompt, area, &mut buf, &(), &theme);
//! assert_eq!(prompt.rect(area), Rect::new(2, 3, 36, 3), "three rows, centered");
//! ```
//!
//! # What a host keeps
//!
//! The title, the initial text, and what a confirm means: the
//! `on_confirm` closure parses or checks the text (with the host's own
//! parsers) and builds the host's message, or gives the reason the text
//! is refused. A prompt that is a menu's follow-up question opens as a
//! step of the menu
//! ([`ModalStack::push_step`](crate::overlay::ModalStack::push_step)):
//! `Esc` returns to the menu, and a commit or `Ctrl-C` closes both. A
//! prompt that edits one value of an overlay that stays open (a settings
//! panel's editor) opens as a step too and answers with
//! [`PromptResult::Back`], so the value applies and the overlay has the
//! keys again.

use std::fmt;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::frame::{ModalBody, ModalFrame, ModalSize};
use crate::keys::{Footer, TEXT_INPUT_KEYS, TextInputKey, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::text;
use crate::text_edit::{LineEdit, TextField};
use crate::theme::Theme;

/// How large every prompt is: the compact modal frame, as tall as its
/// rows.
pub const PROMPT_SIZE: ModalSize = ModalSize::COMPACT;

/// What a prompt's `on_confirm` decides about the text it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptResult<M> {
    /// Take the text: the prompt closes, then the host handles `M`.
    Commit(M),
    /// Take the text and step back: the prompt closes as `Esc` closes it
    /// (a step gives the keys back to the overlay under it, which stays
    /// open), then the host handles `M`. The prompt answers
    /// [`Outcome::Back`].
    Back(M),
    /// Refuse the text and say why: the prompt stays open and shows the
    /// reason on its error line until the next edit.
    Reject(String),
    /// Neither take nor refuse it: the prompt stays open as it is.
    Stay,
}

/// The host's decision about a confirmed text, shared by every copy of
/// the prompt.
type OnConfirm<M> = Arc<dyn Fn(&str) -> PromptResult<M> + Send + Sync>;

/// A modal that asks for one line of text: a title, the field, an error
/// line while a confirm is refused, and the prompt's keys.
///
/// The prompt implements [`Overlay<M, Cx>`] for every `Cx`, so any host
/// can stack it; it reads nothing from the context. `on_confirm` is
/// shared behind an [`Arc`], so a prompt clones cheaply and a host's enum
/// of overlays can stay `Clone`.
pub struct TextPrompt<M> {
    title: String,
    edit: LineEdit,
    placeholder: String,
    error: Option<String>,
    on_confirm: OnConfirm<M>,
}

impl<M> TextPrompt<M> {
    /// A prompt titled `title` holding `initial`, with the cursor at its
    /// end so typing appends and the text can be edited in place.
    /// `on_confirm` gets the text as typed (untrimmed) when `Enter` is
    /// pressed and decides what the prompt does.
    pub fn new(
        title: impl Into<String>,
        initial: impl Into<String>,
        on_confirm: impl Fn(&str) -> PromptResult<M> + Send + Sync + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            edit: LineEdit::new(initial),
            placeholder: String::new(),
            error: None,
            on_confirm: Arc::new(on_confirm),
        }
    }

    /// What the field shows, in the theme's `inert`, while it is empty.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The title on the prompt's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The text being edited.
    pub fn value(&self) -> &str {
        self.edit.value()
    }

    /// The line editor, for its cursor or a host that needs more of it.
    pub fn edit(&self) -> &LineEdit {
        &self.edit
    }

    /// The reason the last confirm was refused, until the next edit.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The rect the prompt draws in on `area`: the compact frame around
    /// its rows, centered. A host tests a click against it.
    pub fn rect(&self, area: Rect) -> Rect {
        // The rows and the border above and below them.
        PROMPT_SIZE.rect(area, self.rows() + 2)
    }

    /// The rows inside the frame: the field, and the error line while
    /// there is one.
    fn rows(&self) -> u16 {
        1 + u16::from(self.error.is_some())
    }

    /// Draws the prompt over `area`: the frame, the field, the error line,
    /// and the footer. A host whose overlay holds the prompt as one of its
    /// levels draws it with this; on a stack, [`Overlay::render`] does.
    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let frame = ModalFrame::new(theme, PROMPT_SIZE).title(self.title.as_str());
        // The footer sits on the bottom border, one space in from each
        // corner, and breaks only between hints.
        let room = usize::from(frame.inner_width(area)).saturating_sub(2);
        let squeezed = self.rect(area.intersection(buf.area)).height < self.rows() + 2;
        let footer = match &self.error {
            // A screen too short for the error line's row shows the
            // reason where the hints were, so no refusal goes unexplained.
            Some(reason) if squeezed => Line::from(Span::styled(
                text::truncate_end(reason, room),
                Style::default().fg(theme.error),
            )),
            _ => Footer::new(TEXT_INPUT_KEYS)
                .lines(room, theme)
                .into_iter()
                .next()
                .unwrap_or_default(),
        };
        frame.footer(footer).render(
            area,
            buf,
            Body {
                prompt: self,
                theme,
            },
        );
    }
}

impl<M> Clone for TextPrompt<M> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            edit: self.edit.clone(),
            placeholder: self.placeholder.clone(),
            error: self.error.clone(),
            on_confirm: Arc::clone(&self.on_confirm),
        }
    }
}

impl<M> fmt::Debug for TextPrompt<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextPrompt")
            .field("title", &self.title)
            .field("value", &self.edit.value())
            .field("placeholder", &self.placeholder)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<M, Cx: ?Sized> Overlay<M, Cx> for TextPrompt<M> {
    /// `Enter` asks `on_confirm`; `Esc` closes the prompt and `Ctrl-C`
    /// dismisses it; `Tab` is swallowed; any other key goes to the line
    /// editor, and one that changes the text clears the error line.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        match resolve(TEXT_INPUT_KEYS, key) {
            Some(TextInputKey::Abort) => Outcome::Dismiss,
            Some(TextInputKey::Cancel) => Outcome::Close,
            Some(TextInputKey::Tab) => Outcome::Consumed,
            Some(TextInputKey::Confirm) => match (self.on_confirm)(self.edit.value()) {
                PromptResult::Commit(message) => Outcome::Commit(message),
                PromptResult::Back(message) => Outcome::Back(message),
                PromptResult::Reject(reason) => {
                    self.error = Some(reason);
                    Outcome::Consumed
                }
                PromptResult::Stay => Outcome::Consumed,
            },
            None => {
                if self.edit.handle(key).changed {
                    self.error = None;
                }
                Outcome::Consumed
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    fn takes_text(&self) -> bool {
        true
    }

    /// The text input table's footer, the one on its bottom border.
    fn hints(&self) -> Footer {
        Footer::new(TEXT_INPUT_KEYS)
    }
}

/// The prompt's rows: its field, and its error line while it has one.
struct Body<'a, M> {
    prompt: &'a TextPrompt<M>,
    theme: &'a Theme,
}

impl<M> ModalBody for Body<'_, M> {
    type Output = ();

    fn measure(&mut self, _width: u16) -> u16 {
        self.prompt.rows()
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        let Self { prompt, theme } = self;
        // The placeholder recedes; the text the user types is plain.
        let style = if prompt.edit.is_empty() {
            Style::default().fg(theme.inert)
        } else {
            Style::default()
        };
        TextField::new(&prompt.edit)
            .placeholder(&prompt.placeholder)
            .style(style)
            .render(area, buf, theme);
        if let Some(reason) = &prompt.error
            && area.height > 1
        {
            let row = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            let shown = text::truncate_end(reason, usize::from(row.width));
            Line::from(Span::styled(shown, Style::default().fg(theme.error))).render(row, buf);
        }
    }
}

#[cfg(test)]
#[path = "text_prompt_tests.rs"]
mod tests;
