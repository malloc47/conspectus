use std::path::{Path, PathBuf};
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::glyphs::marks;
use crate::keys::{
    Convention, EnterMeans, FORM_KEYS, FORM_OPTION_KEYS, Footer, SpaceMeans, ctrl as ctrl_key,
    include, key as bind, shown,
};
use crate::overlay::ModalStack;
use crate::path_field::{PathField, PathKind};
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::conformance::{self, Case, Registry, Rig};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{chord, ctrl, press};
use crate::testkit::paths::MemoryPaths;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Made {
        name: String,
        place: String,
        copy: bool,
        size: Option<usize>,
    },
}

/// A form with option rows: moving, the option keys, `Enter make`.
const KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(ctrl_key('r'), FormKey::Inherit, "Ctrl-R", "reset"),
    shown(bind(KeyCode::Enter), FormKey::Submit, "Enter", "make"),
    shown(bind(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

/// A form of text and path fields only: the option keys edit.
const TEXT_KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    shown(bind(KeyCode::Enter), FormKey::Submit, "Enter", "make"),
    shown(bind(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

fn paths() -> Arc<MemoryPaths> {
    Arc::new(
        MemoryPaths::new()
            .with_home("/home/me")
            .dir("/home/me/projects/alpha")
            .dir("/srv"),
    )
}

fn required(what: &'static str) -> impl Fn(&str) -> Result<(), String> + Send + Sync {
    move |text: &str| {
        if text.trim().is_empty() {
            Err(format!("{what} is required"))
        } else {
            Ok(())
        }
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_string()).collect()
}

/// A form of every kind of field:
///
/// - `name` (text, required), `place` (a directory, required),
/// - `copy` (a toggle) and `copies` (text, shown only while `copy` is on),
/// - `size` (one of three), `parts` (a check group of two), `tool` (a
///   choice that takes a custom value), and
/// - `about` (a note), which is never a stop.
fn form() -> Form<Msg> {
    Form::new("Make a thing", KEYS, |values| {
        Submit::Commit(Msg::Made {
            name: values.text("name").trim().to_string(),
            place: values
                .path("place")
                .map(PathField::expanded_value)
                .unwrap_or_default(),
            copy: values.is_on("copy"),
            size: values.chosen("size"),
        })
    })
    .field(Field::read_only(
        "about",
        "about",
        ReadOnly::new("a thing to make", Tone::Note),
    ))
    .field(Field::text("name", "name", "").validate(required("a name")))
    .field(
        Field::path(
            "place",
            "place",
            PathField::new("~/projects/alpha", PathKind::Dir, paths()),
        )
        .validate(required("a place")),
    )
    .field(Field::toggle(
        "copy",
        "copy",
        Toggle::new(false, "keep a copy").off_text("no copy"),
    ))
    .field(
        Field::text("copies", "copies", "1")
            .validate(required("a count"))
            .visible_if(|values| values.is_on("copy")),
    )
    .field(Field::one_of(
        "size",
        "size",
        OneOf::new(names(&["small", "medium", "large"]), 0),
    ))
    .field(Field::check_group(
        "parts",
        "parts",
        CheckGroup::new([("lid", false), ("base", true)]),
    ))
    .field(Field::choice(
        "tool",
        "tool",
        Choice::new("saw", names(&["saw", "drill"])).custom(true),
    ))
}

fn key(form: &mut Form<Msg>, event: KeyEvent) -> Outcome<Msg> {
    form.handle(event, &())
}

fn type_text(form: &mut Form<Msg>, text: &str) {
    for c in text.chars() {
        key(form, press(KeyCode::Char(c)));
    }
}

/// Moves the focus to `id` with `↓`.
fn go(form: &mut Form<Msg>, id: &str) {
    for _ in 0..form.fields().len() {
        if form.focus() == Some(id) {
            return;
        }
        key(form, press(KeyCode::Down));
    }
    panic!("no stop {id}");
}

fn drawn(form: &Form<Msg>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    form.draw(buf.area, &mut buf, &Theme::default());
    buf
}

fn screen(form: &Form<Msg>, width: u16, height: u16) -> String {
    rows(&drawn(form, width, height)).join("\n")
}

// ---- the ring --------------------------------------------------------------

#[test]
fn the_focus_starts_on_the_first_stop_and_a_read_only_line_is_never_one() {
    let mut form = form();
    assert_eq!(
        form.focus(),
        Some("name"),
        "the note above it is not a stop"
    );
    let mut seen = Vec::new();
    for _ in 0..7 {
        seen.push(form.focus().expect("a focus"));
        key(&mut form, press(KeyCode::Down));
    }
    assert_eq!(
        seen,
        ["name", "place", "copy", "size", "parts", "tool", "name"],
        "Down visits each shown stop once and wraps; the hidden field is skipped"
    );
}

#[test]
fn tab_and_down_go_on_and_shift_tab_and_up_go_back_over_one_ring() {
    let mut form = form();
    key(&mut form, press(KeyCode::Down));
    assert_eq!(form.focus(), Some("place"));
    key(&mut form, press(KeyCode::Up));
    assert_eq!(form.focus(), Some("name"));
    key(&mut form, press(KeyCode::Up));
    assert_eq!(form.focus(), Some("tool"), "Up wraps to the last stop");
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(form.focus(), Some("name"), "Tab wraps to the first");
    key(&mut form, press(KeyCode::BackTab));
    assert_eq!(form.focus(), Some("tool"), "S-Tab goes back like Up");
    key(&mut form, ctrl('n'));
    assert_eq!(form.focus(), Some("name"), "Ctrl-N goes on like Down");
    key(&mut form, ctrl('p'));
    assert_eq!(form.focus(), Some("tool"), "Ctrl-P goes back like Up");
    assert_eq!(form.edited(), None, "moving changes nothing");
}

#[test]
fn tab_completes_a_path_before_it_moves_on() {
    let mut form = form();
    go(&mut form, "place");
    key(&mut form, chord(KeyModifiers::CONTROL, KeyCode::Char('u')));
    type_text(&mut form, "~/proj");
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(form.focus(), Some("place"), "it completed, so it stayed");
    assert_eq!(form.values().text("place"), "~/projects");
    assert_eq!(form.edited(), Some("place"));
    type_text(&mut form, "/");
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(form.values().text("place"), "~/projects/alpha");
    assert_eq!(form.focus(), Some("place"));
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(form.focus(), Some("copy"), "nothing left to complete");
    assert_eq!(form.edited(), None, "moving on changed nothing");
}

#[test]
fn a_field_shown_by_a_toggle_joins_the_ring_after_it() {
    let mut form = form();
    go(&mut form, "copy");
    key(&mut form, press(KeyCode::Char(' ')));
    assert!(form.values().is_on("copy"));
    key(&mut form, press(KeyCode::Down));
    assert_eq!(form.focus(), Some("copies"));
    key(&mut form, press(KeyCode::Up));
    key(&mut form, press(KeyCode::Char(' ')));
    key(&mut form, press(KeyCode::Down));
    assert_eq!(form.focus(), Some("size"), "hidden again, so skipped");
}

#[test]
fn the_focus_leaves_a_field_the_host_hides() {
    let mut form = form();
    go(&mut form, "copy");
    key(&mut form, press(KeyCode::Char(' ')));
    key(&mut form, press(KeyCode::Down));
    assert_eq!(form.focus(), Some("copies"));
    // The host turns the toggle off under the focused field; the next key
    // finds the focus on the stop that took its place.
    if let Some(FieldKind::Toggle(toggle)) = form.get_mut("copy").map(Field::kind_mut) {
        toggle.set(false);
    }
    assert_eq!(form.focus(), Some("size"), "the next stop took its place");
    type_text(&mut form, "2");
    assert_eq!(
        form.values().text("copies"),
        "1",
        "a hidden field takes no keys"
    );
    key(&mut form, press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        Some("parts"),
        "moving starts where the cursor shows"
    );
    assert!(form.focus_on("tool"));
    assert!(
        !form.focus_on("copies"),
        "a hidden field cannot take the focus"
    );
    assert!(!form.focus_on("about"), "nor can a read-only line");
    assert_eq!(form.focus(), Some("tool"));
}

// ---- submitting ------------------------------------------------------------

#[test]
fn enter_submits_from_any_field() {
    for stops in 0..6 {
        let mut form = form();
        type_text(&mut form, "box");
        for _ in 0..stops {
            key(&mut form, press(KeyCode::Down));
        }
        let outcome = key(&mut form, press(KeyCode::Enter));
        assert_eq!(
            outcome,
            Outcome::Commit(Msg::Made {
                name: "box".into(),
                place: "/home/me/projects/alpha".into(),
                copy: false,
                size: Some(0),
            }),
            "Enter on the field {stops} stops down"
        );
    }
}

#[test]
fn a_failed_check_explains_itself_and_takes_the_focus() {
    let mut form = form();
    go(&mut form, "size");
    assert_eq!(key(&mut form, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(form.error(), Some("a name is required"));
    assert_eq!(form.focus(), Some("name"), "the field the reason is about");
    type_text(&mut form, "box");
    assert_eq!(form.error(), None, "the next edit clears it");
    go(&mut form, "place");
    key(&mut form, chord(KeyModifiers::CONTROL, KeyCode::Char('u')));
    go(&mut form, "tool");
    key(&mut form, press(KeyCode::Enter));
    assert_eq!(form.error(), Some("a place is required"));
    assert_eq!(form.focus(), Some("place"));
}

#[test]
fn a_hidden_field_is_not_checked() {
    let mut form = form();
    type_text(&mut form, "box");
    if let Some(FieldKind::Text(edit)) = form.get_mut("copies").map(Field::kind_mut) {
        edit.set_value("");
    }
    assert!(matches!(
        key(&mut form, press(KeyCode::Enter)),
        Outcome::Commit(_)
    ));
    let mut shown = form.clone();
    go(&mut shown, "copy");
    key(&mut shown, press(KeyCode::Char(' ')));
    key(&mut shown, press(KeyCode::Enter));
    assert_eq!(shown.error(), Some("a count is required"));
    assert_eq!(shown.focus(), Some("copies"));
}

fn rejecting(submit: Submit<Msg>) -> Form<Msg> {
    Form::new("Make a thing", TEXT_KEYS, move |_| submit.clone())
        .field(Field::text("name", "name", "box"))
}

#[test]
fn on_submit_can_refuse_with_a_reason_or_close() {
    let mut form = rejecting(Submit::Reject("that name is taken".into()));
    assert_eq!(key(&mut form, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(form.error(), Some("that name is taken"));
    key(&mut form, press(KeyCode::Left));
    assert_eq!(
        form.error(),
        Some("that name is taken"),
        "a move edits nothing"
    );
    key(&mut form, press(KeyCode::Down));
    assert_eq!(
        form.error(),
        Some("that name is taken"),
        "nor does moving on"
    );
    key(&mut form, press(KeyCode::Backspace));
    assert_eq!(form.error(), None);
    let mut closing = rejecting(Submit::Close);
    assert_eq!(key(&mut closing, press(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn esc_and_ctrl_c_close_and_q_is_typed() {
    let mut form = form();
    type_text(&mut form, "q");
    assert_eq!(form.values().text("name"), "q", "a form is a typing level");
    assert_eq!(key(&mut form, press(KeyCode::Esc)), Outcome::Close);
    assert_eq!(key(&mut form, ctrl('c')), Outcome::Close);
    go(&mut form, "size");
    assert_eq!(
        key(&mut form, press(KeyCode::Char('q'))),
        Outcome::Consumed,
        "q types on an option row too, where it does nothing"
    );
}

// ---- each kind of field ----------------------------------------------------

#[test]
fn the_option_keys_edit_a_text_field() {
    let mut form = form();
    type_text(&mut form, "ab");
    key(&mut form, press(KeyCode::Left));
    key(&mut form, press(KeyCode::Char(' ')));
    assert_eq!(form.values().text("name"), "a b", "Left moved, Space typed");
    key(&mut form, press(KeyCode::Right));
    type_text(&mut form, "!");
    assert_eq!(form.values().text("name"), "a b!");
}

#[test]
fn the_option_keys_flip_a_toggle() {
    let mut form = form();
    go(&mut form, "copy");
    for (code, on) in [
        (KeyCode::Right, true),
        (KeyCode::Left, false),
        (KeyCode::Char(' '), true),
    ] {
        key(&mut form, press(code));
        assert_eq!(form.values().is_on("copy"), on, "{code:?}");
        assert_eq!(form.edited(), Some("copy"));
    }
}

#[test]
fn the_option_keys_step_a_one_of_row_and_wrap() {
    let mut form = form();
    go(&mut form, "size");
    key(&mut form, press(KeyCode::Right));
    assert_eq!(form.values().chosen("size"), Some(1));
    key(&mut form, press(KeyCode::Char(' ')));
    assert_eq!(form.values().chosen("size"), Some(2));
    key(&mut form, press(KeyCode::Right));
    assert_eq!(form.values().chosen("size"), Some(0), "wraps on");
    key(&mut form, press(KeyCode::Left));
    assert_eq!(form.values().chosen("size"), Some(2), "and back");
}

#[test]
fn a_check_group_moves_its_cursor_with_the_arrows_and_flips_with_space() {
    let mut form = form();
    go(&mut form, "parts");
    let group = |form: &Form<Msg>| form.values().check_group("parts").cloned().expect("group");
    key(&mut form, press(KeyCode::Char(' ')));
    assert_eq!(group(&form).checked(), [true, true]);
    key(&mut form, press(KeyCode::Right));
    assert_eq!(group(&form).at(), 1);
    assert_eq!(form.edited(), None, "moving its cursor changes nothing");
    key(&mut form, press(KeyCode::Char(' ')));
    assert_eq!(group(&form).checked(), [true, false]);
    key(&mut form, press(KeyCode::Right));
    assert_eq!(group(&form).at(), 0, "wraps");

    let mut single = CheckGroup::new([("only", false)]);
    assert!(
        single.act(FormKey::OptionNext),
        "one item flips on any option key"
    );
    assert!(single.act(FormKey::OptionPrev));
    assert_eq!(single.checked(), [false]);
}

#[test]
fn a_choice_steps_through_its_options_and_takes_a_typed_value() {
    let mut form = form();
    go(&mut form, "tool");
    key(&mut form, press(KeyCode::Right));
    assert_eq!(form.values().text("tool"), "drill");
    key(&mut form, press(KeyCode::Right));
    assert_eq!(form.values().text("tool"), "saw", "wraps");
    type_text(&mut form, "s");
    assert_eq!(
        form.values().text("tool"),
        "saws",
        "a custom value is typed"
    );
    key(&mut form, press(KeyCode::Left));
    assert_eq!(
        form.values().text("tool"),
        "drill",
        "from a custom value, back is the last"
    );
    assert!(form.takes_text());

    let mut fixed = Choice::new("", names(&["one", "two"]));
    assert!(fixed.step(1));
    assert_eq!(fixed.value(), "one", "from nothing, on is the first");
    assert!(!fixed.allows_custom());
    let mut plain = Form::new("Pick", KEYS, |_| Submit::<Msg>::Close)
        .field(Field::choice("pick", "pick", fixed));
    plain.handle(press(KeyCode::Char('x')), &());
    assert_eq!(
        plain.values().text("pick"),
        "one",
        "no typing without custom"
    );
    assert!(!plain.takes_text());
}

/// The reset key acts on override rows only (their tests are with
/// `OverrideRow`); this form has none, so it changes nothing, and it
/// never reaches the field's text.
#[test]
fn the_reset_key_leaves_a_field_without_an_override_alone() {
    let mut form = form();
    type_text(&mut form, "box");
    assert_eq!(key(&mut form, ctrl('r')), Outcome::Consumed);
    assert_eq!(form.values().text("name"), "box");
    assert_eq!(form.edited(), None);
}

#[test]
fn only_the_fields_that_take_text_take_a_paste() {
    let mut form = form();
    let mut takes = Vec::new();
    for _ in 0..6 {
        takes.push((form.focus().expect("a focus"), form.takes_text()));
        key(&mut form, press(KeyCode::Down));
    }
    assert_eq!(
        takes,
        [
            ("name", true),
            ("place", true),
            ("copy", false),
            ("size", false),
            ("parts", false),
            ("tool", true),
        ]
    );
}

// ---- drawing ---------------------------------------------------------------

#[test]
fn the_rows_share_one_grid_and_the_focused_one_carries_the_cursor() {
    let form = form();
    let buf = drawn(&form, 80, 24);
    let at = |needle: &str| find(&buf, needle).unwrap_or_else(|| panic!("{needle} drawn"));
    let name = at("name ");
    let place = at("place ");
    assert_eq!(name.x, place.x, "labels share a column");
    let values = [
        at("a thing to make"),
        at("~/projects/alpha"),
        at("[ ] no copy"),
        at("(•) small"),
        at("[ ] lid"),
    ];
    for value in values {
        assert_eq!(value.x, values[0].x, "values share a column");
    }
    let pointer = at(marks::POINTER);
    assert_eq!(pointer.y, name.y, "the cursor is on the focused field");
    assert!(screen(&form, 80, 24).contains("( ) medium ( ) large"));
}

#[test]
fn the_error_line_sits_above_the_fixed_footer() {
    let mut form = form();
    key(&mut form, press(KeyCode::Enter));
    let buf = drawn(&form, 80, 24);
    let error = find(&buf, "a name is required").expect("the error line");
    let footer = find(&buf, "↑/↓ move").expect("the footer");
    assert_eq!(error.y + 1, footer.y, "the error is in the footer slot");
    assert_eq!(
        style_at(&buf, "a name is required").and_then(|style| style.fg),
        Some(Theme::default().error)
    );
}

#[test]
fn on_a_short_screen_the_footer_stays_and_the_rows_follow_the_cursor() {
    let mut form = form();
    go(&mut form, "tool");
    key(&mut form, press(KeyCode::Enter));
    form.focus_on("tool");
    // Forty columns by ten rows: six inside the frame.
    let text = screen(&form, 40, 10);
    let footer = Footer::new(KEYS);
    for segment in footer.segments() {
        assert!(
            text.contains(&segment.text),
            "{} missing:\n{text}",
            segment.text
        );
    }
    assert!(
        text.contains("tool"),
        "the focused row stays in view:\n{text}"
    );
    assert!(
        !text.contains("~/projects/alpha"),
        "rows above it scrolled away:\n{text}"
    );
    assert!(text.contains("a name is required"), "{text}");
}

#[test]
fn the_frame_grows_by_a_row_while_an_error_shows() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 80, 30);
    let mut form = form();
    let before = form.rect(area, &theme);
    key(&mut form, press(KeyCode::Enter));
    let after = form.rect(area, &theme);
    assert_eq!(after.height, before.height + 1);
    assert_eq!(before.width, 80 - 4, "the form size, two columns clear");
}

#[test]
fn drawing_asks_the_path_source_nothing() {
    let source = paths();
    let form = Form::new("Where", TEXT_KEYS, |_| Submit::<Msg>::Close).field(Field::path(
        "place",
        "place",
        PathField::new("~/pro", PathKind::Dir, source.clone()),
    ));
    let asked = source.calls();
    let _ = drawn(&form, 80, 24);
    let _ = drawn(&form, 40, 10);
    assert_eq!(source.calls(), asked);
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/form/goldens")
        .join(name)
}

#[test]
fn the_form_looks_as_its_golden_says() {
    let mut form = form();
    go(&mut form, "copy");
    key(&mut form, press(KeyCode::Char(' ')));
    go(&mut form, "name");
    key(&mut form, press(KeyCode::Enter));
    let buf = drawn(&form, 64, 20);
    assert_golden(&golden_path("form.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
    let narrow = drawn(&form, 40, 10);
    assert_golden(&golden_path("narrow.golden"), &narrow, |path, text| {
        std::fs::write(path, text)
    });
}

// ---- the conventions ---------------------------------------------------------

/// A host of one stack of forms over a screen of text.
struct Host(Theme);

impl Rig for Host {
    type State = ModalStack<Form<Msg>>;

    fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        Paragraph::new("the screen under the form").render(area, &mut buf);
        stack.draw::<Msg, ()>(area, &mut buf, &(), &self.0);
        buf
    }

    fn close_top(&self, stack: &mut Self::State) {
        stack.pop();
    }

    fn press(&self, stack: &mut Self::State, key: KeyEvent) {
        if let Some(outcome) = stack.route::<Msg, ()>(key, &()) {
            let _ = stack.settle(outcome);
        }
    }

    fn depth(&self, stack: &Self::State) -> usize {
        stack.len()
    }

    fn theme(&self) -> &Theme {
        &self.0
    }
}

#[test]
fn a_form_keeps_the_conventions_every_modal_keeps() {
    let open = || {
        let mut stack = ModalStack::new();
        stack.push(form());
        stack
    };
    let registry = Registry::new(Host(Theme::default())).case(
        Case::new("Form", open)
            .typed()
            .moves()
            .keys(
                Convention::new("Form", EnterMeans::Submits, SpaceMeans::Toggles, ""),
                [conformance::table("KEYS", KEYS)],
            )
            .footer(KEYS),
    );
    conformance::assert_conventions(&registry);
}
