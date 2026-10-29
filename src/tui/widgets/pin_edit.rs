//! The pin edit form: every field a pin can change, listed with its
//! current value and where that value comes from.
//!
//! The form is the framework's `Form`, so it moves, submits, refuses, and
//! shows its keys the way every form does: one ring of fields (`Tab`/`↓`
//! next, `S-Tab`/`↑` previous), `Enter` saves from any of them, a field
//! that stops the save says why on the error line and takes the focus,
//! and the footer stays in view.
//!
//! The first field renames the pin (a live mux pin renames its tmux
//! session with it). Below it are the label and the fields the pin's kind
//! can override, each an override row: it shows the value in effect and
//! where that value comes from, editing it makes an override, and
//! `Ctrl-R` drops the override so the field follows the entity again.
//! Paths complete with `Tab`, the harness is chosen among the known
//! harnesses, and a harness's launch options (skipping permissions) are
//! checkboxes that add or remove their flags in the argv.
//!
//! This module keeps what only Conspectus knows: which fields each kind
//! of pin has and what they are labelled; the words for where a value
//! comes from (`running`, `last seen`, `saved`, `default`) and what the
//! focused row says about following; the rename's checks; and the save
//! ([`PinEditRequest`], which the runtime writes to the pin store). The
//! launch options and the argv a harness change carries are kept in step
//! after each key with the launch form's helpers (`launch_argv`). The
//! default argv is decided when the form opens and again only when the
//! harness or the cwd changes, so drawing the form reads no files.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use tuikit::form::{
    CheckGroup, Choice, Field, FieldKind, Form, OverrideRow, OverrideView, ReadOnly, Standing,
    Submit, Tone, Values,
};
use tuikit::keys::{FormKey, resolve};
use tuikit::overlay::{Outcome, Overlay};
use tuikit::path_field::{Candidate, PathField};
use tuikit::text_edit::LineEdit;

use crate::argv::split_command_line;
use crate::discovery::harness::launch_options_for;
use crate::launch_default::LaunchDefaults;
use crate::model::{FieldSource, FieldValue, PinKind, RecipeField, SlotFields};
use crate::tui::Msg;
use crate::tui::keybindings::overlay::PIN_EDIT_KEYS;
use crate::tui::widgets::launch_argv::{self, DefaultArgv};

/// The form's title.
const TITLE: &str = "Edit pin";

/// The read-only line naming the pin, its kind, and its status.
pub const SUMMARY: &str = "summary";
/// The pin's own name: the first field, which renames the pin.
pub const PIN: &str = "pin";
/// The note under the pin name of a live mux pin.
pub const RENAMES: &str = "renames";
/// The harness's launch options, under the harness of a mux pin.
pub const OPTIONS: &str = "options";
/// The mux harness, among the known harnesses.
const HARNESS: &str = "harness";
/// The mux cwd, which the default argv depends on.
const CWD: &str = "cwd";
/// The mux argv.
const ARGV: &str = "argv";

/// Lines under the rows that explain the focused field.
const DETAIL_LINES: usize = 2;

/// What the form saves: a rename, overrides to set, overrides to drop.
#[derive(Debug, Clone, PartialEq)]
pub struct PinEditRequest {
    pub pin: String,
    pub rename: Option<String>,
    pub set: Vec<(String, FieldValue)>,
    pub unset: Vec<String>,
}

impl PinEditRequest {
    /// Whether saving it would change nothing.
    fn is_empty(&self) -> bool {
        self.rename.is_none() && self.set.is_empty() && self.unset.is_empty()
    }
}

/// What opening the form needs to know about the pin.
#[derive(Debug, Clone)]
pub struct PinEditInit {
    pub pin: String,
    pub kind: PinKind,
    /// The status word shown next to the kind (`live`, `gone`, ...).
    pub status: String,
    /// Whether this is a live mux pin, whose rename renames the session.
    pub live_mux: bool,
    /// Each field's value in effect and the layer it came from.
    pub effective: Vec<RecipeField>,
    /// The overrides the pin has now.
    pub overrides: SlotFields,
    /// Each field's value without the pin's overrides.
    pub inherited: Vec<RecipeField>,
    pub known_pin_names: Vec<String>,
    pub known_harness_keys: Vec<String>,
    pub cwd_candidates: Vec<Candidate>,
    pub defaults: Option<LaunchDefaults>,
}

