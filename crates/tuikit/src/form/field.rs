//! A form's fields: what each kind holds, how its keys change it, and the
//! read-only view of every field's value that a form's closures get.

use std::fmt;
use std::sync::Arc;

use ratatui::crossterm::event::KeyEvent;
use ratatui::text::{Line, Span};

use super::override_row::{OverrideRow, Standing};
use super::row::{self, Cols, Tone};
use crate::keys::FormKey;
use crate::path_field::PathField;
use crate::selection::Ends;
use crate::text;
use crate::text_edit::LineEdit;
use crate::theme::Theme;

/// A check on a field's text when the form is submitted: `Err` with the
/// reason the form shows on its error line.
type Validator = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Whether a field is shown, decided from the form's values.
type Condition = Arc<dyn Fn(&Values<'_>) -> bool + Send + Sync>;

/// One field of a [`Form`](super::Form): an id the host reads it back by,
/// the label its row shows, and what kind of value it holds.
///
/// A field may carry a check of its text ([`validate`](Self::validate)),
/// run when the form is submitted, and a condition for being shown
/// ([`visible_if`](Self::visible_if)), evaluated over the form's values
/// each time they change. A hidden field has no row, is not a stop of the
/// ring, and is not checked. A text, path, or choice field may follow an
/// inherited value ([`with_override`](Self::with_override)), and any
/// field may explain itself in the form's detail lines while it has
/// focus ([`hint`](Self::hint)).
#[derive(Clone)]
pub struct Field {
    id: &'static str,
    label: String,
    kind: FieldKind,
    validate: Option<Validator>,
    visible: Option<Condition>,
    overrides: Option<OverrideRow>,
    hints: Vec<String>,
}

/// What a field holds and how its row is drawn.
#[derive(Clone, Debug)]
pub enum FieldKind {
    /// One line of text, edited in place.
    Text(LineEdit),
    /// A path, edited in place, checked, and completed with `Tab`.
    Path(PathField),
    /// One of several named options, shown as chips after the value:
    /// the option keys step through them, and a choice that allows a
    /// custom value takes typing too.
    Choice(Choice),
    /// One checkbox: on or off.
    Toggle(Toggle),
    /// A row of checkboxes, any number of them on.
    CheckGroup(CheckGroup),
    /// A row of options of which exactly one is chosen.
    OneOf(OneOf),
    /// A line that is only read: a note, or output derived from the
    /// other fields. Never focused and never a stop of the ring.
    ReadOnly(ReadOnly),
}

impl Field {
    /// A field of `kind`, read back by `id`, labelled `label`.
    pub fn new(id: &'static str, label: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            validate: None,
            visible: None,
            overrides: None,
            hints: Vec::new(),
        }
    }

    /// A text field holding `value`, the cursor at its end.
    pub fn text(id: &'static str, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(id, label, FieldKind::Text(LineEdit::new(value)))
    }

    /// A path field over `field`.
    pub fn path(id: &'static str, label: impl Into<String>, field: PathField) -> Self {
        Self::new(id, label, FieldKind::Path(field))
    }

    /// A choice among named options.
    pub fn choice(id: &'static str, label: impl Into<String>, choice: Choice) -> Self {
        Self::new(id, label, FieldKind::Choice(choice))
    }

    /// One checkbox.
    pub fn toggle(id: &'static str, label: impl Into<String>, toggle: Toggle) -> Self {
        Self::new(id, label, FieldKind::Toggle(toggle))
    }

    /// A row of checkboxes.
    pub fn check_group(id: &'static str, label: impl Into<String>, group: CheckGroup) -> Self {
        Self::new(id, label, FieldKind::CheckGroup(group))
    }

    /// A row of options, one of them chosen.
    pub fn one_of(id: &'static str, label: impl Into<String>, one_of: OneOf) -> Self {
        Self::new(id, label, FieldKind::OneOf(one_of))
    }

    /// A line that is only read.
    pub fn read_only(id: &'static str, label: impl Into<String>, read_only: ReadOnly) -> Self {
        Self::new(id, label, FieldKind::ReadOnly(read_only))
    }

    /// Checks the field's text when the form is submitted: the text as
    /// typed (untrimmed) of a text, path, or choice field. An `Err`
    /// stops the submit, puts its reason on the form's error line, and
    /// moves the focus here. Fields are checked top to bottom, the first
    /// failure wins, and the form's own `on_submit` runs only when every
    /// shown field passes. Other kinds have no text, so they pass.
    #[must_use]
    pub fn validate(
        mut self,
        check: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.validate = Some(Arc::new(check));
        self
    }

    /// Shows the field only while `shown` says so, given the form's
    /// values: a field that matters only when a toggle is on, or only for
    /// some choices.
    #[must_use]
    pub fn visible_if(
        mut self,
        shown: impl Fn(&Values<'_>) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.visible = Some(Arc::new(shown));
        self
    }

    /// Makes the field an override row: it follows `row`'s inherited
    /// value until the user changes it, the form's reset key puts that
    /// value back, and its row ends in a word for where the value stands.
    /// The field's text now is the value the form opens with, so decorate
    /// a field once its value is set. Only fields that hold text (text,
    /// path, and choice fields) can follow a value; on another kind the
    /// decorator has no effect.
    #[must_use]
    pub fn with_override(mut self, mut row: OverrideRow) -> Self {
        if let Some(text) = self.text_value() {
            row.open(text);
            self.overrides = Some(row);
        }
        self
    }

    /// Adds a line the form's detail lines show while the field has focus
    /// (what its keys do, what its value means), after the lines its
    /// override row's hint gives. A form shows them only when it reserves
    /// detail lines ([`Form::details`](super::Form::details)).
    #[must_use]
    pub fn hint(mut self, line: impl Into<String>) -> Self {
        self.hints.push(line.into());
        self
    }

    /// The id the host reads the field back by.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// The label its row shows.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Changes the label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// What the field holds.
    pub fn kind(&self) -> &FieldKind {
        &self.kind
    }

    /// What the field holds, to change it from the host (new options, a
    /// derived line's text). A change made here does not clear the form's
    /// error line; only the user's edits do.
    pub fn kind_mut(&mut self) -> &mut FieldKind {
        &mut self.kind
    }

    /// The text of a text, path, or choice field, as typed; `None` for
    /// the kinds that hold no text.
    pub fn text_value(&self) -> Option<&str> {
        match &self.kind {
            FieldKind::Text(edit) => Some(edit.value()),
            FieldKind::Path(path) => Some(path.value()),
            FieldKind::Choice(choice) => Some(choice.value()),
            FieldKind::Toggle(_)
            | FieldKind::CheckGroup(_)
            | FieldKind::OneOf(_)
            | FieldKind::ReadOnly(_) => None,
        }
    }

    /// The override row the field is decorated with, if any.
    pub fn override_row(&self) -> Option<&OverrideRow> {
        self.overrides.as_ref()
    }

    /// Where an override row's value stands; `None` for a field that is
    /// not one.
    pub fn standing(&self) -> Option<Standing> {
        let row = self.overrides.as_ref()?;
        Some(row.standing(self.text_value().unwrap_or("")))
    }

    /// The lines the form's detail lines show while the field has focus:
    /// what its override row's hint says, then its own hint lines.
    pub fn hints(&self) -> Vec<String> {
        let mut lines = self
            .overrides
            .as_ref()
            .map_or_else(Vec::new, |row| row.hints(self.text_value().unwrap_or("")));
        lines.extend(self.hints.iter().cloned());
        lines
    }

    /// Whether the field is a stop of the form's ring: every kind but a
    /// read-only line.
    pub fn is_stop(&self) -> bool {
        !matches!(self.kind, FieldKind::ReadOnly(_))
    }

    /// Whether typing edits the field (a text or path field, or a choice
    /// that allows a custom value), so a paste belongs to it.
    pub fn takes_text(&self) -> bool {
        match &self.kind {
            FieldKind::Text(_) | FieldKind::Path(_) => true,
            FieldKind::Choice(choice) => choice.allows_custom(),
            FieldKind::Toggle(_)
            | FieldKind::CheckGroup(_)
            | FieldKind::OneOf(_)
            | FieldKind::ReadOnly(_) => false,
        }
    }

    /// Whether the option keys act on the field rather than edit it.
    pub(super) fn takes_options(&self) -> bool {
        matches!(
            self.kind,
            FieldKind::Choice(_)
                | FieldKind::Toggle(_)
                | FieldKind::CheckGroup(_)
                | FieldKind::OneOf(_)
        )
    }

    /// Whether the field is shown, given the form's values.
    pub(super) fn shown(&self, values: &Values<'_>) -> bool {
        self.visible.as_ref().is_none_or(|shown| shown(values))
    }

    /// The field's check, over its text.
    pub(super) fn check(&self) -> Result<(), String> {
        match (&self.validate, self.text_value()) {
            (Some(check), Some(text)) => check(text),
            _ => Ok(()),
        }
    }

    /// Gives a key that edits to the field; whether the value changed.
    pub(super) fn edit(&mut self, key: KeyEvent) -> bool {
        let changed = match &mut self.kind {
            FieldKind::Text(edit) => edit.handle(key).changed,
            FieldKind::Path(path) => path.handle(key).changed,
            FieldKind::Choice(choice) if choice.allows_custom() => choice.edit.handle(key).changed,
            FieldKind::Choice(_)
            | FieldKind::Toggle(_)
            | FieldKind::CheckGroup(_)
            | FieldKind::OneOf(_)
            | FieldKind::ReadOnly(_) => false,
        };
        self.touched(changed)
    }

    /// Acts on the field with an option key; whether the value changed.
    pub(super) fn change(&mut self, action: FormKey) -> bool {
        let delta = if action == FormKey::OptionPrev { -1 } else { 1 };
        let changed = match &mut self.kind {
            FieldKind::Choice(choice) => choice.step(delta),
            FieldKind::Toggle(toggle) => {
                toggle.flip();
                true
            }
            FieldKind::CheckGroup(group) => group.act(action),
            FieldKind::OneOf(one_of) => one_of.step(delta),
            FieldKind::Text(_) | FieldKind::Path(_) | FieldKind::ReadOnly(_) => false,
        };
        self.touched(changed)
    }

    /// Completes a path field; whether anything was completed.
    pub(super) fn complete(&mut self) -> bool {
        let changed = match &mut self.kind {
            FieldKind::Path(path) => path.complete(),
            _ => false,
        };
        self.touched(changed)
    }

    /// Puts an override row's inherited value back; whether the value or
    /// where it stands changed. A field that is not an override row
    /// keeps its value.
    pub(super) fn reset(&mut self) -> bool {
        let Some(row) = &self.overrides else {
            return false;
        };
        let inherited = row.inherited().to_string();
        let before = (self.text_value().map(str::to_string), self.standing());
        match &mut self.kind {
            FieldKind::Text(edit) => {
                edit.set_value(inherited);
            }
            FieldKind::Path(path) => {
                path.set_value(inherited);
            }
            FieldKind::Choice(choice) => choice.set_value(inherited),
            FieldKind::Toggle(_)
            | FieldKind::CheckGroup(_)
            | FieldKind::OneOf(_)
            | FieldKind::ReadOnly(_) => return false,
        }
        if let Some(row) = &mut self.overrides {
            row.set_reset(true);
        }
        before != (self.text_value().map(str::to_string), self.standing())
    }

    /// After the user's key: a change to an override row's value makes it
    /// the field's own, so a reset before it no longer stands.
    fn touched(&mut self, changed: bool) -> bool {
        if changed && let Some(row) = &mut self.overrides {
            row.set_reset(false);
        }
        changed
    }

    /// The field's row in `cols`.
    pub(super) fn line(&self, cols: Cols, focused: bool, theme: &Theme) -> Line<'static> {
        let label = self.label.as_str();
        if cols.source > 0
            && let Some(cells) = self.value_cells(cols.value(), focused, theme)
        {
            return self.sourced_line(cells, cols, focused, theme);
        }
        match &self.kind {
            FieldKind::Text(edit) => row::text_row(label, edit, focused, cols, theme),
            FieldKind::Path(path) => row::path_row(label, path, focused, cols, theme),
            FieldKind::Choice(choice) => {
                row::choice_row(label, &choice.edit, &choice.options, focused, cols, theme)
            }
            FieldKind::Toggle(toggle) => {
                row::toggle_row(label, toggle.on, toggle.text(), focused, cols, theme)
            }
            FieldKind::CheckGroup(group) => {
                row::check_row(label, &group.items, Some(group.at), focused, cols, theme)
            }
            FieldKind::OneOf(one_of) => {
                row::one_of_row(label, &one_of.options, one_of.chosen, focused, cols, theme)
            }
            FieldKind::ReadOnly(read_only) => {
                row::read_only_row(label, &read_only.text, read_only.tone, cols, theme)
            }
        }
    }

    /// The value of a text, path, or choice field in `width` cells; `None`
    /// for the kinds whose row has no source column.
    fn value_cells(
        &self,
        width: usize,
        focused: bool,
        theme: &Theme,
    ) -> Option<Vec<Span<'static>>> {
        match &self.kind {
            FieldKind::Text(edit) => Some(row::text_cells(edit, width, focused, theme)),
            FieldKind::Path(path) => Some(row::path_cells(path, width, focused, theme)),
            FieldKind::Choice(choice) => Some(row::choice_cells(
                &choice.edit,
                &choice.options,
                width,
                focused,
                theme,
            )),
            FieldKind::Toggle(_)
            | FieldKind::CheckGroup(_)
            | FieldKind::OneOf(_)
            | FieldKind::ReadOnly(_) => None,
        }
    }

    /// The row of a form with a source column: the label, the value
    /// filling the value column, and the column, which holds the word for
    /// where an override row's value stands and is blank on another row.
    fn sourced_line(
        &self,
        cells: Vec<Span<'static>>,
        cols: Cols,
        focused: bool,
        theme: &Theme,
    ) -> Line<'static> {
        let mut spans = vec![Span::raw(row::label(&self.label, cols.label))];
        let cells = text::fit_spans(cells, cols.value());
        let gap = cols.value().saturating_sub(text::spans_width(&cells));
        spans.extend(cells);
        if gap > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        let (word, pending) = match (&self.overrides, self.text_value()) {
            (Some(row), Some(value)) => (row.word(value), row.standing(value).is_pending()),
            _ => ("", false),
        };
        spans.extend(row::source_cells(word, cols.source, pending, theme));
        row::row(spans, focused, theme)
    }
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Field")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("kind", &self.kind)
            .field("validates", &self.validate.is_some())
            .field("conditional", &self.visible.is_some())
            .field("overrides", &self.overrides)
            .field("hints", &self.hints)
            .finish()
    }
}

