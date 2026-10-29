use std::path::{Path, PathBuf};
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;

use crate::form::{
    Choice, Field, Form, KEPT, OverrideRow, SOURCE_GAP, Standing, Submit, Toggle, Values,
    WILL_FOLLOW, WILL_KEEP,
};
use crate::glyphs::marks;
use crate::keys::{FORM_KEYS, FORM_OPTION_KEYS, FormKey, KeyRow, ctrl, include, key, shown};
use crate::overlay::{Outcome, Overlay};
use crate::path_field::{PathField, PathKind};
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{ctrl as ctrl_press, press};
use crate::testkit::paths::MemoryPaths;
use crate::theme::Theme;

/// What a submit of the settings form changes: values kept as the
/// fields' own, and fields that follow their inherited value again.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Save {
        keep: Vec<(String, String)>,
        drop: Vec<String>,
    },
}

/// The settings form's keys: the standard rows, a reset key of its own,
/// `Enter save`, and `Esc cancel`.
const KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(ctrl('r'), FormKey::Inherit, "Ctrl-R", "reset"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

/// The same form's keys without a reset key.
const NO_RESET_KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

/// The fields that follow a value, in order.
const FOLLOWERS: [&str; 4] = ["color", "size", "place", "tool"];

fn paths() -> Arc<MemoryPaths> {
    Arc::new(
        MemoryPaths::new()
            .with_home("/home/me")
            .dir("/home/me/projects/alpha")
            .dir("/srv"),
    )
}

/// What a submit means: each changed field is kept, each reset one
/// dropped, and a submit that changes nothing closes.
fn save(values: &Values<'_>) -> Submit<Change> {
    let mut keep = Vec::new();
    let mut drop = Vec::new();
    for id in FOLLOWERS {
        match values.standing(id) {
            Some(Standing::Changed) => keep.push((id.to_string(), values.text(id).to_string())),
            Some(Standing::Reset) => drop.push(id.to_string()),
            _ => {}
        }
    }
    if keep.is_empty() && drop.is_empty() {
        Submit::Close
    } else {
        Submit::Commit(Change::Save { keep, drop })
    }
}

/// What the color row's hint says, by where its value stands.
fn color_hint(view: &crate::form::OverrideView<'_>) -> Vec<String> {
    let line = match view.standing {
        Standing::Follows => format!("follows the {}: {}", view.source, view.value),
        Standing::Kept => format!("its own: {}", view.value),
        Standing::Changed => format!("will keep: {}", view.value),
        Standing::Reset => format!("will follow the {} again", view.source),
    };
    vec![line]
}

/// A settings form:
///
/// - `title` (text) has no inherited value;
/// - `color` (text) follows the default `blue`, with a hint;
/// - `size` (text) holds its own `large` over the theme's `medium`;
/// - `place` (a directory) follows the default `~/projects/alpha`;
/// - `tool` (a choice) follows the default `saw`;
/// - `lock` (a toggle) is decorated but holds no text, so it does not
///   follow anything.
fn settings_with(keys: &'static [KeyRow<FormKey>]) -> Form<Change> {
    Form::new("Settings", keys, save)
        .intro("A field follows its default until you change it.")
        .details(2)
        .field(Field::text("title", "title", "draft").hint("what the thing is called"))
        .field(
            Field::text("color", "color", "blue")
                .with_override(OverrideRow::new("blue", "default").hint(color_hint))
                .hint("type a color"),
        )
        .field(
            Field::text("size", "size", "large")
                .with_override(OverrideRow::new("medium", "theme").overridden(true))
                .hint("the theme's size is medium"),
        )
        .field(
            Field::path(
                "place",
                "place",
                PathField::new("~/projects/alpha", PathKind::Dir, paths()),
            )
            .with_override(OverrideRow::new("~/projects/alpha", "default")),
        )
        .field(
            Field::choice(
                "tool",
                "tool",
                Choice::new("saw", vec!["saw".to_string(), "drill".to_string()]).custom(true),
            )
            .with_override(OverrideRow::new("saw", "default")),
        )
        .field(
            Field::toggle("lock", "lock", Toggle::new(false, "locked"))
                .with_override(OverrideRow::new("on", "default")),
        )
}

fn settings() -> Form<Change> {
    settings_with(KEYS)
}

fn send(form: &mut Form<Change>, event: KeyEvent) -> Outcome<Change> {
    form.handle(event, &())
}

fn type_text(form: &mut Form<Change>, text: &str) {
    for c in text.chars() {
        send(form, press(KeyCode::Char(c)));
    }
}

fn go(form: &mut Form<Change>, id: &str) {
    assert!(form.focus_on(id), "no stop {id}");
}

fn word(form: &Form<Change>, id: &str) -> String {
    let field = form.get(id).expect("the field");
    let row = field.override_row().expect("an override row");
    row.word(field.text_value().unwrap_or("")).to_string()
}

fn drawn(form: &Form<Change>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    form.draw(buf.area, &mut buf, &Theme::default());
    buf
}

fn screen(form: &Form<Change>, width: u16, height: u16) -> String {
    rows(&drawn(form, width, height)).join("\n")
}

// ---- where a value stands --------------------------------------------------

#[test]
fn a_row_follows_its_inherited_value_and_names_where_it_comes_from() {
    let form = settings();
    assert_eq!(form.values().standing("color"), Some(Standing::Follows));
    assert_eq!(word(&form, "color"), "default");
    assert_eq!(form.values().standing("size"), Some(Standing::Kept));
    assert_eq!(word(&form, "size"), KEPT);
    assert_eq!(
        form.values().standing("title"),
        None,
        "a plain field stands nowhere"
    );
}

#[test]
fn editing_a_row_makes_the_value_its_own() {
    let mut form = settings();
    go(&mut form, "color");
    type_text(&mut form, "ish");
    assert_eq!(form.values().standing("color"), Some(Standing::Changed));
    assert_eq!(word(&form, "color"), WILL_KEEP);
    assert_eq!(
        send(&mut form, press(KeyCode::Enter)),
        Outcome::Commit(Change::Save {
            keep: vec![("color".into(), "blueish".into())],
            drop: vec![],
        })
    );
}

#[test]
fn the_reset_key_puts_the_inherited_value_back() {
    let mut form = settings();
    go(&mut form, "size");
    assert_eq!(send(&mut form, ctrl_press('r')), Outcome::Consumed);
    assert_eq!(form.values().text("size"), "medium");
    assert_eq!(form.values().standing("size"), Some(Standing::Reset));
    assert_eq!(word(&form, "size"), WILL_FOLLOW);
    assert_eq!(form.edited(), Some("size"), "a reset is an edit");
    assert_eq!(
        send(&mut form, press(KeyCode::Enter)),
        Outcome::Commit(Change::Save {
            keep: vec![],
            drop: vec!["size".into()],
        })
    );
}

#[test]
fn typing_after_a_reset_makes_the_value_its_own_again() {
    let mut form = settings();
    go(&mut form, "size");
    send(&mut form, ctrl_press('r'));
    type_text(&mut form, "!");
    assert_eq!(form.values().text("size"), "medium!");
    assert_eq!(form.values().standing("size"), Some(Standing::Changed));
}

#[test]
fn resetting_a_row_with_no_value_of_its_own_just_follows_again() {
    let mut form = settings();
    go(&mut form, "color");
    type_text(&mut form, "ish");
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.values().text("color"), "blue");
    assert_eq!(
        form.values().standing("color"),
        Some(Standing::Follows),
        "there was nothing of its own to drop"
    );
    assert_eq!(word(&form, "color"), "default");
    assert_eq!(send(&mut form, press(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn a_reset_clears_the_error_line_as_an_edit_does_unless_it_changes_nothing() {
    let mut form = settings().field(Field::text("count", "count", "").validate(|text| {
        if text.is_empty() {
            Err("a count is required".into())
        } else {
            Ok(())
        }
    }));
    send(&mut form, press(KeyCode::Enter));
    assert_eq!(form.error(), Some("a count is required"));

    // The color already follows its value: resetting it changes nothing.
    go(&mut form, "color");
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.edited(), None);
    assert_eq!(form.error(), Some("a count is required"));

    go(&mut form, "size");
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.edited(), Some("size"));
    assert_eq!(form.error(), None);
}

#[test]
fn a_path_and_a_choice_follow_a_value_too() {
    let mut form = settings();
    go(&mut form, "place");
    type_text(&mut form, "/x");
    assert_eq!(form.values().standing("place"), Some(Standing::Changed));
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.values().text("place"), "~/projects/alpha");
    assert_eq!(form.values().standing("place"), Some(Standing::Follows));

    go(&mut form, "tool");
    send(&mut form, press(KeyCode::Right));
    assert_eq!(form.values().text("tool"), "drill");
    assert_eq!(form.values().standing("tool"), Some(Standing::Changed));
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.values().text("tool"), "saw");
    assert_eq!(form.values().standing("tool"), Some(Standing::Follows));
}

