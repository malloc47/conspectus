//! Forms: several fields in one modal, moved through on one ring and
//! submitted from any of them, with one error line and a footer that
//! stays in view.
//!
//! This is a layer 1 control (house rules 6, 7, 8, 13, 14, 15, 16, and
//! 17). A program that asks for several values at once (a name and a
//! directory, a command and its options) should move, submit, refuse, and
//! show its keys the same way in every such form, or one form advances on
//! `Enter` and the next submits, one refuses silently and the next shows
//! its reason somewhere else, and on a short screen one footer scrolls
//! away while another stays:
//!
//! - **Fields.** A [`Form<M>`](Form) holds an ordered list of [`Field`]s,
//!   each of a [`FieldKind`]: text (with an optional check of its text),
//!   a path ([`PathField`](crate::path_field::PathField)), a [`Choice`]
//!   of named options shown as chips (optionally taking a typed value of
//!   its own), a [`Toggle`], a [`CheckGroup`], a [`OneOf`] row (drawn
//!   with the one-of-N box `(•)`, which a checkbox never looks like), and
//!   [`ReadOnly`] lines in a [`Tone`] (a note, derived output, or a
//!   warning). A field can be shown only while a condition over the
//!   form's [`Values`] holds.
//! - **One ring.** `Tab` and `↓` go to the next field and `S-Tab` and `↑`
//!   to the previous, wrapping, one stop per field; read-only lines and
//!   hidden fields are not stops. `Tab` on a path field completes the
//!   path first and moves on only when there is nothing to complete.
//! - **Enter submits from any field.** Each shown field's check runs top
//!   to bottom; the first that fails puts its reason on the error line
//!   and takes the focus. When all pass, the host's `on_submit` gets the
//!   [`Values`] and answers with a [`Submit`]: commit a message, refuse
//!   with a reason, or close.
//! - **One error line**, in the footer slot above the footer, set by a
//!   refused submit and cleared by the next edit.
//! - **A fixed footer**, generated from the form's key table and wrapped
//!   between hints, which never scrolls away: on a short screen only the
//!   rows scroll, keeping the focused one in view ([`FormBody`]).
//! - **One grid.** Labels sit in one column ([`Cols`]), values start in
//!   the next, and every color is a theme token.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::form::{Field, Form, OneOf, Submit};
//! use tuikit::keys::{FORM_KEYS, FORM_OPTION_KEYS, FormKey, KeyRow, include, key, shown};
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Debug, PartialEq)]
//! enum Msg {
//!     Create { name: String, size: usize },
//! }
//!
//! const KEYS: &[KeyRow<FormKey>] = &[
//!     include(FORM_KEYS),
//!     include(FORM_OPTION_KEYS),
//!     shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "create"),
//!     shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
//! ];
//!
//! let sizes = vec!["small".to_string(), "large".to_string()];
//! let mut form = Form::new("New volume", KEYS, |values| Submit::Commit(Msg::Create {
//!     name: values.text("name").trim().to_string(),
//!     size: values.chosen("size").unwrap_or(0),
//! }))
//! .field(Field::text("name", "name", "").validate(|text| {
//!     if text.trim().is_empty() { Err("a name is required".into()) } else { Ok(()) }
//! }))
//! .field(Field::one_of("size", "size", OneOf::new(sizes, 0)));
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // Enter submits from any field; the empty name is refused and says why.
//! assert_eq!(form.handle(press(KeyCode::Enter), &()), Outcome::Consumed);
//! assert_eq!(form.error(), Some("a name is required"));
//!
//! // Typing clears the error; the arrows step the one-of-N row.
//! form.handle(press(KeyCode::Char('v')), &());
//! assert_eq!(form.error(), None);
//! form.handle(press(KeyCode::Down), &());
//! form.handle(press(KeyCode::Right), &());
//! let done = form.handle(press(KeyCode::Enter), &());
//! assert_eq!(done, Outcome::Commit(Msg::Create { name: "v".into(), size: 1 }));
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
//! form.draw(buf.area, &mut buf, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("( ) small (•) large"), "{drawn}");
//! assert!(drawn.contains("Enter create · Esc cancel"), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! The fields and their order, labels, and seeds; the key table, with the
//! form's own `Enter` verb (`Enter save`); the checks' reasons; and what
//! a submit means: `on_submit` reads the values (a path's expanded value,
//! a toggle's state) and builds the host's message, or gives the reason
//! the values are refused. A host that must change one field when another
//! changes (a line derived from the others, the options that follow a
//! choice) wraps the form, routes the key to it, and reads
//! [`Form::edited`] to see which field the key changed.

