use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::keys::{alias, ch, ctrl, include, key, shown, with_availability};

/// What a small document reader's keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reader {
    Down,
    Up,
    Top,
    Find,
    Leave,
    /// Bound by [`UNLISTED_KEYS`] but left out of the listing.
    Hidden,
}

impl HelpAction for Reader {
    const LISTED: &'static [Self] = &[
        Reader::Down,
        Reader::Up,
        Reader::Top,
        Reader::Find,
        Reader::Leave,
        // Listed twice: the page lists it once.
        Reader::Down,
    ];

    fn help(self) -> HelpEntry {
        match self {
            Reader::Down => HelpEntry::new("Moving", "Scroll one line down"),
            Reader::Up => HelpEntry::new("Moving", "Scroll one line up"),
            Reader::Top => HelpEntry::new("Moving", "Jump to the start"),
            Reader::Find => HelpEntry::new("Finding", "Find in the document"),
            Reader::Leave => HelpEntry::new("Leaving", "Close the reader"),
            Reader::Hidden => HelpEntry::new("Moving", "Never listed"),
        }
    }
}

const MOVING: &[KeyRow<Reader>] = &[
    alias(ch('j'), Reader::Down),
    alias(key(KeyCode::Down), Reader::Down),
    shown(ch('k'), Reader::Up, "k", "scroll"),
];

const READER_KEYS: &[KeyRow<Reader>] = &[
    alias(ctrl('c'), Reader::Leave),
    include(MOVING),
    // The same key twice for one action is listed once.
    alias(ch('j'), Reader::Down),
    alias(ch('g'), Reader::Top),
    alias(ch('q'), Reader::Leave),
    KeyRow::Note("type to find"),
];

const UNLISTED_KEYS: &[KeyRow<Reader>] = &[
    alias(ctrl('c'), Reader::Leave),
    alias(ch('x'), Reader::Hidden),
];

fn keys_rows(sections: &[HelpSection]) -> Vec<(&str, String, bool)> {
    sections
        .iter()
        .flat_map(|section| {
            section.rows.iter().filter_map(|row| match row {
                HelpRow::Keys {
                    keys,
                    description,
                    available,
                } => Some((
                    section.title.as_ref(),
                    format!("{keys} {description}"),
                    *available,
                )),
                HelpRow::Legend { .. } => None,
            })
        })
        .collect()
}

#[test]
fn a_row_per_listed_action_under_its_section_in_listing_order() {
    let sections = sections_for(READER_KEYS);
    let titles: Vec<&str> = sections.iter().map(|s| s.title.as_ref()).collect();
    assert_eq!(titles, ["Moving", "Leaving"], "Finding has no key");
    assert_eq!(
        keys_rows(&sections),
        [
            ("Moving", "j / ↓ Scroll one line down".to_string(), true),
            ("Moving", "k Scroll one line up".to_string(), true),
            ("Moving", "g Jump to the start".to_string(), true),
            ("Leaving", "q / Ctrl-C Close the reader".to_string(), true),
        ]
    );
}

#[test]
fn an_action_no_key_binds_is_left_out() {
    let sections = sections_for(MOVING);
    assert!(
        keys_rows(&sections)
            .iter()
            .all(|(_, row, _)| !row.contains("Close") && !row.contains("Find")),
        "{sections:?}"
    );
}

#[test]
fn plain_keys_come_before_chords_whatever_the_table_order() {
    const KEYS: &[KeyRow<Reader>] = &[
        alias(ctrl('d'), Reader::Down),
        alias(crate::keys::alt('j'), Reader::Down),
        alias(ch('j'), Reader::Down),
        alias(key(KeyCode::PageDown), Reader::Down),
    ];
    let sections = sections_for(KEYS);
    assert_eq!(
        keys_rows(&sections),
        [(
            "Moving",
            "j / PgDn / Ctrl-D / Alt-j Scroll one line down".to_string(),
            true
        )]
    );
}

#[test]
fn a_row_is_dimmed_only_when_none_of_its_keys_can_act() {
    let rows = with_availability(READER_KEYS, |action| action != Reader::Top);
    let sections = sections_for(&rows);
    let available: Vec<bool> = keys_rows(&sections).iter().map(|row| row.2).collect();
    assert_eq!(available, [true, true, false, true]);
}

#[test]
fn every_key_of_a_described_table_is_described() {
    assert!(undescribed(READER_KEYS).is_empty());
}

#[test]
fn a_key_whose_action_is_not_listed_is_reported() {
    assert_eq!(undescribed(UNLISTED_KEYS), ["x"]);
    let sections = sections_for(UNLISTED_KEYS);
    assert!(
        keys_rows(&sections)
            .iter()
            .all(|row| !row.1.contains("Never")),
        "an unlisted action has no row"
    );
}

#[test]
fn a_key_whose_action_says_nothing_is_reported() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Blank {
        Quiet,
        Nameless,
        Said,
    }
    impl HelpAction for Blank {
        const LISTED: &'static [Self] = &[Blank::Quiet, Blank::Nameless, Blank::Said];
        fn help(self) -> HelpEntry {
            match self {
                Blank::Quiet => HelpEntry::new("Moving", " "),
                Blank::Nameless => HelpEntry::new("", "Has no section"),
                Blank::Said => HelpEntry::new("Moving", "Says something"),
            }
        }
    }
    const KEYS: &[KeyRow<Blank>] = &[
        alias(ch('a'), Blank::Quiet),
        alias(ch('b'), Blank::Nameless),
        alias(ch('c'), Blank::Said),
    ];
    assert_eq!(undescribed(KEYS), ["a", "b"]);
}

#[test]
fn hand_written_sections_build_in_order() {
    let mark = Span::raw("*");
    let section = HelpSection::new("Marks")
        .keys("x", "Mark the row")
        .legend(mark.clone(), "Marked", "the row is marked")
        .row(HelpRow::keys("y", "Unmark it").available(false));
    assert_eq!(section.title, "Marks");
    assert_eq!(
        section.rows,
        [
            HelpRow::keys("x", "Mark the row"),
            HelpRow::legend(mark.clone(), "Marked", "the row is marked"),
            HelpRow::Keys {
                keys: "y".into(),
                description: "Unmark it".into(),
                available: false
            },
        ]
    );
    assert_eq!(
        HelpRow::legend(mark.clone(), "Marked", "x").available(false),
        HelpRow::legend(mark, "Marked", "x"),
        "a legend row has no availability"
    );
}