/// A choice among named options, shown as chips after its value.
///
/// The option keys step the value through the options, wrapping at the
/// ends; from a value that is none of them, the next option is the first
/// and the previous one the last. A choice that
/// [allows a custom value](Self::custom) also takes typing, so it can hold
/// a value no option names, which its row marks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    edit: LineEdit,
    options: Vec<String>,
    custom: bool,
}

impl Choice {
    /// A choice holding `value` among `options`, taking no typing.
    pub fn new(value: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            edit: LineEdit::new(value),
            options,
            custom: false,
        }
    }

    /// Whether typing may give the choice a value no option names.
    #[must_use]
    pub fn custom(mut self, allow: bool) -> Self {
        self.custom = allow;
        self
    }

    /// The value, as typed or as chosen.
    pub fn value(&self) -> &str {
        self.edit.value()
    }

    /// The line editor that holds the value.
    pub fn edit(&self) -> &LineEdit {
        &self.edit
    }

    /// The options, in order.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Replaces the options; the value stays as it is.
    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options;
    }

    /// Replaces the value, the cursor at its end.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.edit = LineEdit::new(value);
    }

    /// Whether typing may give the choice a value no option names.
    pub fn allows_custom(&self) -> bool {
        self.custom
    }

    /// Whether the value is set and is none of the options.
    pub fn is_custom(&self) -> bool {
        let value = self.value().trim();
        !value.is_empty() && !self.options.iter().any(|option| option == value)
    }

    /// Steps the value `delta` options on (back when negative), wrapping;
    /// whether it changed.
    pub fn step(&mut self, delta: i32) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let value = self.value().trim();
        let index = match self.options.iter().position(|option| option == value) {
            Some(index) => Ends::Wrap.step(index, self.options.len(), delta),
            None if delta < 0 => self.options.len() - 1,
            None => 0,
        };
        let next = self.options[index].clone();
        let changed = next != self.value();
        self.set_value(next);
        changed
    }
}