mod body;
mod field;
mod row;

pub use body::FormBody;
pub use field::{CheckGroup, Choice, Field, FieldKind, OneOf, ReadOnly, Toggle, Values};
pub use row::{
    CHOICE_SLOT, CUSTOM, Cols, EMPTY, LABEL_GAP, LABEL_MAX, LABEL_MIN, Tone, check_row, checkbox,
    chips, choice_cells, choice_row, error_line, field_style, footer_lines, label, one_of_row,
    option, path_cells, path_row, read_only_row, row, static_cells, static_row, text_cells,
    text_row, toggle_row,
};

use std::fmt;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::frame::{ModalBody, ModalFrame, ModalSize};
use crate::keys::{FormKey, KeyRow, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::selection::Ends;
use crate::theme::Theme;

/// How large every form is: the form modal frame, as tall as its rows.
pub const FORM_SIZE: ModalSize = ModalSize::FORM;

/// What a form's `on_submit` decides about the values it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Submit<M> {
    /// Take the values: the form closes, then the host handles `M`.
    Commit(M),
    /// Refuse them and say why: the form stays open with the reason on
    /// its error line until the next edit.
    Reject(String),
    /// Close the form with nothing to do (a save that changes nothing).
    Close,
}

/// The host's decision about submitted values, shared by every copy of
/// the form.
type OnSubmit<M> = Arc<dyn Fn(&Values<'_>) -> Submit<M> + Send + Sync>;

/// A modal of several fields: a title, the fields' rows on one ring, an
/// error line while a submit is refused, and the footer its key table
/// generates.
///
/// The form implements [`Overlay<M, Cx>`] for every `Cx` and reads
/// nothing from the context. `on_submit` is shared behind an [`Arc`], so
/// a form clones cheaply and a host's enum of overlays can stay `Clone`.
///
/// The key table is the host's, built on the standard
/// [`FORM_KEYS`](crate::keys::FORM_KEYS) (with
/// [`FORM_OPTION_KEYS`](crate::keys::FORM_OPTION_KEYS) when the form has
/// option rows) and ending with its `Enter` verb and `Esc cancel`. Keys
/// the table does not bind edit the focused field; on a text field the
/// option keys (`←`, `→`, `Space`) edit too.
pub struct Form<M> {
    title: String,
    keys: &'static [KeyRow<FormKey>],
    fields: Vec<Field>,
    focus: Option<usize>,
    error: Option<String>,
    edited: Option<&'static str>,
    on_submit: OnSubmit<M>,
}

impl<M> Form<M> {
    /// An empty form titled `title` that resolves keys against `keys` and
    /// asks `on_submit` what a submit means once every shown field's
    /// check passes. Add its fields with [`field`](Self::field).
    pub fn new(
        title: impl Into<String>,
        keys: &'static [KeyRow<FormKey>],
        on_submit: impl Fn(&Values<'_>) -> Submit<M> + Send + Sync + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            keys,
            fields: Vec::new(),
            focus: None,
            error: None,
            edited: None,
            on_submit: Arc::new(on_submit),
        }
    }

    /// Adds `field` after the others. The focus starts on the first field
    /// that is a shown stop of the ring.
    #[must_use]
    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        if self.focus.is_none() {
            self.settle_focus();
        }
        self
    }

    /// The title on the form's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The key table the form resolves keys against.
    pub fn keys(&self) -> &'static [KeyRow<FormKey>] {
        self.keys
    }

    /// Every field, shown or not, in order.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// The field `id`.
    pub fn get(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.id() == id)
    }

    /// The field `id`, to change it from the host. A change made here
    /// does not clear the error line; only the user's edits do.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Field> {
        self.fields.iter_mut().find(|field| field.id() == id)
    }

    /// What the fields hold, as `on_submit` sees them.
    pub fn values(&self) -> Values<'_> {
        Values::of(&self.fields)
    }

    /// The id of the field that has the focus; `None` when no field is a
    /// shown stop.
    pub fn focus(&self) -> Option<&'static str> {
        self.settled().map(|index| self.fields[index].id())
    }

    /// Moves the focus to field `id`; `false`, and the focus left where
    /// it was, when no shown stop has that id.
    pub fn focus_on(&mut self, id: &str) -> bool {
        let found = self
            .stops()
            .into_iter()
            .find(|index| self.fields[*index].id() == id);
        if found.is_some() {
            self.focus = found;
        }
        found.is_some()
    }

    /// The reason the last submit was refused, until the next edit.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The field the last key changed (typed in, stepped, flipped, or
    /// completed); `None` when it changed nothing. A host that keeps
    /// another field in step with this one reads it after each key.
    pub fn edited(&self) -> Option<&'static str> {
        self.edited
    }

    /// Whether typing edits the focused field, so a paste belongs to it.
    pub fn takes_text(&self) -> bool {
        self.settled()
            .is_some_and(|index| self.fields[index].takes_text())
    }

    /// The rect the form draws in on `area`: the form frame around its
    /// rows, centered. A host tests a click against it.
    pub fn rect(&self, area: Rect, theme: &Theme) -> Rect {
        let frame = self.frame(theme);
        let width = frame.inner_width(area);
        let mut body = self.body(usize::from(width), theme);
        frame.rect(area, body.measure(width))
    }

    /// Draws the form over `area`: the frame, the rows, the error line,
    /// and the footer. On a stack, [`Overlay::render`] does this.
    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let frame = self.frame(theme);
        let width = usize::from(frame.inner_width(area.intersection(buf.area)));
        frame.render(area, buf, self.body(width, theme));
    }

    fn frame<'a>(&'a self, theme: &'a Theme) -> ModalFrame<'a> {
        ModalFrame::new(theme, FORM_SIZE).title(self.title.as_str())
    }

    /// The body laid out `width` cells wide: a row per shown field, the
    /// cursor on the focused one; a blank line; the error line; the
    /// footer.
    fn body(&self, width: usize, theme: &Theme) -> FormBody<'static> {
        let shown = self.shown();
        let focus = self.settled();
        let cols = Cols::for_labels(width, shown.iter().map(|index| self.fields[*index].label()));
        let mut cursor = 0;
        let rows = shown
            .iter()
            .enumerate()
            .map(|(row, index)| {
                let focused = focus == Some(*index);
                if focused {
                    cursor = row;
                }
                self.fields[*index].line(cols, focused, theme)
            })
            .collect();
        FormBody::new(rows, cursor)
            .notes(vec![Line::default()])
            .error(
                self.error
                    .as_deref()
                    .map(|reason| error_line(reason, width, theme)),
            )
            .footer(footer_lines(self.keys, width, theme))
    }

    /// The indexes of the shown fields, in order.
    fn shown(&self) -> Vec<usize> {
        let values = self.values();
        (0..self.fields.len())
            .filter(|index| self.fields[*index].shown(&values))
            .collect()
    }

    /// The indexes of the ring's stops: the shown fields that are not
    /// read-only lines.
    fn stops(&self) -> Vec<usize> {
        self.shown()
            .into_iter()
            .filter(|index| self.fields[*index].is_stop())
            .collect()
    }

    /// Moves the focus `delta` stops along the ring, wrapping.
    fn step(&mut self, delta: i32) {
        let stops = self.stops();
        if stops.is_empty() {
            self.focus = None;
            return;
        }
        let at = self
            .focus
            .and_then(|focus| stops.iter().position(|index| *index == focus))
            .unwrap_or(0);
        self.focus = Some(stops[Ends::Wrap.step(at, stops.len(), delta)]);
    }

    /// Where the focus is once the fields are as they are now: on the
    /// focused field while it is a shown stop, else on the next stop after
    /// it, else on the last one before it. A field the user's change or
    /// the host hid hands the focus on this way.
    fn settled(&self) -> Option<usize> {
        let stops = self.stops();
        let from = self.focus.unwrap_or(0);
        if stops.contains(&from) {
            Some(from)
        } else {
            stops
                .iter()
                .copied()
                .find(|index| *index > from)
                .or_else(|| stops.last().copied())
        }
    }

    /// Moves the focus where [`settled`](Self::settled) says it is.
    fn settle_focus(&mut self) {
        self.focus = self.settled();
    }

    /// Records that the focused field changed: the error line goes, and
    /// a field shown or hidden by the change moves the focus if it must.
    fn changed(&mut self, index: usize) {
        self.error = None;
        self.edited = Some(self.fields[index].id());
        self.settle_focus();
    }

    /// Checks every shown field, top to bottom, then asks `on_submit`.
    fn submit(&mut self) -> Outcome<M> {
        let failed = self.shown().into_iter().find_map(|index| {
            self.fields[index]
                .check()
                .err()
                .map(|reason| (index, reason))
        });
        if let Some((index, reason)) = failed {
            self.error = Some(reason);
            if self.fields[index].is_stop() {
                self.focus = Some(index);
            }
            return Outcome::Consumed;
        }
        match (self.on_submit)(&self.values()) {
            Submit::Commit(message) => Outcome::Commit(message),
            Submit::Reject(reason) => {
                self.error = Some(reason);
                Outcome::Consumed
            }
            Submit::Close => Outcome::Close,
        }
    }

    /// Handles one key; see [`Overlay::handle`].
    fn key(&mut self, key: KeyEvent) -> Outcome<M> {
        self.edited = None;
        // The host may have hidden the focused field since the last key.
        self.settle_focus();
        let action = resolve(self.keys, key);
        match action {
            Some(FormKey::Abort | FormKey::Cancel) => return Outcome::Close,
            Some(FormKey::Up | FormKey::Back) => self.step(-1),
            Some(FormKey::Down) => self.step(1),
            Some(FormKey::Tab) => {
                let completed = self.focus.filter(|index| self.fields[*index].complete());
                match completed {
                    Some(index) => self.changed(index),
                    None => self.step(1),
                }
            }
            Some(FormKey::Submit) => return self.submit(),
            // No field takes an inherited value yet; the key is the
            // form's, so it never reaches a field's text.
            Some(FormKey::Inherit) => {}
            Some(option @ (FormKey::OptionPrev | FormKey::OptionNext | FormKey::Toggle)) => {
                if let Some(index) = self.focus {
                    let field = &mut self.fields[index];
                    let changed = if field.takes_options() {
                        field.change(option)
                    } else {
                        field.edit(key)
                    };
                    if changed {
                        self.changed(index);
                    }
                }
            }
            None => {
                if let Some(index) = self.focus
                    && self.fields[index].edit(key)
                {
                    self.changed(index);
                }
            }
        }
        Outcome::Consumed
    }
}

impl<M> Clone for Form<M> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            keys: self.keys,
            fields: self.fields.clone(),
            focus: self.focus,
            error: self.error.clone(),
            edited: self.edited,
            on_submit: Arc::clone(&self.on_submit),
        }
    }
}

impl<M> fmt::Debug for Form<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Form")
            .field("title", &self.title)
            .field("fields", &self.fields)
            .field("focus", &self.focus())
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<M, Cx: ?Sized> Overlay<M, Cx> for Form<M> {
    /// `Enter` submits from any field; `Esc` and `Ctrl-C` close the form;
    /// `Tab`/`↓` and `S-Tab`/`↑` move along the ring (`Tab` completing a
    /// path first); the option keys act on an option row and edit a text
    /// field; any other key edits the focused field, and one that changes
    /// it clears the error line.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        self.key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    fn takes_text(&self) -> bool {
        Form::takes_text(self)
    }
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
