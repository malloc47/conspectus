use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;

use super::*;
use crate::glyphs::marks;
use crate::testkit::keys::{chord, press};
use crate::text::wrap_at;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Abort,
    Up,
    Down,
    Pick,
    Close,
    Copy,
}

impl KeyAction for Key {
    fn role(self) -> KeyRole {
        match self {
            Key::Abort => KeyRole::Abort,
            Key::Up | Key::Down => KeyRole::Move,
            Key::Pick => KeyRole::Apply,
            Key::Close => KeyRole::Leave,
            Key::Copy => KeyRole::Other,
        }
    }
}

const ROWS: &[KeyRow<Key>] = &[
    alias(ctrl('c'), Key::Abort),
    shown(key(KeyCode::Up), Key::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), Key::Up),
    shown(key(KeyCode::Down), Key::Down, marks::ARROW_DOWN, "move"),
    KeyRow::Note("type to filter"),
    shown(key(KeyCode::Enter), Key::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), Key::Close, "Esc", "close"),
];

const MORE: &[KeyRow<Key>] = &[
    include(ROWS),
    shown(ch('q'), Key::Close, "q", "close"),
    shown(ch('y'), Key::Copy, "y", "copy"),
];

/// A table that binds the convention's keys to the wrong roles.
const BACKWARDS: &[KeyRow<Key>] = &[
    alias(ctrl('c'), Key::Abort),
    alias(key(KeyCode::Enter), Key::Close),
    alias(key(KeyCode::Esc), Key::Pick),
    alias(ch(' '), Key::Copy),
];

/// A form's table built on the standard rows.
const FORM: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

fn segments(footer: &Footer) -> Vec<(&str, bool)> {
    footer
        .segments()
        .iter()
        .map(|segment| (segment.text.as_str(), segment.available))
        .collect()
}

#[test]
fn a_character_binds_as_typed_and_never_as_a_chord() {
    let g = ch('g');
    assert!(g.matches(KeyModifiers::NONE, KeyCode::Char('g')));
    assert!(g.matches(KeyModifiers::SHIFT, KeyCode::Char('g')));
    assert!(!g.matches(KeyModifiers::CONTROL, KeyCode::Char('g')));
    assert!(!g.matches(KeyModifiers::ALT, KeyCode::Char('g')));
    let upper = ch('G');
    assert!(upper.matches(KeyModifiers::SHIFT, KeyCode::Char('G')));
    assert!(upper.matches(KeyModifiers::NONE, KeyCode::Char('G')));
    assert!(!upper.matches(KeyModifiers::ALT, KeyCode::Char('G')));
    assert!(!upper.matches(
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        KeyCode::Char('G')
    ));
    assert!(!upper.matches(KeyModifiers::NONE, KeyCode::Char('g')));
}

#[test]
fn chords_need_their_exact_modifiers_and_other_keys_ignore_them() {
    let ctrl_r = ctrl('r');
    assert!(ctrl_r.matches(KeyModifiers::CONTROL, KeyCode::Char('r')));
    assert!(!ctrl_r.matches(KeyModifiers::NONE, KeyCode::Char('r')));
    assert!(!ctrl_r.matches(
        KeyModifiers::CONTROL | KeyModifiers::ALT,
        KeyCode::Char('r')
    ));
    assert!(alt('b').matches(KeyModifiers::ALT, KeyCode::Char('b')));
    assert!(ctrl_key(KeyCode::Left).matches(KeyModifiers::CONTROL, KeyCode::Left));
    let enter = key(KeyCode::Enter);
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT] {
        assert!(enter.matches(modifiers, KeyCode::Enter));
    }
    let global = KeyMatcher::AnyModExceptCtrl('r');
    assert!(global.matches(KeyModifiers::ALT, KeyCode::Char('r')));
    assert!(!global.matches(KeyModifiers::CONTROL, KeyCode::Char('r')));
}

#[test]
fn key_refuses_a_character() {
    let caught = std::panic::catch_unwind(|| key(KeyCode::Char('x')));
    assert!(caught.is_err(), "a character must be bound with `ch`");
}

#[test]
fn every_matcher_fires_on_its_own_example() {
    for matcher in [
        ch('j'),
        ch('G'),
        ctrl('n'),
        alt('f'),
        ctrl_key(KeyCode::Right),
        key(KeyCode::BackTab),
        KeyMatcher::AnyModExceptCtrl('w'),
    ] {
        assert!(matcher.matches_event(matcher.example()), "{matcher:?}");
    }
}