/// One checkbox: on or off, with the text its row shows after the box.
///
/// The text may differ with the state ([`off_text`](Self::off_text)), for
/// a toggle whose row says what it does: `[x] create it` and `[ ] leave
/// it alone`. Every option key flips it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toggle {
    on: bool,
    on_text: String,
    off_text: String,
}

impl Toggle {
    /// A toggle, `on` or not, whose row shows `text` after the box.
    pub fn new(on: bool, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            on,
            off_text: text.clone(),
            on_text: text,
        }
    }

    /// The text the row shows while the toggle is off, when it differs.
    #[must_use]
    pub fn off_text(mut self, text: impl Into<String>) -> Self {
        self.off_text = text.into();
        self
    }

    /// Whether it is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns it on or off.
    pub fn set(&mut self, on: bool) {
        self.on = on;
    }

    /// Turns it the other way.
    pub fn flip(&mut self) {
        self.on = !self.on;
    }

    /// The text for its state.
    pub fn text(&self) -> &str {
        if self.on {
            &self.on_text
        } else {
            &self.off_text
        }
    }
}

/// A row of checkboxes, any number of them on.
///
/// A group of one item flips on any option key. A group of several has a
/// cursor of its own: `←` and `→` move it between the items, wrapping, and
/// `Space` ([`FormKey::Toggle`]) flips the item under it; while the row has
/// focus, that item is drawn in the caret's look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckGroup {
    items: Vec<(String, bool)>,
    at: usize,
}

