use std::path::{Path, PathBuf};
use std::sync::Arc;

use ratatui::crossterm::event::{KeyCode, KeyEventKind};

use super::*;
use crate::path_field::collapse_home;
use crate::testkit::keys::{ctrl, press, release};
use crate::testkit::paths::{Calls, MemoryPaths};

/// A home with two project directories, a file beside them, and a
/// hidden directory; a second user's home next to it.
fn tree() -> Arc<MemoryPaths> {
    Arc::new(
        MemoryPaths::new()
            .with_home("/home/me")
            .dir("/home/me/projects/alpha")
            .dir("/home/me/projects/beta")
            .file("/home/me/projects/notes.txt")
            .dir("/home/me/.config")
            .dir("/home/meadow")
            .dir("/srv/web"),
    )
}

fn field(value: &str, kind: PathKind) -> (PathField, Arc<MemoryPaths>) {
    let paths = tree();
    (PathField::new(value, kind, paths.clone()), paths)
}

fn values(field: &PathField) -> Vec<&str> {
    field.candidates().iter().map(Completion::value).collect()
}

fn type_text(field: &mut PathField, text: &str) {
    for c in text.chars() {
        field.handle(press(KeyCode::Char(c)));
    }
}

// ---- ranking -------------------------------------------------------------

#[test]
fn known_candidates_rank_by_priority_before_what_the_source_lists() {
    let (field, _) = field("/home/me/projects/", PathKind::Dir);
    let field = field.with_candidates(vec![
        Candidate::new("/home/me/projects/zeta", 10),
        Candidate::new("/home/me/projects/gamma", 300),
        Candidate::new("/home/me/projects/delta", 300),
    ]);
    assert_eq!(
        values(&field),
        [
            "/home/me/projects/delta",
            "/home/me/projects/gamma",
            "/home/me/projects/zeta",
            "/home/me/projects/alpha",
            "/home/me/projects/beta",
        ],
        "equal priorities go in path order; listed entries in name order"
    );
}

#[test]
fn a_path_the_host_knows_is_offered_once_at_its_best_priority() {
    let (field, _) = field("/home/me/projects/a", PathKind::Dir);
    let field = field.with_candidates(vec![
        Candidate::new("/home/me/projects/alpha", 5),
        Candidate::new("  /home/me/projects/alpha  ", 50),
        Candidate::new("   ", 900),
        Candidate::new("/home/me/projects/aardvark", 20),
    ]);
    assert_eq!(
        values(&field),
        ["/home/me/projects/alpha", "/home/me/projects/aardvark"],
        "a known path and the listed entry of the same name are one candidate"
    );
}

#[test]
fn a_candidate_only_ever_extends_the_typed_text() {
    let (field, _) = field("/home/me/projects/b", PathKind::Dir);
    let field = field.with_candidates(vec![
        Candidate::new("/srv/web/projects/b-side", 900),
        Candidate::new("/home/me/projects/b", 800),
    ]);
    assert_eq!(
        values(&field),
        ["/home/me/projects/beta"],
        "a path that only contains the text, or equals it, completes nothing"
    );
}

#[test]
fn with_nothing_typed_every_known_path_completes_and_nothing_is_listed() {
    let (field, paths) = field("", PathKind::Dir);
    let field = field.with_candidates(vec![
        Candidate::new("/srv/web", 1),
        Candidate::new("/home/me", 2),
    ]);
    assert_eq!(values(&field), ["/home/me", "/srv/web"]);
    assert_eq!(field.validity(), Validity::Unknown);
    assert_eq!(field.found(), None);
    assert_eq!(paths.calls().entries, 0, "an empty field lists nothing");
}

#[test]
fn a_bare_name_lists_nothing() {
    let (field, paths) = field("projects", PathKind::Dir);
    assert!(field.candidates().is_empty());
    assert_eq!(paths.calls().entries, 0);
}

// ---- completion ----------------------------------------------------------