/// The pin's recipe fields the form edits, in order: the label, then the
/// fields its kind can override.
fn recipe_fields(kind: PinKind) -> impl Iterator<Item = &'static str> {
    std::iter::once("label").chain(kind.overridable_fields().iter().copied())
}

/// The label a recipe field's row shows: the field's name in the short
/// vocabulary the other forms use.
pub fn label_for(field: &str) -> &str {
    match field {
        "worktree_branch" => "branch",
        "resume_into" => "resume in",
        other => other,
    }
}

/// Whether `field` of a `kind` pin holds a directory.
fn is_path(kind: PinKind, field: &str) -> bool {
    matches!(
        (kind, field),
        (PinKind::Mux, "cwd") | (PinKind::Checkout, "location") | (PinKind::Repo, "path")
    )
}

/// The pin edit form: the framework form and what keeps the launch
/// options and the argv in step.
#[derive(Debug, Clone)]
pub struct PinEditForm {
    form: Form<Msg>,
    pin: String,
    kind: PinKind,
    known_pin_names: Vec<String>,
    /// Where an empty argv comes from.
    defaults: Option<LaunchDefaults>,
    /// The argv an empty argv means for the harness and cwd as they are
    /// now, decided when either changes so drawing reads it from here.
    default: DefaultArgv,
}

impl PinEditForm {
    /// The form for the pin `init` describes, the focus on its name.
    pub fn new(init: PinEditInit) -> Self {
        let PinEditInit {
            pin,
            kind,
            status,
            live_mux,
            effective,
            overrides,
            inherited,
            known_pin_names,
            known_harness_keys,
            cwd_candidates,
            defaults,
        } = init;
        let submit = {
            let pin = pin.clone();
            move |values: &Values<'_>| save(&pin, kind, values)
        };
        let mut form = Form::new(TITLE, PIN_EDIT_KEYS, submit)
            .intro(intro(kind))
            .details(DETAIL_LINES)
            .field(Field::read_only(
                SUMMARY,
                "pin",
                ReadOnly::new(format!("{pin} · {} · {status}", kind.as_str()), Tone::Note),
            ))
            .field(
                Field::text(PIN, "pin name", pin.clone())
                    .validate(name_check(pin.clone(), known_pin_names.clone()))
                    .hint("the name this pin is listed under"),
            );
        if live_mux {
            form = form.field(Field::read_only(
                RENAMES,
                "",
                ReadOnly::new("renames the tmux session too", Tone::Note),
            ));
        }
        for field in recipe_fields(kind) {
            let (value, source) = lookup(&effective, field);
            let overridden = overrides.contains_key(field);
            // A field with an override would follow its value without the
            // pin's overrides; one without follows what it has now, which
            // can come from another field's override (the default argv of
            // an overridden harness).
            let (follows, layer) = if overridden {
                lookup(&inherited, field)
            } else {
                (value.clone(), source)
            };
            let row = OverrideRow::new(follows, layer_word(layer))
                .overridden(overridden)
                .hint(hint(kind, layer));
            let label = label_for(field);
            let entry = if is_path(kind, field) {
                Field::path(
                    field,
                    label,
                    crate::tui::paths::directory_field(value, cwd_candidates.clone()),
                )
                .with_override(row)
                .hint("Tab completes the path")
            } else if (kind, field) == (PinKind::Mux, HARNESS) {
                Field::choice(
                    field,
                    label,
                    Choice::new(value, known_harness_keys.clone()).custom(true),
                )
                .with_override(row)
                .hint("←/→ choose a harness")
            } else if field == ARGV {
                Field::text(field, label, value)
                    .with_override(row)
                    .validate(move |text| override_check(kind, ARGV, text))
            } else {
                Field::text(field, label, value).with_override(row)
            };
            form = form.field(entry);
            if (kind, field) == (PinKind::Mux, HARNESS) {
                form = form.field(
                    Field::check_group(
                        OPTIONS,
                        OPTIONS,
                        CheckGroup::new(Vec::<(String, bool)>::new()),
                    )
                    .visible_if(|values| {
                        !launch_options_for(values.text(HARNESS).trim()).is_empty()
                    })
                    .hint("launch options for this harness")
                    .hint("Space or ←/→ turns an option on or off"),
                );
            }
        }
        let mut edit = Self {
            form,
            pin,
            kind,
            known_pin_names,
            defaults,
            default: DefaultArgv::default(),
        };
        edit.default = edit.decide_default();
        edit.follow();
        edit
    }