#[test]
fn labels_are_written_one_way() {
    let cases = [
        (key(KeyCode::Up), marks::ARROW_UP),
        (key(KeyCode::Down), marks::ARROW_DOWN),
        (key(KeyCode::Left), marks::ARROW_LEFT),
        (key(KeyCode::Right), marks::ARROW_RIGHT),
        (ch(' '), "Space"),
        (key(KeyCode::BackTab), "S-Tab"),
        (key(KeyCode::PageUp), "PgUp"),
        (key(KeyCode::PageDown), "PgDn"),
        (key(KeyCode::Enter), "Enter"),
        (key(KeyCode::Esc), "Esc"),
        (key(KeyCode::F(5)), "F5"),
        (ch('G'), "G"),
        (ch('?'), "?"),
        (ctrl('r'), "Ctrl-R"),
        (alt('b'), "Alt-b"),
        (ctrl_key(KeyCode::Left), "Ctrl-←"),
        (
            KeyMatcher::Exact {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::Enter,
            },
            "Shift-Enter",
        ),
        (
            KeyMatcher::Exact {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::BackTab,
            },
            "S-Tab",
        ),
        (KeyMatcher::AnyModExceptCtrl('w'), "w"),
    ];
    for (matcher, label) in cases {
        assert_eq!(key_label(&matcher), label, "{matcher:?}");
    }
}

#[test]
fn resolve_returns_the_first_matching_row_and_looks_inside_includes() {
    assert_eq!(resolve(ROWS, press(KeyCode::Up)), Some(Key::Up));
    assert_eq!(resolve(ROWS, press(KeyCode::Char('k'))), Some(Key::Up));
    assert_eq!(
        resolve(ROWS, chord(KeyModifiers::CONTROL, KeyCode::Char('c'))),
        Some(Key::Abort)
    );
    assert_eq!(resolve(ROWS, press(KeyCode::Char('x'))), None);
    assert_eq!(resolve(MORE, press(KeyCode::Esc)), Some(Key::Close));
    assert_eq!(resolve(MORE, press(KeyCode::Char('y'))), Some(Key::Copy));
    let flat: Vec<_> = rows(MORE).collect();
    assert_eq!(flat.len(), ROWS.len() + 2);
    assert!(flat.iter().all(|row| !matches!(row, KeyRow::Include(_))));
}

#[test]
fn footer_groups_labels_by_verb_in_order_and_skips_aliases() {
    let text = footer(ROWS);
    assert_eq!(
        text,
        ["↑/↓ move", "type to filter", "Enter pick", "Esc close"].join(HINT_SEPARATOR)
    );
    // An included table's verbs gather the including table's labels.
    assert_eq!(
        footer(MORE),
        [
            "↑/↓ move",
            "type to filter",
            "Enter pick",
            "Esc/q close",
            "y copy"
        ]
        .join(HINT_SEPARATOR)
    );
    let aliases: &[KeyRow<Key>] = &[alias(key(KeyCode::Esc), Key::Close)];
    assert!(Footer::new(aliases).is_empty());
    assert_eq!(footer(aliases), "");
    assert_eq!(Footer::new(ROWS).to_string(), text);
    assert_eq!(Footer::from(ROWS), Footer::new(ROWS));
}

#[test]
fn a_footer_can_show_some_actions_and_take_segments_of_a_control_own() {
    let leaving = Footer::only(MORE, |action| matches!(action, Key::Close | Key::Copy));
    assert_eq!(
        leaving.text(),
        ["type to filter", "Esc/q close", "y copy"].join(HINT_SEPARATOR),
        "notes stay; rows of other actions go"
    );
    let composed: Footer = std::iter::once(Segment {
        text: "m merge".into(),
        available: true,
    })
    .chain(leaving.segments()[1..].iter().cloned())
    .collect();
    assert_eq!(
        composed.text(),
        ["m merge", "Esc/q close", "y copy"].join(HINT_SEPARATOR)
    );
}

#[test]
fn footer_lines_wrap_between_segments_as_wrap_at_does() {
    let theme = Theme::default();
    let footer = Footer::new(MORE);
    for width in [6, 12, 20, 30, 45, 200] {
        let drawn: Vec<String> = footer
            .lines(width, &theme)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        let wrapped: Vec<String> = wrap_at(&footer.text(), HINT_SEPARATOR, width)
            .iter()
            .map(|row| row.trim_end().to_string())
            .collect();
        assert_eq!(drawn, wrapped, "{width}");
    }
}