#[test]
fn tab_adds_the_remainder_the_hint_names() {
    let (mut field, _) = field("/home/me/projects/al", PathKind::Dir);
    let best = field.completion().cloned().expect("a completion");
    assert_eq!(best.remainder(), "pha");
    let edit = field.handle(press(KeyCode::Tab));
    assert!(edit.changed && edit.consumed);
    assert_eq!(
        field.value(),
        format!("/home/me/projects/al{}", best.remainder())
    );
    assert_eq!(field.edit().cursor(), field.value().chars().count());
    assert_eq!(field.validity(), Validity::Exists);
}

#[test]
fn tab_with_nothing_to_complete_is_left_to_the_host() {
    let (mut field, _) = field("/home/me/projects/alpha", PathKind::Dir);
    assert!(field.completion().is_none());
    assert_eq!(field.handle(press(KeyCode::Tab)), Edit::IGNORED);
    assert!(!field.complete());
    assert_eq!(field.value(), "/home/me/projects/alpha");
}

#[test]
fn a_released_tab_completes_nothing() {
    let (mut field, _) = field("/home/me/projects/al", PathKind::Dir);
    let tab = release(press(KeyCode::Tab));
    assert_eq!(tab.kind, KeyEventKind::Release);
    assert_eq!(field.handle(tab), Edit::IGNORED);
    assert_eq!(field.value(), "/home/me/projects/al");
}

#[test]
fn repeated_tab_descends_once_a_separator_is_typed() {
    let (mut field, _) = field("/home/me/pro", PathKind::Dir);
    assert!(field.complete());
    assert_eq!(field.value(), "/home/me/projects");
    assert!(
        !field.complete(),
        "a completed directory has no extension of its own name"
    );
    type_text(&mut field, "/");
    assert!(field.complete());
    assert_eq!(field.value(), "/home/me/projects/alpha");
}

#[test]
fn surrounding_spaces_are_not_part_of_the_path() {
    let (mut field, _) = field("  /home/me/projects/be  ", PathKind::Dir);
    assert_eq!(field.completion().map(Completion::remainder), Some("ta"));
    assert!(field.complete());
    assert_eq!(field.value(), "/home/me/projects/beta");
    assert_eq!(field.expanded_value(), "/home/me/projects/beta");
}

#[test]
fn edits_go_to_the_line_and_say_what_they_did() {
    let (mut field, _) = field("/srv/we", PathKind::Dir);
    let typed = field.handle(press(KeyCode::Char('b')));
    assert!(typed.changed && typed.consumed);
    assert_eq!(field.validity(), Validity::Exists);
    let moved = field.handle(press(KeyCode::Left));
    assert!(moved.consumed && !moved.changed);
    assert_eq!(field.handle(press(KeyCode::Enter)), Edit::IGNORED);
    field.handle(ctrl('u'));
    assert_eq!(field.value(), "");
    assert_eq!(field.validity(), Validity::Unknown);
}

// ---- kind ----------------------------------------------------------------

#[test]
fn a_directory_field_lists_only_directories_and_a_file_is_advisory() {
    let (mut field, _) = field("/home/me/projects/", PathKind::Dir);
    assert_eq!(
        values(&field),
        ["/home/me/projects/alpha", "/home/me/projects/beta"]
    );
    field.set_value("/home/me/projects/notes.txt");
    assert_eq!(field.validity(), Validity::Advisory);
    assert_eq!(field.found(), Some(PathKind::File));
}

#[test]
fn a_file_field_lists_files_and_the_directories_that_lead_to_them() {
    let (mut field, _) = field("/home/me/projects/", PathKind::File);
    assert_eq!(
        values(&field),
        [
            "/home/me/projects/alpha",
            "/home/me/projects/beta",
            "/home/me/projects/notes.txt",
        ]
    );
    field.set_value("/home/me/projects/notes.txt");
    assert_eq!(field.validity(), Validity::Exists);
    field.set_value("/home/me/projects");
    assert_eq!(field.validity(), Validity::Advisory);
    assert_eq!(field.found(), Some(PathKind::Dir));
}

#[test]
fn a_missing_path_is_advisory_and_still_the_value() {
    let (field, _) = field("/definitely/not/here", PathKind::Dir);
    assert_eq!(field.validity(), Validity::Advisory);
    assert_eq!(field.found(), None);
    assert_eq!(field.value(), "/definitely/not/here");
    assert_eq!(field.expanded_value(), "/definitely/not/here");
}