    /// The pin the form edits, by its name when the form opened.
    pub fn pin(&self) -> &str {
        &self.pin
    }

    /// The framework form: its fields, values, focus, and error line.
    pub fn form(&self) -> &Form<Msg> {
        &self.form
    }

    /// The reason the last save was refused, until the next edit.
    pub fn error(&self) -> Option<&str> {
        self.form.error()
    }

    /// The text of the row for recipe field `field` (`None` for a field
    /// the pin's kind does not have).
    pub fn field_text(&self, field: &str) -> Option<&str> {
        self.form
            .get(field)
            .filter(|_| field != PIN)
            .and_then(Field::text_value)
    }

    /// The pin name row's text.
    pub fn name_text(&self) -> &str {
        self.form.values().text(PIN)
    }

    /// The labels of the pin name and recipe field rows, in order.
    pub fn row_labels(&self) -> Vec<&str> {
        self.form
            .fields()
            .iter()
            .filter(|field| field.id() == PIN || field.override_row().is_some())
            .map(Field::label)
            .collect()
    }

    /// The id of the field that has the focus.
    pub fn focus(&self) -> Option<&'static str> {
        self.form.focus()
    }

    /// The label of the row that has the focus.
    pub fn focused_label(&self) -> Option<&str> {
        let id = self.form.focus()?;
        self.form.get(id).map(Field::label)
    }

    /// Whether typing edits the focused field, so a paste belongs to it.
    pub fn takes_text(&self) -> bool {
        self.form.takes_text()
    }

    /// Draws the form over `area`.
    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: &tuikit::theme::Theme) {
        self.form.draw(area, buf, theme);
    }

    /// The request the form would save, or why it can't.
    pub fn request(&self) -> Result<PinEditRequest, String> {
        let values = self.form.values();
        check_name(&self.pin, &self.known_pin_names, values.text(PIN))?;
        build_request(&self.pin, self.kind, &values)
    }

    /// One key: the form edits, moves, resets, or saves; then the fields
    /// that follow others catch up. Stepping the harness carries a typed
    /// argv across, and flipping a launch option writes its flags into
    /// the argv.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        let steps_harness = self.focus() == Some(HARNESS)
            && matches!(
                resolve(PIN_EDIT_KEYS, key),
                Some(FormKey::OptionPrev | FormKey::OptionNext | FormKey::Toggle)
            );
        let old_default = self.default.argv().to_vec();
        let outcome = Overlay::<Msg, ()>::handle(&mut self.form, key, &());
        match self.form.edited() {
            Some(HARNESS) if steps_harness => self.carry_argv(&old_default),
            Some(OPTIONS) => self.apply_options(),
            _ => {}
        }
        self.follow();
        outcome
    }

    /// The text of field `id`, as typed.
    fn text(&self, id: &str) -> &str {
        self.form.values().text(id)
    }

    /// The cwd the default argv is decided for, `~` expanded.
    fn cwd(&self) -> String {
        self.form
            .values()
            .path(CWD)
            .map(PathField::expanded_value)
            .unwrap_or_default()
    }

    /// The argv a mux pin with this harness and cwd starts from when it
    /// has none of its own: a profile, `atelier exec`, or the harness
    /// default. Nothing for the other kinds, which have no argv.
    fn decide_default(&self) -> DefaultArgv {
        if self.kind != PinKind::Mux {
            return DefaultArgv::default();
        }
        DefaultArgv::resolve(
            self.defaults.as_ref(),
            self.text(HARNESS).trim(),
            &self.cwd(),
        )
    }

    /// The argv the pin launches: the argv row read as far as it goes,
    /// or the default when it is empty.
    fn argv_now(&self) -> Vec<String> {
        if self.form.get(ARGV).is_none() {
            return Vec::new();
        }
        launch_argv::effective(split_command_line(self.text(ARGV)), self.default.argv())
    }

    fn set_argv(&mut self, argv: &[String]) {
        if let Some(FieldKind::Text(edit)) = self.form.get_mut(ARGV).map(Field::kind_mut) {
            *edit = LineEdit::new(launch_argv::display(argv));
        }
    }

    /// After a step of the harness: a typed argv loses the old harness's
    /// flags, and one that was only the old default follows the new one.
    fn carry_argv(&mut self, old_default: &[String]) {
        let typed = split_command_line(self.text(ARGV));
        if typed.is_empty() {
            return;
        }
        let (harness, cwd) = (self.text(HARNESS).trim().to_string(), self.cwd());
        self.default.refresh(self.defaults.as_ref(), &harness, &cwd);
        let next = launch_argv::carried(typed, old_default, self.default.argv());
        self.set_argv(&next);
    }

    /// After a launch option flipped: the argv takes the options as the
    /// checkboxes now say.
    fn apply_options(&mut self) {
        let options = launch_options_for(self.text(HARNESS).trim());
        let Some(checked) = self
            .form
            .values()
            .check_group(OPTIONS)
            .map(CheckGroup::checked)
        else {
            return;
        };
        let mut argv = self.argv_now();
        for (option, on) in options.iter().zip(checked) {
            argv = launch_argv::with_option(argv, *option, on);
        }
        self.set_argv(&argv);
    }

    /// Brings the fields that follow others up to date: the default argv
    /// (decided again only when the harness or the cwd changed) and the
    /// launch options' checkboxes.
    fn follow(&mut self) {
        if self.kind != PinKind::Mux {
            return;
        }
        let (harness, cwd) = (self.text(HARNESS).trim().to_string(), self.cwd());
        self.default.refresh(self.defaults.as_ref(), &harness, &cwd);
        let argv = self.argv_now();
        let options = launch_options_for(&harness);
        let items: Vec<(String, bool)> = options
            .iter()
            .zip(launch_argv::options_on(options, &argv))
            .map(|(option, on)| (option.label.to_string(), on))
            .collect();
        if let Some(FieldKind::CheckGroup(group)) = self.form.get_mut(OPTIONS).map(Field::kind_mut)
        {
            group.set_items(items);
        }
    }
}

