use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use super::*;
use crate::testkit::buffer::{find, rows, style_at};
use crate::testkit::golden::assert_golden;
use crate::testkit::keys::{chord, ctrl, press};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Named(String),
}

/// A prompt that takes any non-blank name and refuses a blank one.
fn naming(initial: &str) -> TextPrompt<Msg> {
    TextPrompt::new("Name", initial, |text: &str| match text.trim() {
        "" => PromptResult::Reject("a name is required".into()),
        name => PromptResult::Commit(Msg::Named(name.to_string())),
    })
}

fn key(prompt: &mut TextPrompt<Msg>, event: KeyEvent) -> Outcome<Msg> {
    prompt.handle(event, &())
}

fn type_text(prompt: &mut TextPrompt<Msg>, text: &str) {
    for c in text.chars() {
        key(prompt, press(KeyCode::Char(c)));
    }
}

/// The prompt drawn alone on a `width` by `height` screen.
fn drawn(prompt: &TextPrompt<Msg>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    prompt.draw(buf.area, &mut buf, &Theme::default());
    buf
}

/// The prompt's rows on screen, border included, trimmed of the space
/// around the frame.
fn framed(prompt: &TextPrompt<Msg>, width: u16, height: u16) -> Vec<String> {
    let buf = drawn(prompt, width, height);
    let rect = prompt.rect(buf.area);
    rows(&buf)[usize::from(rect.y)..usize::from(rect.bottom())]
        .iter()
        .map(|row| {
            row.chars()
                .skip(usize::from(rect.x))
                .take(usize::from(rect.width))
                .collect()
        })
        .collect()
}

#[test]
fn enter_hands_the_text_as_typed_to_on_confirm() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let mut prompt = TextPrompt::new("Ask", "  spaced  ", move |text: &str| {
        log.lock().expect("the log").push(text.to_string());
        PromptResult::<Msg>::Stay
    });
    key(&mut prompt, press(KeyCode::Enter));
    assert_eq!(*seen.lock().expect("the log"), ["  spaced  "], "untrimmed");
}

#[test]
fn a_commit_closes_the_prompt_with_the_hosts_message() {
    let mut prompt = naming("old");
    type_text(&mut prompt, "er");
    assert_eq!(
        key(&mut prompt, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Named("older".into()))
    );
}

#[test]
fn a_step_back_closes_the_prompt_one_level_with_the_hosts_message() {
    let mut prompt = TextPrompt::new("Ask", "7d", |text: &str| {
        PromptResult::Back(Msg::Named(text.to_string()))
    });
    assert_eq!(
        key(&mut prompt, press(KeyCode::Enter)),
        Outcome::Back(Msg::Named("7d".into()))
    );
}

