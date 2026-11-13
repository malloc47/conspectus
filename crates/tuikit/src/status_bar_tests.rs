use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::*;
use crate::feedback::Severity;
use crate::glyphs::marks;
use crate::keys::{KeyRow, MENU_KEYS, alias, ch, ctrl, include, key, shown};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Up,
    Down,
    Open,
    Edit,
    Delete,
    Quit,
    Reload,
}

/// A list's keys: moving, opening, editing, deleting, and leaving.
const LIST: &[KeyRow<Key>] = &[
    shown(key(KeyCode::Up), Key::Up, marks::ARROW_UP, "move").rank(3),
    alias(ch('k'), Key::Up),
    shown(key(KeyCode::Down), Key::Down, marks::ARROW_DOWN, "move").rank(3),
    shown(key(KeyCode::Enter), Key::Open, "Enter", "open").rank(6),
    shown(ch('e'), Key::Edit, "e", "edit"),
    shown(ch('d'), Key::Delete, "d", "delete").rank(2),
    KeyRow::Note("type to filter"),
];

/// The program's keys, around any list's.
const PROGRAM: &[KeyRow<Key>] = &[
    shown(ch('r'), Key::Reload, "r", "reload").rank(1),
    shown(ctrl('q'), Key::Quit, "Ctrl-Q", "quit").rank(9),
];

const BOTH: &[KeyRow<Key>] = &[include(LIST), include(PROGRAM)];

fn texts(bar: &KeyHintBar) -> Vec<String> {
    bar.hints().iter().map(KeyHint::text).collect()
}

fn ranks(bar: &KeyHintBar) -> Vec<u32> {
    bar.hints().iter().map(KeyHint::rank).collect()
}

#[test]
fn a_table_is_hints_in_its_order_with_labels_that_share_a_verb_joined() {
    let bar = KeyHintBar::new().table(BOTH, 4);
    assert_eq!(
        texts(&bar),
        [
            "↑/↓ move",
            "Enter open",
            "e edit",
            "d delete",
            "type to filter",
            "r reload",
            "Ctrl-Q quit"
        ]
    );
    // A row's own rank, else the table's; a note takes the table's.
    assert_eq!(ranks(&bar), [3, 6, 4, 2, 4, 1, 9]);
    assert_eq!(
        bar.text(),
        "↑/↓ move · Enter open · e edit · d delete · type to filter · r reload · Ctrl-Q quit"
    );
    // The text a footer of the same table draws, word for word.
    assert_eq!(bar.text(), crate::keys::footer(BOTH));
}

#[test]
fn every_key_a_hint_names_is_listed_and_an_alias_is_not() {
    let bar = KeyHintBar::new().table(LIST, 1);
    let keys: Vec<KeyMatcher> = bar.keys().collect();
    assert_eq!(
        keys,
        [
            key(KeyCode::Up),
            key(KeyCode::Down),
            key(KeyCode::Enter),
            ch('e'),
            ch('d')
        ]
    );
    assert!(!keys.contains(&ch('k')), "an alias is not advertised");
    let note = &bar.hints()[4];
    assert_eq!(
        (note.text().as_str(), note.keys().len()),
        ("type to filter", 0)
    );
}

#[test]
fn an_annotation_leaves_a_row_out_or_changes_what_it_says() {
    let bar = KeyHintBar::new().annotated(BOTH, 4, |key| match key {
        Key::Up | Key::Down => None,
        Key::Open => Some(Annotation::new().verb("play").detail("track 3")),
        Key::Edit => Some(Annotation::new().rank(8)),
        Key::Delete => Some(Annotation::new().because("read-only")),
        Key::Quit | Key::Reload => Some(Annotation::new()),
    });
    assert_eq!(
        texts(&bar),
        [
            "Enter play track 3",
            "e edit",
            "d delete: read-only",
            "type to filter",
            "r reload",
            "Ctrl-Q quit"
        ],
        "a note stays; a row left out goes"
    );
    assert_eq!(ranks(&bar), [6, 8, 2, 4, 1, 9]);
    let available: Vec<bool> = bar.hints().iter().map(KeyHint::is_available).collect();
    assert_eq!(available, [true, true, false, true, true, true]);
    assert_eq!(
        bar.hints()[2].keys(),
        [ch('d')],
        "a dimmed key is still named"
    );
}

