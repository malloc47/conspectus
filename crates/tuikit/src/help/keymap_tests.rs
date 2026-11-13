use ratatui::crossterm::event::KeyCode;
use ratatui::text::Span;

use super::*;
use crate::glyphs::marks;
use crate::keys::{Bind, Scope, bind, ch, ctrl, key};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    Down,
    Up,
    Find,
    Back,
    Quit,
}

/// Whether there is anything to go back to.
struct Model {
    history: usize,
}

const LIST: Scope = Scope::Pane("list");

const KEYS: &[Bind<Do, Model>] = &[
    bind("list.up", LIST, &[key(KeyCode::Up), ch('k')], Do::Up)
        .help("Moving", "Move to the previous row"),
    bind("list.down", LIST, &[key(KeyCode::Down), ch('j')], Do::Down)
        .help("Moving", "Move to the next row"),
    bind("find", Scope::Global, &[ch('/'), ctrl('f')], Do::Find).help("Finding", "Find a row"),
    bind("back", Scope::Global, &[key(KeyCode::Backspace)], Do::Back)
        .help("Moving", "Go back")
        .available_when(|model: &Model| model.history > 0),
    bind("quit", Scope::Global, &[ch('q'), ctrl('c')], Do::Quit).help("Program", "Quit"),
];

fn keymap() -> Keymap<Do, Model> {
    Keymap::builder().rows(KEYS).build().unwrap()
}

/// Each section's title and its rows as `keys: description`.
fn outline(sections: &[HelpSection]) -> Vec<(String, Vec<String>)> {
    sections
        .iter()
        .map(|section| {
            let rows = section
                .rows
                .iter()
                .map(|row| match row {
                    HelpRow::Keys {
                        keys, description, ..
                    } => format!("{keys}: {description}"),
                    HelpRow::Legend { name, blurb, .. } => format!("[{name}] {blurb}"),
                })
                .collect();
            (section.title.to_string(), rows)
        })
        .collect()
}

#[test]
fn one_row_per_binding_under_its_section_in_the_keymaps_order() {
    let sections = keymap_sections(&keymap(), &[]);
    let up = format!("{} / k: Move to the previous row", marks::ARROW_UP);
    let down = format!("{} / j: Move to the next row", marks::ARROW_DOWN);
    assert_eq!(
        outline(&sections),
        vec![
            (
                "Moving".to_string(),
                vec![up, down, "Backspace: Go back".to_string()]
            ),
            (
                "Finding".to_string(),
                vec!["/ / Ctrl-F: Find a row".to_string()]
            ),
            ("Program".to_string(), vec!["q / Ctrl-C: Quit".to_string()]),
        ]
    );
}

#[test]
fn annotations_extend_a_row_follow_a_row_or_add_a_section() {
    let annotations = [
        HelpAnnotation::more("find", "; Enter picks it"),
        HelpAnnotation::row_after("find", HelpRow::keys("/ then Tab", "Find anything")),
        HelpAnnotation::row_after("find", HelpRow::keys("/ then Esc", "Find nothing")),
        HelpAnnotation::section(HelpSection::new("Marks").legend(
            Span::raw(marks::WARNING),
            "Warning",
            "needs a look",
        )),
        HelpAnnotation::more("find", "; Esc closes it"),
    ];
    let sections = keymap_sections(&keymap(), &annotations);
    let outline = outline(&sections);
    assert_eq!(
        outline[1].1,
        vec![
            "/ / Ctrl-F: Find a row; Enter picks it; Esc closes it".to_string(),
            "/ then Tab: Find anything".to_string(),
            "/ then Esc: Find nothing".to_string(),
        ]
    );
    assert_eq!(
        outline.last().map(|(title, _)| title.as_str()),
        Some("Marks")
    );
    assert_eq!(outline.len(), 4);
}

#[test]
fn an_annotation_whose_binding_is_gone_is_found() {
    let annotations = [
        HelpAnnotation::more("find", "; Enter picks it"),
        HelpAnnotation::row_after("search", HelpRow::keys("s", "Search")),
        HelpAnnotation::more("gone", "!"),
        HelpAnnotation::section(HelpSection::new("Marks")),
    ];
    assert_eq!(unanchored(&keymap(), &annotations), ["search", "gone"]);
    assert_eq!(annotations[3].anchor(), None);
}

#[test]
fn a_page_generated_in_a_context_dims_what_cannot_act_now() {
    let keymap = keymap();
    let back_row = |sections: Vec<HelpSection>| {
        sections[0]
            .rows
            .iter()
            .find_map(|row| match row {
                HelpRow::Keys {
                    keys, available, ..
                } if keys == "Backspace" => Some(*available),
                _ => None,
            })
            .expect("the back row")
    };
    assert!(
        back_row(keymap_sections(&keymap, &[])),
        "no context: all act"
    );
    assert!(!back_row(keymap_sections_in(
        &keymap,
        &[],
        &Model { history: 0 }
    )));
    assert!(back_row(keymap_sections_in(
        &keymap,
        &[],
        &Model { history: 1 }
    )));
}