#[test]
fn an_unavailable_action_is_dimmed_in_the_footer() {
    let theme = Theme::default();
    let rows = with_availability(MORE, |action| action != Key::Copy);
    assert_eq!(rows.len(), ROWS.len() + 2, "includes are spliced in");
    let footer = Footer::new(&rows);
    assert_eq!(
        segments(&footer),
        [
            ("↑/↓ move", true),
            ("type to filter", true),
            ("Enter pick", true),
            ("Esc/q close", true),
            ("y copy", false),
        ]
    );
    let line = &footer.lines(200, &theme)[0];
    let copy = line
        .spans
        .iter()
        .find(|span| span.content == "y copy")
        .unwrap();
    assert_eq!(copy.style, unavailable_style(&theme));
    let pick = line
        .spans
        .iter()
        .find(|span| span.content == "Enter pick")
        .unwrap();
    assert_eq!(
        pick.style,
        Style::default(),
        "available hints take the footer's style"
    );
    assert_ne!(unavailable_style(&theme), Style::default());
    // A verb stays lit while any of its keys can act.
    let one_off = with_availability(MORE, |_| true)
        .into_iter()
        .map(|row| match row {
            KeyRow::Bind {
                key: matcher,
                action,
                hint,
                ..
            } if matcher == ch('q') => KeyRow::Bind {
                key: matcher,
                action,
                hint,
                available: false,
            },
            other => other,
        })
        .collect::<Vec<_>>();
    assert!(Footer::new(&one_off).segments().iter().all(|s| s.available));
    // Availability is how a row is drawn; the key still resolves.
    assert_eq!(resolve(&rows, press(KeyCode::Char('y'))), Some(Key::Copy));
}

#[test]
fn a_segment_too_wide_for_its_row_wraps_on_rows_of_its_own() {
    let theme = Theme::default();
    let unavailable = with_availability(MORE, |action| action != Key::Pick);
    let lines = Footer::new(&unavailable).lines(6, &theme);
    let dim: Vec<String> = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .filter(|span| span.style == unavailable_style(&theme))
        .map(|span| span.content.to_string())
        .collect();
    assert_eq!(dim, ["Enter", "pick"]);
}

#[test]
fn mislabeled_flags_a_label_that_names_another_key() {
    assert!(mislabeled(MORE).is_empty());
    let wrong: &[KeyRow<Key>] = &[
        shown(ch('n'), Key::Close, "y", "confirm"),
        shown(key(KeyCode::Up), Key::Up, "Up", "move"),
        shown(ctrl('r'), Key::Copy, "Ctrl-r", "copy"),
    ];
    assert_eq!(
        mislabeled(wrong),
        vec![
            ("y", "n".to_string()),
            ("Up", marks::ARROW_UP.to_string()),
            ("Ctrl-r", "Ctrl-R".to_string()),
        ]
    );
}

#[test]
fn unresolved_flags_an_advertised_key_an_earlier_row_takes() {
    assert!(unresolved(MORE).is_empty());
    let shadowed: &[KeyRow<Key>] = &[
        alias(key(KeyCode::Esc), Key::Abort),
        shown(key(KeyCode::Esc), Key::Close, "Esc", "close"),
        shown(ch('y'), Key::Copy, "y", "copy"),
    ];
    assert_eq!(unresolved(shadowed), vec![("Esc", "Esc".to_string())]);
}

#[test]
fn loose_characters_flags_a_character_bound_with_its_chords() {
    assert!(loose_characters(MORE).is_empty());
    let loose: &[KeyRow<Key>] = &[
        alias(KeyMatcher::AnyMod(KeyCode::Char('g')), Key::Up),
        alias(KeyMatcher::AnyModExceptCtrl('y'), Key::Copy),
        alias(key(KeyCode::Home), Key::Up),
    ];
    assert_eq!(loose_characters(loose), ["g", "y"]);
}