#[test]
fn rows_that_end_up_with_the_same_words_join_and_keep_the_higher_rank() {
    let bar = KeyHintBar::new().annotated(LIST, 1, |key| match key {
        Key::Open | Key::Edit => Some(Annotation::new().verb("edit").detail("notes.txt")),
        Key::Delete => Some(
            Annotation::new()
                .verb("edit")
                .detail("notes.txt")
                .unavailable(),
        ),
        _ => None,
    });
    assert_eq!(texts(&bar), ["Enter/e/d edit notes.txt", "type to filter"]);
    let hint = &bar.hints()[0];
    assert_eq!(hint.keys(), [key(KeyCode::Enter), ch('e'), ch('d')]);
    assert_eq!(hint.rank(), 6, "Enter's rank, the highest of the three");
    assert!(
        hint.is_available(),
        "available while any of its keys can act"
    );
    // Words shared across tables join too: one grammar for the whole bar.
    let across = KeyHintBar::new()
        .annotated(LIST, 1, |key| {
            (*key == Key::Open).then(|| Annotation::new().verb("quit"))
        })
        .table(PROGRAM, 1);
    assert_eq!(
        texts(&across),
        ["Enter/Ctrl-Q quit", "type to filter", "r reload"]
    );
    assert_eq!(across.hints()[0].rank(), 9);
}

#[test]
fn a_controls_footer_goes_in_whole_with_its_keys() {
    let footer = Footer::new(MENU_KEYS);
    let bar = KeyHintBar::new().footer(&footer, 7);
    assert_eq!(bar.text(), footer.text());
    assert!(bar.hints().iter().all(|hint| hint.rank() == 7));
    let keys: Vec<KeyMatcher> = bar.keys().collect();
    let expected: Vec<KeyMatcher> = footer
        .segments()
        .iter()
        .flat_map(|segment| segment.keys.iter().copied())
        .collect();
    assert_eq!(keys, expected);
    assert!(!keys.is_empty());
    // A ranked row keeps its rank in a footer, and in the bar; a note,
    // which names no key, holds on a step below the footer's rank, so it
    // gives way before the keys.
    let ranked = KeyHintBar::new().footer(&Footer::new(LIST), 1);
    assert_eq!(ranks(&ranked), [3, 6, 1, 2, 0]);
    let noted = KeyHintBar::new().footer(&Footer::new(LIST), 5);
    assert_eq!(ranks(&noted), [3, 6, 5, 2, 4]);
    // A whole segment never joins another, even with the same words.
    let twice = KeyHintBar::new().footer(&footer, 7).footer(&footer, 7);
    assert_eq!(twice.hints().len(), 2 * footer.segments().len());
}

#[test]
fn hints_recede_and_a_hint_that_cannot_act_is_dimmed() {
    let theme = Theme::default();
    let bar = KeyHintBar::new().annotated(LIST, 1, |key| match key {
        Key::Delete => Some(Annotation::new().because("read-only")),
        Key::Open => Some(Annotation::new()),
        _ => None,
    });
    let segments = bar.segments(&theme);
    let line = PriorityLine::new(&theme)
        .segment(segments[0].clone())
        .segment(segments[1].clone());
    let spans = line.spans(80);
    let style_of = |text: &str| {
        spans
            .iter()
            .find(|span| span.content == text)
            .map(|span| span.style)
    };
    assert_eq!(
        style_of("Enter open"),
        Some(Style::default().add_modifier(theme.muted))
    );
    assert_eq!(
        style_of("d delete: read-only"),
        Some(unavailable_style(&theme))
    );
    assert_eq!(
        segments.iter().map(Segment::rank_of).collect::<Vec<_>>(),
        [Some(6), Some(2), Some(1)]
    );
}