#[test]
fn hidden_entries_are_listed_like_any_other() {
    let (field, _) = field("/home/me/.c", PathKind::Dir);
    assert_eq!(values(&field), ["/home/me/.config"]);
}

// ---- tilde ---------------------------------------------------------------

#[test]
fn tilde_text_checks_and_commits_the_home_path_and_completes_in_tilde_form() {
    let (mut field, _) = field("~/pro", PathKind::Dir);
    assert_eq!(
        field.completion().map(Completion::value),
        Some("~/projects")
    );
    assert!(field.complete());
    assert_eq!(field.value(), "~/projects");
    assert_eq!(field.validity(), Validity::Exists);
    assert_eq!(field.expanded_value(), "/home/me/projects");
    field.set_value("~");
    assert_eq!(field.validity(), Validity::Exists);
    assert_eq!(field.expanded_value(), "/home/me");
}

#[test]
fn absolute_known_paths_complete_tilde_text_in_tilde_form() {
    let (field, _) = field("~/pro", PathKind::Dir);
    let field = field.with_candidates(vec![Candidate::new("/home/me/projects/alpha", 100)]);
    assert_eq!(values(&field), ["~/projects/alpha", "~/projects"]);
}

#[test]
fn tilde_candidates_complete_absolute_text() {
    let (field, _) = field("/home/me/p", PathKind::Dir);
    let field = field.with_candidates(vec![Candidate::new("~/plans", 100)]);
    assert_eq!(values(&field), ["/home/me/plans", "/home/me/projects"]);
}

#[test]
fn a_bare_tilde_completes_under_home_and_never_beside_it() {
    let (field, _) = field("~", PathKind::Dir);
    let field = field.with_candidates(vec![
        Candidate::new("/home/meadow", 200),
        Candidate::new("/home/me/projects", 100),
    ]);
    assert_eq!(
        values(&field),
        ["~/projects"],
        "`/home/meadow` cannot be written as `~` and more"
    );
}

#[test]
fn without_a_home_tilde_is_text() {
    let paths = Arc::new(MemoryPaths::new().dir("/home/me/projects"));
    let field = PathField::new("~/projects", PathKind::Dir, paths);
    assert_eq!(field.validity(), Validity::Advisory);
    assert_eq!(field.expanded_value(), "~/projects");
}

#[test]
fn another_users_home_is_not_expanded() {
    let (field, _) = field("~meadow", PathKind::Dir);
    assert_eq!(field.expanded_value(), "~meadow");
    assert_eq!(field.validity(), Validity::Advisory);
}

#[test]
fn the_tilde_rule_expands_and_collapses_only_the_home_prefix() {
    let home = Some(Path::new("/home/me"));
    assert_eq!(expand_home("~", home), PathBuf::from("/home/me"));
    assert_eq!(expand_home("~/", home), PathBuf::from("/home/me/"));
    assert_eq!(expand_home("~/a/b", home), PathBuf::from("/home/me/a/b"));
    assert_eq!(expand_home("/x/~/y", home), PathBuf::from("/x/~/y"));
    assert_eq!(expand_home("", home), PathBuf::from(""));
    assert_eq!(collapse_home("/home/me/", home), "~/");
    assert_eq!(
        collapse_home("/home/me/a", Some(Path::new("/home/me/"))),
        "~/a"
    );
    assert_eq!(collapse_home("/x", Some(Path::new("/"))), "/x");
    assert_eq!(collapse_home("/home/me/a", None), "/home/me/a");
}

// ---- caching -------------------------------------------------------------

#[test]
fn building_a_field_looks_once() {
    let (_, paths) = field("/home/me/projects/al", PathKind::Dir);
    assert_eq!(
        paths.calls(),
        Calls {
            entries: 1,
            kind: 1,
            home: 1
        }
    );
}