#[test]
fn ctrl_c_comes_first_and_dismisses() {
    assert!(ctrl_c_first(ROWS));
    assert!(ctrl_c_first(MORE), "through an include");
    let late: &[KeyRow<Key>] = &[
        shown(key(KeyCode::Esc), Key::Close, "Esc", "close"),
        alias(ctrl('c'), Key::Abort),
    ];
    assert!(!ctrl_c_first(late));
    let copies: &[KeyRow<Key>] = &[alias(ctrl('c'), Key::Copy)];
    assert!(!ctrl_c_first(copies));
    let empty: &[KeyRow<Key>] = &[];
    assert!(!ctrl_c_first(empty));
}

#[test]
fn typed_chars_are_plain_and_shifted_characters_only() {
    assert_eq!(typed_char(press(KeyCode::Char('a'))), Some('a'));
    assert_eq!(
        typed_char(chord(KeyModifiers::SHIFT, KeyCode::Char('A'))),
        Some('A')
    );
    assert_eq!(typed_char(press(KeyCode::Char(' '))), Some(' '));
    assert_eq!(
        typed_char(chord(KeyModifiers::CONTROL, KeyCode::Char('a'))),
        None
    );
    assert_eq!(
        typed_char(chord(KeyModifiers::ALT, KeyCode::Char('a'))),
        None
    );
    assert_eq!(typed_char(press(KeyCode::Enter)), None);
}

fn roles<A: KeyAction>(table: &'static [KeyRow<A>]) -> impl Fn(KeyEvent) -> Vec<KeyRole> {
    move |event| {
        resolve(table, event)
            .map(KeyAction::role)
            .into_iter()
            .collect()
    }
}

#[test]
fn the_convention_is_checked_by_role() {
    let menu = Convention::new("menu", EnterMeans::Activates, SpaceMeans::Types, "");
    assert!(menu.breaks(&roles(ROWS)).is_empty());
    let pager = Convention::new("pager", EnterMeans::Nothing, SpaceMeans::Pages, "");
    let breaks = pager.breaks(&roles(ROWS));
    assert_eq!(breaks.len(), 2, "{breaks:?}");
    assert!(
        breaks[0].contains("Enter is bound to [Apply]"),
        "{breaks:?}"
    );
    assert!(breaks[1].contains("not to Page"), "{breaks:?}");
    let breaks = menu.breaks(&roles(BACKWARDS));
    assert_eq!(breaks.len(), 4, "{breaks:?}");
}

#[test]
fn the_standard_tables_keep_the_rules() {
    fn check<A: KeyAction + PartialEq + std::fmt::Debug>(
        name: &str,
        table: &'static [KeyRow<A>],
        convention: Convention,
    ) {
        assert!(
            mislabeled(table).is_empty(),
            "{name}: {:?}",
            mislabeled(table)
        );
        assert!(
            unresolved(table).is_empty(),
            "{name}: {:?}",
            unresolved(table)
        );
        assert!(loose_characters(table).is_empty(), "{name}");
        assert!(ctrl_c_first(table), "{name}: Ctrl-C first");
        let breaks = convention.breaks(&roles(table));
        assert!(breaks.is_empty(), "{name}: {breaks:?}");
    }
    use EnterMeans::{Activates, Nothing, Submits};
    use SpaceMeans::{Pages, Toggles, Types};
    check(
        "menu",
        MENU_KEYS,
        Convention::new("menu", Activates, Types, ""),
    );
    check(
        "dialog",
        DIALOG_KEYS,
        Convention::new("dialog", Submits, Types, ""),
    );
    check(
        "pager",
        PAGER_KEYS,
        Convention::new("pager", Nothing, Pages, ""),
    );
    check(
        "text",
        TEXT_INPUT_KEYS,
        Convention::new("text", Submits, Types, ""),
    );
    check(
        "checklist",
        CHECKLIST_KEYS,
        Convention::new("checklist", Submits, Toggles, ""),
    );
    check("form", FORM, Convention::new("form", Submits, Toggles, ""));
}

#[test]
fn q_dismisses_where_nothing_is_typed() {
    // House rule 4: q is a dismiss key in the tables with no text field,
    // and left to type in the ones that edit text.
    let q = press(KeyCode::Char('q'));
    assert_eq!(resolve(MENU_KEYS, q), Some(MenuKey::Abort));
    assert_eq!(resolve(DIALOG_KEYS, q), Some(DialogKey::Abort));
    assert_eq!(resolve(CHECKLIST_KEYS, q), Some(ChecklistKey::Abort));
    assert_eq!(resolve(PAGER_KEYS, q), Some(PagerKey::Close));
    assert_eq!(resolve(TEXT_INPUT_KEYS, q), None);
    assert_eq!(resolve(FORM_KEYS, q), None);
    assert_eq!(resolve(FORM_OPTION_KEYS, q), None);
    // A chord of q is not q.
    assert_eq!(
        resolve(MENU_KEYS, chord(KeyModifiers::CONTROL, KeyCode::Char('q'))),
        None
    );
}

