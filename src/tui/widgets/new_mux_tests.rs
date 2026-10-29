use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::form::Form;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::testkit::keys::{ctrl, press};

use super::*;

fn key(form: &mut Form<Msg>, event: ratatui::crossterm::event::KeyEvent) -> Outcome<Msg> {
    form.handle(event, &())
}

fn name(form: &Form<Msg>) -> &str {
    form.values().text(NAME)
}

fn cwd(form: &Form<Msg>) -> &str {
    form.values().text(CWD)
}

fn commit(name: &str, cwd: &str) -> Outcome<Msg> {
    Outcome::Commit(Msg::CommitMuxNew {
        name: name.into(),
        cwd: cwd.into(),
    })
}

#[test]
fn form_starts_focused_on_name_field() {
    let form = form("dev", "/home/op/proj", Vec::new());
    assert_eq!(form.focus(), Some(NAME));
    assert_eq!(name(&form), "dev");
    assert_eq!(cwd(&form), "/home/op/proj");
    assert_eq!(form.title(), "New tmux session");
}

#[test]
fn tab_cycles_focus_between_fields() {
    let mut form = form("dev", "/home/op/proj", Vec::new());
    assert_eq!(key(&mut form, press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(form.focus(), Some(CWD));
    assert_eq!(key(&mut form, press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(form.focus(), Some(NAME));
}

#[test]
fn enter_on_the_name_field_creates_the_session() {
    let mut form = form("dev", "/home/op/proj", Vec::new());
    assert_eq!(
        key(&mut form, press(KeyCode::Enter)),
        commit("dev", "/home/op/proj"),
        "Enter submits from any field"
    );
}

#[test]
fn enter_on_cwd_commits_when_both_fields_populated() {
    let mut form = form("dev", "/home/op/proj", Vec::new());
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(
        key(&mut form, press(KeyCode::Enter)),
        commit("dev", "/home/op/proj")
    );
}

#[test]
fn enter_on_empty_name_snaps_focus_back_to_name() {
    let mut form = form("", "/tmp", Vec::new());
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(key(&mut form, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(form.error(), Some("mux: name is required"));
    assert_eq!(form.focus(), Some(NAME), "the field to fill in");
}

#[test]
fn enter_on_empty_cwd_stays_on_cwd() {
    let mut form = form("dev", "", Vec::new());
    assert_eq!(key(&mut form, press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(form.error(), Some("mux: cwd is required"));
    assert_eq!(form.focus(), Some(CWD));
}

#[test]
fn the_next_edit_clears_the_error() {
    let mut form = form("", "/tmp", Vec::new());
    key(&mut form, press(KeyCode::Enter));
    key(&mut form, press(KeyCode::Down));
    assert_eq!(
        form.error(),
        Some("mux: name is required"),
        "moving keeps it"
    );
    key(&mut form, press(KeyCode::Up));
    key(&mut form, press(KeyCode::Char('d')));
    assert_eq!(form.error(), None);
}

#[test]
fn the_cwd_is_committed_with_the_home_expanded() {
    let home = std::env::var("HOME").expect("a home directory");
    let mut form = form("dev", "~/projects", Vec::new());
    assert_eq!(
        key(&mut form, press(KeyCode::Enter)),
        commit("dev", &format!("{home}/projects"))
    );
}

#[test]
fn esc_closes_the_form() {
    let mut form = form("dev", "/tmp", Vec::new());
    assert_eq!(key(&mut form, press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn ctrl_c_closes_the_form() {
    let mut form = form("dev", "/tmp", Vec::new());
    assert_eq!(key(&mut form, ctrl('c')), Outcome::Close);
}

#[test]
fn typing_edits_the_focused_field_only() {
    let mut form = form("", "", Vec::new());
    key(&mut form, press(KeyCode::Char('a')));
    key(&mut form, press(KeyCode::Char('b')));
    assert_eq!(name(&form), "ab");
    assert_eq!(cwd(&form), "");
    key(&mut form, press(KeyCode::Tab));
    key(&mut form, press(KeyCode::Char('/')));
    key(&mut form, press(KeyCode::Char('t')));
    assert_eq!(name(&form), "ab");
    assert_eq!(cwd(&form), "/t");
}

#[test]
fn up_and_down_move_between_name_and_cwd() {
    let mut form = form("dev", "/home/op/proj", Vec::new());
    key(&mut form, press(KeyCode::Down));
    assert_eq!(form.focus(), Some(CWD));
    key(&mut form, press(KeyCode::Up));
    assert_eq!(form.focus(), Some(NAME));
}

#[test]
fn tab_completes_a_directory_in_the_cwd_field() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("alpha-dir")).unwrap();
    let mut form = form("dev", format!("{}/alp", dir.path().display()), Vec::new());
    key(&mut form, press(KeyCode::Down));
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(form.focus(), Some(CWD), "completed, so it stayed");
    assert_eq!(cwd(&form), format!("{}/alpha-dir", dir.path().display()));
}

#[test]
fn candidates_offer_known_directories() {
    let mut form = form(
        "dev",
        "/srv/wor",
        vec![tuikit::path_field::Candidate::new("/srv/work/known", 300)],
    );
    key(&mut form, press(KeyCode::Down));
    key(&mut form, press(KeyCode::Tab));
    assert_eq!(cwd(&form), "/srv/work/known");
}

#[test]
fn typing_in_the_cwd_field_edits_the_path() {
    let mut form = form("dev", "/a", Vec::new());
    key(&mut form, press(KeyCode::Down));
    key(&mut form, press(KeyCode::Char('b')));
    assert_eq!(cwd(&form), "/ab");
}

#[test]
fn every_field_takes_a_paste() {
    let mut form = form("dev", "/a", Vec::new());
    assert!(Overlay::<Msg, ()>::takes_text(&form));
    key(&mut form, press(KeyCode::Down));
    assert!(Overlay::<Msg, ()>::takes_text(&form));
}

fn screen(form: &Form<Msg>, width: u16, height: u16) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    form.draw(buf.area, &mut buf, &crate::tui::Theme::default());
    tuikit::testkit::buffer::rows(&buf).join("\n")
}

#[test]
fn the_form_shows_the_path_mark_and_its_keys() {
    let frame = screen(&form("dev", "/tmp", Vec::new()), 90, 14);
    assert!(
        frame.contains('√') && frame.contains("↑/↓ move") && frame.contains("Enter create"),
        "{frame}"
    );
}

#[test]
fn a_refusal_and_the_keys_stay_in_view_on_a_short_screen() {
    let mut form = form("", "/tmp", Vec::new());
    key(&mut form, press(KeyCode::Enter));
    let frame = screen(&form, 40, 10);
    for needle in [
        "name",
        "mux: name is required",
        "Enter create",
        "Esc cancel",
    ] {
        assert!(frame.contains(needle), "{needle} missing:\n{frame}");
    }
}