/// What the paragraph above the rows says about following.
fn intro(kind: PinKind) -> String {
    format!(
        "A blank field follows the running {}, then its last seen value, then what was saved \
         with the pin. Type to keep a value of your own with this pin; Ctrl-R goes back to \
         following.",
        kind.as_str()
    )
}

fn text_of(value: &FieldValue) -> String {
    match value {
        FieldValue::Text(text) => text.clone(),
        FieldValue::List(list) => launch_argv::display(list),
    }
}

/// `field`'s value in `fields`, as the form shows it, and its layer.
fn lookup(fields: &[RecipeField], name: &str) -> (String, Option<FieldSource>) {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| (text_of(&field.value), Some(field.source)))
        .unwrap_or_default()
}

/// The word for where a value comes from.
fn layer_word(layer: Option<FieldSource>) -> &'static str {
    match layer {
        Some(FieldSource::Live) => "running",
        Some(FieldSource::History) => "last seen",
        Some(FieldSource::Snapshot) => "saved",
        Some(FieldSource::Default) => "default",
        Some(FieldSource::Override) => "kept",
        None => "unset",
    }
}

/// What following `layer` gives a `kind` pin's field: `value`, from
/// where.
fn following(kind: PinKind, layer: Option<FieldSource>, value: &str) -> String {
    match layer {
        Some(FieldSource::Live) => format!("following the running {}: {value}", kind.as_str()),
        Some(FieldSource::History) => format!("following the last seen value: {value}"),
        Some(FieldSource::Snapshot) => format!("following the value saved with the pin: {value}"),
        Some(FieldSource::Default) => format!("following the default: {value}"),
        Some(FieldSource::Override) | None => {
            "nothing to follow yet; type a value to keep with this pin".to_string()
        }
    }
}