fn sample_bar(theme: &Theme) -> StatusBar<'_> {
    let hints = KeyHintBar::new().table(BOTH, 4);
    StatusBar::new(theme)
        .lead([Span::raw("! 2  ")])
        .scope([Span::raw("[list] ")])
        .segment(Segment::new("sort:name").rank(5))
        .hints(&hints)
        .trail([Span::raw("  net:off")])
}

#[test]
fn the_keys_fit_in_the_cells_the_fixed_spans_leave() {
    let theme = Theme::default();
    let bar = sample_bar(&theme);
    let text = |width| bar.line(width).to_string();
    assert_eq!(
        text(140),
        "! 2  [list] sort:name · ↑/↓ move · Enter open · e edit · d delete · type to filter · \
         r reload · Ctrl-Q quit  net:off"
    );
    // The lowest ranks give way first; the fixed spans never do.
    assert_eq!(
        text(60),
        "! 2  [list] sort:name · Enter open · Ctrl-Q quit …  net:off"
    );
    assert_eq!(text(40), "! 2  [list] Ctrl-Q quit …  net:off");
    for width in 20..=140 {
        let line = bar.line(width);
        assert!(
            line.width() <= width.max(5 + 7 + 9),
            "{width}: {line} is {} cells",
            line.width()
        );
    }
    assert!(!bar.shows_message());
}

#[test]
fn a_message_takes_the_bars_place_in_its_severitys_style() {
    let theme = Theme::default();
    for severity in Severity::ALL {
        let feedback = Feedback::new(severity, "copied the path");
        let bar = sample_bar(&theme).message(Some(&feedback));
        assert!(bar.shows_message());
        let spans = bar.spans(80);
        assert_eq!(
            Line::from(spans.clone()).to_string(),
            "! 2  copied the path",
            "the lead stays; the scope, the line, and the trail go"
        );
        assert_eq!(
            spans.last().map(|span| span.style),
            Some(severity.style(&theme))
        );
    }
    // No message is the keys.
    assert!(!sample_bar(&theme).message(None).shows_message());
}

#[test]
fn a_message_too_wide_for_its_room_ends_in_an_ellipsis() {
    let theme = Theme::default();
    let feedback = Feedback::warning("nothing to open on this row yet");
    let bar = StatusBar::new(&theme)
        .lead([Span::raw("! 2  ")])
        .message(Some(&feedback));
    assert_eq!(bar.line(20).to_string(), "! 2  nothing to ope…");
    assert_eq!(bar.line(20).width(), 20);
    assert_eq!(bar.line(5).to_string(), "! 2  ", "no room, no message");
}

#[test]
fn the_bar_draws_on_the_first_row_of_its_area() {
    let theme = Theme::default();
    let bar = sample_bar(&theme);
    let area = Rect::new(0, 0, 40, 2);
    let mut buf = Buffer::empty(area);
    bar.render(area, &mut buf);
    let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert_eq!(row.trim_end(), "! 2  [list] Ctrl-Q quit …  net:off");
    let below: String = (0..40).map(|x| buf[(x, 1)].symbol().to_string()).collect();
    assert_eq!(below.trim(), "");
    // The hints recede; the host's segment keeps its own style.
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let styled = StatusBar::new(&theme)
        .segment(Segment::new(Span::styled("sort:name", bold)).rank(1))
        .spans(20);
    assert_eq!(styled[0].style, bold);
}

mod from_a_keymap {
    use super::*;
    use crate::keys::{ActiveScopes, Bind, Keymap, Scope, bind, exact};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Do {
        Up,
        Down,
        Open,
        Back,
        Peek,
        Quit,
    }