#[test]
fn a_field_without_text_takes_no_override() {
    let form = settings();
    let lock = form.get("lock").expect("the toggle");
    assert!(lock.override_row().is_none());
    assert_eq!(lock.standing(), None);
}

// ---- the reset key is the form's own ---------------------------------------

#[test]
fn the_reset_key_does_nothing_on_a_field_that_follows_nothing() {
    let mut form = settings();
    type_text(&mut form, "s");
    assert_eq!(form.focus(), Some("title"));
    assert_eq!(send(&mut form, ctrl_press('r')), Outcome::Consumed);
    assert_eq!(form.values().text("title"), "drafts");
    assert_eq!(form.edited(), None);
}

#[test]
fn a_form_whose_table_binds_no_reset_key_never_resets() {
    let mut form = settings_with(NO_RESET_KEYS);
    go(&mut form, "size");
    send(&mut form, ctrl_press('r'));
    assert_eq!(form.values().text("size"), "large");
    assert_eq!(form.values().standing("size"), Some(Standing::Kept));
    assert!(!screen(&form, 80, 30).contains("Ctrl-R"));
}

// ---- drawing ---------------------------------------------------------------

/// The row whose label is `label`, focused or not.
fn row_of<'a>(lines: &'a [String], label: &str) -> &'a str {
    lines
        .iter()
        .find(|line| {
            line.contains(&format!("│  {label} "))
                || line.contains(&format!("│{} {label} ", marks::POINTER))
        })
        .unwrap_or_else(|| panic!("no {label} row:\n{}", lines.join("\n")))
}