#[test]
fn reading_and_drawing_ask_the_source_nothing() {
    let (field, paths) = field("~/projects/a", PathKind::Dir);
    let before = paths.calls();
    let theme = Theme::default();
    for focused in [true, false] {
        for width in [0, 1, 2, 8, 20, 40, 80] {
            let _ = field
                .view()
                .focused(focused)
                .marks(true)
                .line(width, &theme);
        }
    }
    let _ = (
        field.validity(),
        field.found(),
        field.candidates().len(),
        field.completion(),
        field.expanded_value(),
        format!("{field:?}"),
        field.clone(),
    );
    assert_eq!(paths.calls(), before);
}

#[test]
fn typing_in_one_directory_lists_it_once() {
    let (mut field, paths) = field("/home/me/projects/", PathKind::Dir);
    let listed = paths.calls().entries;
    type_text(&mut field, "be");
    assert_eq!(paths.calls().entries, listed, "the listing is kept");
    assert_eq!(values(&field), ["/home/me/projects/beta"]);
    assert!(paths.calls().kind >= 3, "each change looks at the new path");
    type_text(&mut field, "ta/");
    assert_eq!(
        paths.calls().entries,
        listed + 1,
        "a new directory is listed"
    );
}

#[test]
fn keys_that_change_nothing_ask_nothing() {
    let (mut field, paths) = field("/srv/web", PathKind::Dir);
    let before = paths.calls();
    field.handle(press(KeyCode::Left));
    field.handle(press(KeyCode::Home));
    field.handle(press(KeyCode::Tab));
    assert_eq!(
        field.set_value("/srv/web"),
        Edit {
            changed: false,
            consumed: true
        }
    );
    field.set_candidates(vec![Candidate::new("/srv/web/static", 1)]);
    assert_eq!(paths.calls(), before);
    assert_eq!(
        values(&field),
        ["/srv/web/static"],
        "candidates rank from what was kept"
    );
}

#[test]
fn refresh_looks_again_at_everything() {
    let (mut field, paths) = field("~/projects/", PathKind::Dir);
    let before = paths.calls();
    field.refresh();
    let after = paths.calls();
    assert_eq!(
        (after.home, after.kind, after.entries),
        (before.home + 1, before.kind + 1, before.entries + 1)
    );
}

#[test]
fn a_refresh_sees_what_changed_under_the_field() {
    use std::sync::Mutex;

    /// A source whose directory appears after the field was built.
    struct Growing(Mutex<bool>);
    impl PathSource for Growing {
        fn entries(&self, dir: &Path) -> Vec<Entry> {
            if *self.0.lock().unwrap() && dir == Path::new("/srv/") {
                vec![Entry::new("new", PathKind::Dir)]
            } else {
                Vec::new()
            }
        }
        fn kind(&self, path: &Path) -> Option<PathKind> {
            (*self.0.lock().unwrap() && path == Path::new("/srv/new")).then_some(PathKind::Dir)
        }
        fn home(&self) -> Option<PathBuf> {
            None
        }
    }

    let source = Arc::new(Growing(Mutex::new(false)));
    let mut field = PathField::new("/srv/new", PathKind::Dir, source.clone());
    assert_eq!(field.validity(), Validity::Advisory);
    *source.0.lock().unwrap() = true;
    assert_eq!(field.validity(), Validity::Advisory, "kept until told");
    field.refresh();
    assert_eq!(field.validity(), Validity::Exists);
}

#[test]
fn debug_names_the_state_and_not_the_source() {
    let (field, _) = field("/srv", PathKind::Dir);
    let shown = format!("{field:?}");
    assert!(shown.starts_with("PathField {"), "{shown}");
    assert!(shown.contains("value: \"/srv\""), "{shown}");
    assert!(shown.contains("validity: Exists"), "{shown}");
}

#[test]
fn the_marks_are_the_shared_success_and_warning_marks() {
    let theme = Theme::default();
    assert_eq!(Validity::Exists.mark(), crate::glyphs::marks::OK);
    assert_eq!(Validity::Advisory.mark(), crate::glyphs::marks::WARNING);
    assert_eq!(Validity::Unknown.mark(), "?");
    assert_eq!(Validity::Exists.style(&theme).fg, Some(theme.success));
    assert_eq!(Validity::Advisory.style(&theme).fg, Some(theme.warning));
    assert_eq!(Validity::Unknown.style(&theme).fg, Some(theme.inert));
}