#[test]
fn the_standard_footers_read_in_the_one_grammar() {
    let sep = HINT_SEPARATOR;
    assert_eq!(
        footer(MENU_KEYS),
        ["↑/↓ move", "Enter pick", "Esc close"].join(sep)
    );
    assert_eq!(
        footer(DIALOG_KEYS),
        ["Enter/y confirm", "Esc/n cancel"].join(sep)
    );
    assert_eq!(
        footer(PAGER_KEYS),
        ["j/k/PgDn/PgUp scroll", "Esc/q close"].join(sep)
    );
    assert_eq!(
        footer(TEXT_INPUT_KEYS),
        ["Enter confirm", "Esc cancel"].join(sep)
    );
    assert_eq!(
        footer(CHECKLIST_KEYS),
        [
            "↑/↓ move",
            "Space toggle",
            "a select all",
            "n select none",
            "Enter confirm",
            "Esc cancel"
        ]
        .join(sep)
    );
    assert_eq!(
        footer(FORM_KEYS),
        ["↑/↓ move", "Tab complete/next", "S-Tab back"].join(sep)
    );
    assert_eq!(footer(FORM_OPTION_KEYS), "←/→/Space change");
    for text in [
        footer(MENU_KEYS),
        footer(DIALOG_KEYS),
        footer(PAGER_KEYS),
        footer(CHECKLIST_KEYS),
        footer(FORM_KEYS),
    ] {
        assert!(!text.contains(" to ") && !text.contains(" = "), "{text}");
    }
}

#[test]
fn every_list_moves_with_ctrl_p_and_ctrl_n() {
    let ctrl_n = chord(KeyModifiers::CONTROL, KeyCode::Char('n'));
    let ctrl_p = chord(KeyModifiers::CONTROL, KeyCode::Char('p'));
    assert_eq!(resolve(MENU_KEYS, ctrl_n), Some(MenuKey::Down));
    assert_eq!(resolve(MENU_KEYS, ctrl_p), Some(MenuKey::Up));
    assert_eq!(resolve(CHECKLIST_KEYS, ctrl_n), Some(ChecklistKey::Down));
    assert_eq!(resolve(CHECKLIST_KEYS, ctrl_p), Some(ChecklistKey::Up));
    assert_eq!(resolve(FORM_KEYS, ctrl_n), Some(FormKey::Down));
    assert_eq!(resolve(FORM_KEYS, ctrl_p), Some(FormKey::Up));
    // A pager has no cursor, and a dialog no list.
    assert_eq!(resolve(PAGER_KEYS, ctrl_n), None);
    assert_eq!(resolve(DIALOG_KEYS, ctrl_n), None);
}

#[test]
fn a_pager_goes_to_its_ends_on_plain_g_and_shift_g_only() {
    let g = |modifiers| chord(modifiers, KeyCode::Char('g'));
    let upper = |modifiers| chord(modifiers, KeyCode::Char('G'));
    assert_eq!(
        resolve(PAGER_KEYS, g(KeyModifiers::NONE)),
        Some(PagerKey::Top)
    );
    assert_eq!(
        resolve(PAGER_KEYS, upper(KeyModifiers::SHIFT)),
        Some(PagerKey::Bottom)
    );
    assert_eq!(
        resolve(PAGER_KEYS, upper(KeyModifiers::NONE)),
        Some(PagerKey::Bottom)
    );
    assert_eq!(resolve(PAGER_KEYS, g(KeyModifiers::CONTROL)), None);
    assert_eq!(resolve(PAGER_KEYS, g(KeyModifiers::ALT)), None);
    assert_eq!(resolve(PAGER_KEYS, upper(KeyModifiers::ALT)), None);
    assert_eq!(
        resolve(PAGER_KEYS, press(KeyCode::Home)),
        Some(PagerKey::Top)
    );
    assert_eq!(
        resolve(PAGER_KEYS, press(KeyCode::End)),
        Some(PagerKey::Bottom)
    );
}