impl CheckGroup {
    /// A group of `items`, each a text and whether it is on, its cursor
    /// on the first.
    pub fn new<S: Into<String>>(items: impl IntoIterator<Item = (S, bool)>) -> Self {
        Self {
            items: items
                .into_iter()
                .map(|(text, on)| (text.into(), on))
                .collect(),
            at: 0,
        }
    }

    /// The items: each a text and whether it is on.
    pub fn items(&self) -> &[(String, bool)] {
        &self.items
    }

    /// Whether each item is on, in order.
    pub fn checked(&self) -> Vec<bool> {
        self.items.iter().map(|(_, on)| *on).collect()
    }

    /// Turns item `index` on or off; an index past the end does nothing.
    pub fn set(&mut self, index: usize, on: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.1 = on;
        }
    }

    /// Replaces the items, keeping the cursor where it still fits.
    pub fn set_items<S: Into<String>>(&mut self, items: impl IntoIterator<Item = (S, bool)>) {
        *self = Self {
            at: self.at,
            ..Self::new(items)
        };
        self.at = self.at.min(self.items.len().saturating_sub(1));
    }

    /// The item under the group's cursor.
    pub fn at(&self) -> usize {
        self.at
    }

    /// Acts on an option key; whether an item changed.
    pub(super) fn act(&mut self, action: FormKey) -> bool {
        let len = self.items.len();
        if len == 0 {
            return false;
        }
        match action {
            FormKey::OptionPrev | FormKey::OptionNext if len > 1 => {
                let delta = if action == FormKey::OptionPrev { -1 } else { 1 };
                self.at = Ends::Wrap.step(self.at, len, delta);
                false
            }
            _ => {
                let item = &mut self.items[self.at.min(len - 1)];
                item.1 = !item.1;
                true
            }
        }
    }
}