    /// Whether there is anything to go back to.
    struct Model {
        history: usize,
    }

    const FILES: Scope = Scope::Pane("files");
    const PREVIEW: Scope = Scope::Pane("preview");

    const KEYS: &[Bind<Do, Model>] = &[
        bind("files.open", FILES, &[exact(KeyCode::Enter)], Do::Open)
            .verb("open")
            .rank(6)
            .help("Files", "Open the file"),
        bind("files.up", FILES, &[key(KeyCode::Up), ch('k')], Do::Up)
            .verb("move")
            .rank(3)
            .help("Files", "Up"),
        bind(
            "files.down",
            FILES,
            &[key(KeyCode::Down), ch('j')],
            Do::Down,
        )
        .verb("move")
        .rank(3)
        .help("Files", "Down"),
        bind("preview.peek", PREVIEW, &[ch('p')], Do::Peek)
            .verb("peek")
            .help("Preview", "Peek"),
        bind("peek", Scope::Global, &[ch('p')], Do::Peek)
            .verb("peek")
            .help("Program", "Peek from anywhere"),
        bind("back", Scope::Global, &[key(KeyCode::Backspace)], Do::Back)
            .verb("back")
            .rank(10)
            .help("Program", "Go back")
            .available_when(|model: &Model| model.history > 0),
        bind("quit", Scope::Global, &[ch('q'), ctrl('c')], Do::Quit)
            .verb("quit")
            .rank(9)
            .help("Program", "Quit"),
    ];

    fn keymap() -> Keymap<Do, Model> {
        Keymap::builder().rows(KEYS).build().unwrap()
    }

    #[test]
    fn the_bar_names_the_active_scopes_bindings_in_the_keymaps_order() {
        let keymap = keymap();
        let model = Model { history: 0 };
        let every = |_: &Do| Some(Annotation::new());
        let bar = KeyHintBar::new().keymap(&keymap, ActiveScopes::pane("files"), &model, 1, every);
        let moves = format!("{}/{} move", marks::ARROW_UP, marks::ARROW_DOWN);
        assert_eq!(
            bar.text(),
            format!("Enter open · {moves} · p peek · q quit")
        );
        // Each hint names the first key of its binding, with its rank.
        let ranks: Vec<u32> = bar.hints().iter().map(KeyHint::rank).collect();
        assert_eq!(ranks, [6, 3, 1, 9]);
        assert_eq!(bar.hints()[3].keys(), &[ch('q')]);
    }

    #[test]
    fn a_key_a_more_particular_scope_takes_is_named_once_as_its() {
        let keymap = keymap();
        let model = Model { history: 0 };
        // On the preview pane its own `p` takes the program's.
        let bar = KeyHintBar::new().keymap(
            &keymap,
            ActiveScopes::pane("preview"),
            &model,
            1,
            |action| {
                Some(match action {
                    Do::Peek => Annotation::new().detail("here"),
                    _ => Annotation::new(),
                })
            },
        );
        assert_eq!(bar.text(), "p peek here · q quit");
        assert_eq!(
            keymap.overrides(),
            vec![("preview.peek", "peek", "p".to_string())]
        );
    }

    #[test]
    fn a_binding_that_cannot_act_now_is_left_out_and_an_annotation_still_decides() {
        let keymap = keymap();
        let global = ActiveScopes::global();
        let every = |_: &Do| Some(Annotation::new());
        let none = KeyHintBar::new().keymap(&keymap, global, &Model { history: 0 }, 1, every);
        assert_eq!(none.text(), "p peek · q quit");
        let some = KeyHintBar::new().keymap(&keymap, global, &Model { history: 3 }, 1, |action| {
            (*action != Do::Peek).then(|| match action {
                Do::Quit => Annotation::new().because("unsaved"),
                _ => Annotation::new(),
            })
        });
        assert_eq!(some.text(), "Backspace back · q quit: unsaved");
        assert!(!some.hints()[1].is_available());
    }
}