#[test]
fn every_row_ends_in_the_source_column() {
    let mut form = settings();
    go(&mut form, "color");
    type_text(&mut form, "ish");
    let buf = drawn(&form, 80, 30);
    let lines = rows(&buf);
    let end = |label: &str| {
        row_of(&lines, label)
            .trim_end()
            .trim_end_matches('│')
            .trim_end()
            .chars()
            .count()
    };
    let column = end("size");
    for label in ["color", "place", "tool"] {
        assert_eq!(end(label), column, "{label} ends where size ends");
    }
    let color = find(&buf, WILL_KEEP).expect("the pending word");
    let kept = find(&buf, KEPT).expect("the kept word");
    assert_eq!(
        color.x + 9,
        kept.x + 4,
        "the words are right-aligned in one column"
    );
    let theme = Theme::default();
    assert_eq!(
        style_at(&buf, WILL_KEEP).and_then(|style| style.fg),
        Some(theme.warning),
        "a pending change is marked"
    );
    assert!(
        style_at(&buf, KEPT).is_some_and(|style| style.add_modifier.contains(theme.muted)),
        "a settled word recedes"
    );
}

#[test]
fn a_plain_row_ends_its_value_where_the_others_do_and_leaves_the_column_blank() {
    let mut form = settings();
    type_text(&mut form, &"x".repeat(80));
    send(&mut form, press(KeyCode::Home));
    let lines = rows(&drawn(&form, 80, 30));
    let title = row_of(&lines, "title");
    let after: String = title.chars().skip_while(|c| *c != '>').skip(1).collect();
    assert_eq!(
        after.trim_end(),
        format!("{}│", " ".repeat(SOURCE_GAP + WILL_FOLLOW.len())),
        "the value's scroll mark is the last cell before the blank column: {title}"
    );
}

#[test]
fn the_detail_lines_explain_the_focused_field_and_keep_their_height() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 80, 30);
    let mut form = settings();
    let at_title = form.rect(area, &theme);
    assert!(screen(&form, 80, 30).contains("what the thing is called"));
    go(&mut form, "color");
    let text = screen(&form, 80, 30);
    assert!(text.contains("follows the default: blue"), "{text}");
    assert!(text.contains("type a color"), "its own hint comes after");
    go(&mut form, "size");
    let text = screen(&form, 80, 30);
    assert!(!text.contains("follows the default"), "{text}");
    assert_eq!(
        form.rect(area, &theme),
        at_title,
        "the band keeps its height"
    );
    go(&mut form, "color");
    type_text(&mut form, "ish");
    assert!(screen(&form, 80, 30).contains("will keep: blueish"));
}

#[test]
fn the_intro_sits_above_the_rows_and_stays_while_they_scroll() {
    let mut form = settings();
    go(&mut form, "tool");
    let text = screen(&form, 80, 14);
    let intro = text
        .find("A field follows its default")
        .expect("the intro shows");
    let tool = text.find(" tool ").expect("the focused row shows");
    assert!(intro < tool, "{text}");
    assert!(!text.contains(" title "), "the first rows scrolled: {text}");
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/form/goldens")
        .join(name)
}

#[test]
fn an_override_form_looks_as_its_golden_says() {
    let mut form = settings();
    go(&mut form, "color");
    type_text(&mut form, "ish");
    go(&mut form, "size");
    send(&mut form, ctrl_press('r'));
    let buf = drawn(&form, 64, 24);
    assert_golden(&golden_path("override.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
}