/// A row of options of which exactly one is chosen. The option keys step
/// the choice, wrapping at the ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneOf {
    options: Vec<String>,
    chosen: usize,
}

impl OneOf {
    /// `options` with the one at `chosen` chosen (the last, when `chosen`
    /// is past the end).
    pub fn new(options: Vec<String>, chosen: usize) -> Self {
        let chosen = chosen.min(options.len().saturating_sub(1));
        Self { options, chosen }
    }

    /// The options, in order.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// The index of the chosen option.
    pub fn chosen(&self) -> usize {
        self.chosen
    }

    /// The chosen option's text; `None` when there are no options.
    pub fn chosen_text(&self) -> Option<&str> {
        self.options.get(self.chosen).map(String::as_str)
    }

    /// Chooses option `index`; an index past the end does nothing.
    pub fn choose(&mut self, index: usize) {
        if index < self.options.len() {
            self.chosen = index;
        }
    }

    /// Steps the choice `delta` options on (back when negative),
    /// wrapping; whether it changed.
    pub fn step(&mut self, delta: i32) -> bool {
        if self.options.len() < 2 {
            return false;
        }
        self.chosen = Ends::Wrap.step(self.chosen, self.options.len(), delta);
        true
    }
}

/// A line that is only read: its text and how it sits among the fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadOnly {
    text: String,
    tone: Tone,
}