/// What the focused override row says about itself: where its value
/// stands, and what `Ctrl-R` would go back to. `layer` is where the
/// value it follows without an override comes from.
fn hint(
    kind: PinKind,
    layer: Option<FieldSource>,
) -> impl Fn(&OverrideView<'_>) -> Vec<String> + Send + Sync + 'static {
    move |view| {
        let value = view.value.trim();
        let back = || match layer {
            Some(_) => format!(
                "Ctrl-R goes back to {}",
                following(kind, layer, view.inherited).replacen("following", "following —", 1)
            ),
            None => "Ctrl-R drops it; there is nothing to follow yet".to_string(),
        };
        match view.standing {
            Standing::Follows => vec![following(kind, layer, value)],
            Standing::Kept => vec![format!("kept with this pin: {value}"), back()],
            Standing::Changed => {
                let mut lines = vec![format!("will be kept with this pin: {value}")];
                if layer.is_some() {
                    lines.push(back());
                }
                lines
            }
            Standing::Reset => vec![
                "will follow again when saved".to_string(),
                following(kind, layer, view.inherited),
            ],
        }
    }
}

/// Refuses a new pin name that is not a valid pin id or that another pin
/// has. The name the pin has now is always fine.
fn check_name(pin: &str, known: &[String], text: &str) -> Result<(), String> {
    let name = text.trim();
    if name == pin {
        return Ok(());
    }
    crate::pin_slots::validate_id(name).map_err(|err| err.to_string())?;
    if known.iter().any(|known| known == name) {
        return Err(format!("a pin named `{name}` already exists"));
    }
    Ok(())
}

/// The pin name field's check.
fn name_check(
    pin: String,
    known: Vec<String>,
) -> impl Fn(&str) -> Result<(), String> + Send + Sync {
    move |text: &str| check_name(&pin, &known, text)
}

/// Refuses a value `field` cannot hold, as `pin edit --set` would (an
/// argv that ends inside a quote).
fn override_check(kind: PinKind, field: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    crate::pin_ops::parse_override(kind, &format!("{field}={text}"))
        .map(drop)
        .map_err(|err| err.to_string())
}

/// The value saving commits for `field`: trimmed, and with `~` expanded
/// in a path, which the commands that read the pin take as given.
fn committed(values: &Values<'_>, field: &str) -> String {
    values.path(field).map_or_else(
        || values.text(field).trim().to_string(),
        PathField::expanded_value,
    )
}

/// The save the values ask for: a rename when the name changed; each
/// changed field kept as an override (an emptied override dropped); each
/// reset field's override dropped.
fn build_request(pin: &str, kind: PinKind, values: &Values<'_>) -> Result<PinEditRequest, String> {
    let name = values.text(PIN).trim();
    let rename = (name != pin).then(|| name.to_string());
    let mut set = Vec::new();
    let mut unset = Vec::new();
    for field in recipe_fields(kind) {
        let overridden = values
            .field(field)
            .and_then(Field::override_row)
            .is_some_and(OverrideRow::is_overridden);
        match values.standing(field) {
            Some(Standing::Reset) => unset.push(field.to_string()),
            Some(Standing::Changed) => {
                let text = committed(values, field);
                if text.is_empty() {
                    if overridden {
                        unset.push(field.to_string());
                    }
                } else {
                    let assignment = format!("{field}={text}");
                    set.push(
                        crate::pin_ops::parse_override(kind, &assignment)
                            .map_err(|err| err.to_string())?,
                    );
                }
            }
            Some(Standing::Follows | Standing::Kept) | None => {}
        }
    }
    Ok(PinEditRequest {
        pin: pin.to_string(),
        rename,
        set,
        unset,
    })
}

/// What `Enter` does: save the changes, close when there are none, or
/// say why the values cannot be saved. The fields' checks (the name, the
/// argv) have passed.
fn save(pin: &str, kind: PinKind, values: &Values<'_>) -> Submit<Msg> {
    match build_request(pin, kind, values) {
        Ok(request) if request.is_empty() => Submit::Close,
        Ok(request) => Submit::Commit(Msg::CommitPinEdit(request)),
        Err(reason) => Submit::Reject(reason),
    }
}

#[cfg(test)]
#[path = "pin_edit_tests.rs"]
mod tests;