#[test]
fn a_rejection_stays_open_and_says_why_until_the_next_edit() {
    let mut prompt = naming("");
    assert_eq!(key(&mut prompt, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(prompt.error(), Some("a name is required"));

    // Moving the cursor or a swallowed Tab is not an edit.
    key(&mut prompt, press(KeyCode::Left));
    key(&mut prompt, press(KeyCode::Tab));
    key(&mut prompt, press(KeyCode::Backspace));
    assert_eq!(
        prompt.error(),
        Some("a name is required"),
        "nothing changed"
    );

    key(&mut prompt, press(KeyCode::Char('x')));
    assert_eq!(prompt.error(), None, "typing clears the reason");
}

#[test]
fn deleting_text_is_an_edit_that_clears_the_reason_too() {
    let mut prompt = TextPrompt::new("Ask", "abc", |_: &str| {
        PromptResult::<Msg>::Reject("no".into())
    });
    key(&mut prompt, press(KeyCode::Enter));
    assert_eq!(prompt.error(), Some("no"));
    key(&mut prompt, ctrl('u'));
    assert_eq!(prompt.error(), None);
    assert_eq!(prompt.value(), "");
}

#[test]
fn stay_keeps_the_prompt_as_it_is() {
    let refuse = Arc::new(Mutex::new(true));
    let flag = Arc::clone(&refuse);
    let mut prompt = TextPrompt::new("Ask", "text", move |_: &str| {
        if *flag.lock().expect("the flag") {
            PromptResult::<Msg>::Reject("not yet".into())
        } else {
            PromptResult::Stay
        }
    });
    key(&mut prompt, press(KeyCode::Enter));
    *refuse.lock().expect("the flag") = false;
    assert_eq!(key(&mut prompt, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(prompt.value(), "text");
    assert_eq!(prompt.error(), Some("not yet"), "a stay leaves the reason");
}

#[test]
fn esc_and_ctrl_c_close_and_q_types() {
    // Esc steps back; Ctrl-C dismisses the prompt and any flow it is a
    // step of.
    for (close, outcome) in [
        (press(KeyCode::Esc), Outcome::Close),
        (ctrl('c'), Outcome::Dismiss),
    ] {
        let mut prompt = naming("kept");
        assert_eq!(key(&mut prompt, close), outcome, "{close:?}");
        assert_eq!(prompt.value(), "kept", "closing changes nothing");
    }
    let mut prompt = naming("");
    assert_eq!(
        key(&mut prompt, press(KeyCode::Char('q'))),
        Outcome::Consumed
    );
    assert_eq!(prompt.value(), "q", "q is text in a prompt");
}

#[test]
fn tab_is_swallowed_and_every_other_key_is_the_prompts() {
    let mut prompt = naming("abc");
    assert_eq!(key(&mut prompt, press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(key(&mut prompt, press(KeyCode::F(5))), Outcome::Consumed);
    assert_eq!(key(&mut prompt, press(KeyCode::Down)), Outcome::Consumed);
    assert_eq!(prompt.value(), "abc");
}

#[test]
fn the_editing_keys_edit_the_line() {
    let mut prompt = naming("alpha beta");
    key(&mut prompt, chord(KeyModifiers::ALT, KeyCode::Char('b')));
    assert_eq!(prompt.edit().cursor(), 6, "Alt-b goes back a word");
    key(&mut prompt, ctrl('k'));
    assert_eq!(prompt.value(), "alpha ");
}

#[test]
fn the_prompt_takes_text_and_does_not_fill_the_screen() {
    let prompt = naming("");
    assert!(Overlay::<Msg, ()>::takes_text(&prompt));
    assert!(!Overlay::<Msg, ()>::fills_screen(&prompt));
}

#[test]
fn a_prompt_is_three_rows_with_its_title_field_and_generated_footer() {
    let prompt = naming("draft");
    assert_eq!(
        framed(&prompt, 40, 9),
        [
            "┌ Name ────────────────────────────┐",
            "│draft                             │",
            "└ Enter confirm · Esc cancel ──────┘",
        ]
    );
    assert_eq!(prompt.rect(Rect::new(0, 0, 40, 9)), Rect::new(2, 3, 36, 3));
}

#[test]
fn an_error_line_grows_the_prompt_by_one_row_while_it_shows() {
    let mut prompt = naming("");
    key(&mut prompt, press(KeyCode::Enter));
    assert_eq!(
        framed(&prompt, 40, 9),
        [
            "┌ Name ────────────────────────────┐",
            "│                                  │",
            "│a name is required                │",
            "└ Enter confirm · Esc cancel ──────┘",
        ]
    );
    let buf = drawn(&prompt, 40, 9);
    let style = style_at(&buf, "a name is required").expect("the error line");
    assert_eq!(style.fg, Some(Theme::default().error));

    key(&mut prompt, press(KeyCode::Char('n')));
    assert_eq!(framed(&prompt, 40, 9).len(), 3, "an edit shrinks it back");
}

#[test]
fn a_long_reason_is_cut_to_one_line() {
    let mut prompt = TextPrompt::new("Ask", "", |_: &str| {
        PromptResult::<Msg>::Reject("a reason far longer than the prompt is wide".into())
    });
    key(&mut prompt, press(KeyCode::Enter));
    let rows = framed(&prompt, 24, 9);
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[2], "│a reason far long…│");
}

#[test]
fn a_narrow_prompt_keeps_the_hints_that_fit_whole_and_the_keys_that_leave() {
    let prompt = naming("");
    let rows = framed(&prompt, 22, 9);
    assert_eq!(
        rows[2], "└ Esc cancel ──────┘",
        "the footer breaks between hints and keeps its last row"
    );
}

#[test]
fn a_screen_too_short_for_the_error_line_shows_the_reason_on_the_border() {
    let mut prompt = naming("");
    key(&mut prompt, press(KeyCode::Enter));
    let rows = framed(&prompt, 40, 5);
    assert_eq!(
        rows,
        [
            "┌ Name ────────────────────────────┐",
            "│                                  │",
            "└ a name is required ──────────────┘",
        ]
    );
    let buf = drawn(&prompt, 40, 5);
    let style = style_at(&buf, "a name is required").expect("the reason");
    assert_eq!(style.fg, Some(Theme::default().error));
}

#[test]
fn a_placeholder_shows_in_inert_while_the_field_is_empty() {
    let theme = Theme::default();
    let mut prompt = naming("").placeholder("2h, 30m, or 7d");
    let buf = drawn(&prompt, 40, 9);
    let at = find(&buf, "h, 30m").expect("the placeholder");
    assert_eq!(buf[(at.x - 1, at.y)].symbol(), "2");
    assert_eq!(
        buf[(at.x - 1, at.y)].style(),
        buf[(at.x - 1, at.y)]
            .style()
            .patch(theme.field_cursor.into_style()),
        "the caret sits on its first cell"
    );
    assert_eq!(buf[(at.x, at.y)].fg, theme.inert);

    type_text(&mut prompt, "5");
    let buf = drawn(&prompt, 40, 9);
    assert!(find(&buf, "30m").is_none(), "typing replaces it");
}

#[test]
fn a_long_value_keeps_the_cursor_in_view() {
    let value: String = (b'a'..=b'z').map(char::from).collect();
    let prompt = naming(&value);
    let rows = framed(&prompt, 20, 9);
    assert_eq!(rows[1], "│jklmnopqrstuvwxyz │", "the caret after the text");
}

#[test]
fn wide_characters_scroll_by_their_cells() {
    let prompt = naming("東京タワーと富士山の写真です");
    let buf = drawn(&prompt, 20, 9);
    let row = rows(&buf)[usize::from(prompt.rect(buf.area).y) + 1].replace(' ', "");
    // Fourteen two-cell characters; the 16-cell field shows the last
    // seven and the caret after them.
    assert!(row.contains("山の写真です"), "{row:?}");
    assert!(!row.contains("と"), "{row:?}");
}

#[test]
fn a_clone_shares_the_hosts_decision() {
    let mut prompt = naming("");
    let mut copy = prompt.clone();
    type_text(&mut copy, "b");
    assert_eq!(
        key(&mut copy, press(KeyCode::Enter)),
        Outcome::Commit(Msg::Named("b".into()))
    );
    assert_eq!(prompt.value(), "", "the copy edits its own line");
    key(&mut prompt, press(KeyCode::Enter));
    assert_eq!(prompt.error(), Some("a name is required"));
}

#[test]
fn debug_names_the_state_and_not_the_closure() {
    let shown = format!("{:?}", naming("x"));
    assert!(shown.starts_with("TextPrompt {"), "{shown}");
    assert!(shown.contains("value: \"x\""), "{shown}");
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/text_prompt/goldens")
        .join(name)
}

#[test]
fn the_prompt_looks_as_its_golden_says() {
    let theme = Theme::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 34, 11));
    let typed = naming("draft");
    typed.draw(Rect::new(0, 0, 34, 5), &mut buf, &theme);
    let mut refused = naming("").placeholder("name");
    key(&mut refused, press(KeyCode::Enter));
    refused.draw(Rect::new(0, 5, 34, 6), &mut buf, &theme);
    assert_golden(&golden_path("prompt.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
}