impl ReadOnly {
    /// A line showing `text` in `tone`.
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The tone.
    pub fn tone(&self) -> Tone {
        self.tone
    }

    /// Replaces the text and the tone, for a line derived from fields
    /// that changed.
    pub fn set(&mut self, text: impl Into<String>, tone: Tone) {
        self.text = text.into();
        self.tone = tone;
    }
}

/// What a form's fields hold, as its closures see them: the conditions
/// that show a field and the host's `on_submit`.
///
/// Every getter takes a field's id. One asked about an id no field has,
/// or about a field of another kind, answers as for an empty field (`""`,
/// `false`, `None`), so a closure reads plainly; a typo shows up in the
/// form's own tests as a value that never changes.
#[derive(Clone, Copy, Debug)]
pub struct Values<'a> {
    fields: &'a [Field],
}

impl<'a> Values<'a> {
    /// The values of `fields`.
    pub fn of(fields: &'a [Field]) -> Self {
        Self { fields }
    }

    /// The field `id`.
    pub fn field(&self, id: &str) -> Option<&'a Field> {
        self.fields.iter().find(|field| field.id == id)
    }

    /// The text of a text, path, or choice field, as typed.
    pub fn text(&self, id: &str) -> &'a str {
        self.field(id).and_then(Field::text_value).unwrap_or("")
    }

    /// A path field, for its expanded value
    /// ([`PathField::expanded_value`]), which is what a form commits.
    pub fn path(&self, id: &str) -> Option<&'a PathField> {
        match self.field(id).map(Field::kind) {
            Some(FieldKind::Path(path)) => Some(path),
            _ => None,
        }
    }

    /// Whether a toggle is on.
    pub fn is_on(&self, id: &str) -> bool {
        matches!(self.field(id).map(Field::kind), Some(FieldKind::Toggle(toggle)) if toggle.on)
    }

    /// The index of a one-of-N field's chosen option.
    pub fn chosen(&self, id: &str) -> Option<usize> {
        match self.field(id).map(Field::kind) {
            Some(FieldKind::OneOf(one_of)) => Some(one_of.chosen),
            _ => None,
        }
    }

    /// A check group.
    pub fn check_group(&self, id: &str) -> Option<&'a CheckGroup> {
        match self.field(id).map(Field::kind) {
            Some(FieldKind::CheckGroup(group)) => Some(group),
            _ => None,
        }
    }

    /// Where an override row's value stands: what a submit should do
    /// with it.
    pub fn standing(&self, id: &str) -> Option<Standing> {
        self.field(id).and_then(Field::standing)
    }
}
