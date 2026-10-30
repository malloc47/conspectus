use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::*;
use crate::glyphs::marks;
use crate::help::HelpEntry;
use crate::keys::{alias, alt_key, ch, ctrl, exact, key, shown};
use crate::testkit::keys::{chord, press, typed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    Down,
    Up,
    Open,
    Reveal,
    Clear,
    Quit,
    Group,
    Top,
    Pane(Step),
}

/// A pane control's own actions, as a framework table binds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Next,
    Previous,
    Fold,
    Never,
}

impl HelpAction for Step {
    // The help's order, not the table's: `Next` first.
    const LISTED: &'static [Self] = &[Step::Next, Step::Previous, Step::Fold, Step::Never];

    fn help(self) -> HelpEntry {
        HelpEntry::new("Steps", "Step")
    }
}

const STEP_KEYS: &[KeyRow<Step>] = &[
    shown(key(KeyCode::Up), Step::Previous, marks::ARROW_UP, "move"),
    alias(ch('k'), Step::Previous),
    alias(ch('j'), Step::Next),
    shown(key(KeyCode::Down), Step::Next, marks::ARROW_DOWN, "move").rank(4),
    shown(ch('e'), Step::Fold, "e", "fold"),
];

/// What the host knows: whether anything is selected.
struct Model {
    selected: bool,
}

const LIST: Scope = Scope::Pane("list");

const KEYS: &[Bind<Do, Model>] = &[
    bind("list.down", LIST, &[key(KeyCode::Down), ch('j')], Do::Down)
        .verb("move")
        .help("Moving", "Move to the next row")
        .rank(3),
    bind("list.up", LIST, &[key(KeyCode::Up), ch('k')], Do::Up)
        .verb("move")
        .help("Moving", "Move to the previous row"),
    bind("list.open", LIST, &[exact(KeyCode::Enter)], Do::Open)
        .verb("open")
        .help("Rows", "Open the selected row")
        .available_when(|model: &Model| model.selected),
    bind(
        "reveal",
        Scope::Global,
        &[
            KeyMatcher::Exact {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::Enter,
            },
            alt_key(KeyCode::Enter),
        ],
        Do::Reveal,
    )
    .help("Rows", "Show the row where it lives"),
    bind("filters.clear", LIST, &[ch('F')], Do::Clear).help("Rows", "Clear the filters"),
    bind("quit", Scope::Global, &[ch('q'), ctrl('c')], Do::Quit)
        .verb("quit")
        .help("Program", "Quit"),
    bind("group", Scope::Global, &[ctrl('g')], Do::Group).help("Program", "Group the rows"),
    bind(
        "list.top",
        LIST,
        &[KeyMatcher::AnyModExceptCtrl('g')],
        Do::Top,
    )
    .help("Moving", "Move to the first row"),
];

fn keymap() -> Keymap<Do, Model> {
    Keymap::builder()
        .rows(KEYS)
        .table(STEP_KEYS, |step| {
            let id = match step {
                Step::Next => "steps.next",
                Step::Previous => "steps.previous",
                Step::Fold => "steps.fold",
                Step::Never => "steps.never",
            };
            bind(id, Scope::Pane("steps"), &[], Do::Pane(step)).help("Steps", "Step")
        })
        .build()
        .expect("the keys collide nowhere")
}

fn found(keymap: &Keymap<Do, Model>, active: ActiveScopes, event: KeyEvent) -> Option<Do> {
    keymap.resolve(active, event).map(Binding::action)
}

#[test]
fn a_key_is_looked_up_in_the_focused_panes_scope_then_the_global_one() {
    let keymap = keymap();
    let list = ActiveScopes::pane("list");
    assert_eq!(found(&keymap, list, press(KeyCode::Down)), Some(Do::Down));
    assert_eq!(found(&keymap, list, typed('q')), Some(Do::Quit));
    assert_eq!(
        found(
            &keymap,
            list,
            chord(KeyModifiers::CONTROL, KeyCode::Char('g'))
        ),
        Some(Do::Group)
    );
    assert_eq!(found(&keymap, list, typed('g')), Some(Do::Top));
    // Another pane has focus: the list's keys are not bound there, the
    // program's still are.
    let steps = ActiveScopes::pane("steps");
    assert_eq!(found(&keymap, steps, typed('F')), None);
    assert_eq!(
        found(&keymap, steps, press(KeyCode::Down)),
        Some(Do::Pane(Step::Next))
    );
    assert_eq!(
        found(&keymap, steps, chord(KeyModifiers::ALT, KeyCode::Enter)),
        Some(Do::Reveal)
    );
    assert_eq!(
        found(&keymap, ActiveScopes::global(), press(KeyCode::Down)),
        None
    );
}

#[test]
fn an_overlay_mode_is_looked_up_before_the_pane_and_the_program() {
    const MODE: &[Bind<Do>] = &[
        bind("mode.quit", Scope::Overlay("select"), &[ch('q')], Do::Clear).help("Mode", "Leave"),
        bind("quit", Scope::Global, &[ch('q')], Do::Quit).help("Program", "Quit"),
        bind("list.down", LIST, &[ch('j')], Do::Down).help("Moving", "Down"),
    ];
    let keymap = Keymap::builder().rows(MODE).build().unwrap();
    let active = ActiveScopes::pane("list").overlay("select");
    assert_eq!(
        active.scopes().collect::<Vec<_>>(),
        vec![Scope::Overlay("select"), LIST, Scope::Global]
    );
    assert_eq!(
        keymap.resolve(active, typed('q')).map(Binding::id),
        Some("mode.quit")
    );
    assert_eq!(
        keymap.resolve(active, typed('j')).map(Binding::id),
        Some("list.down")
    );
    assert_eq!(
        keymap
            .resolve(ActiveScopes::pane("list"), typed('q'))
            .map(Binding::id),
        Some("quit")
    );
    assert_eq!(
        keymap.overrides(),
        vec![("mode.quit", "quit", "q".to_string())],
        "the mode takes the program's q while it is on"
    );
}

/// The ids the build reports for each conflict, in order.
fn conflicts<A: Copy + PartialEq + std::fmt::Debug>(rows: &[Bind<A>]) -> Vec<Conflict> {
    Keymap::builder()
        .rows(rows)
        .build()
        .expect_err("the rows conflict")
        .conflicts()
        .to_vec()
}

#[test]
fn a_key_shadowed_by_an_earlier_key_of_its_scope_fails_the_build() {
    // `g` with any modifier comes first and would take `Ctrl-G`.
    const SHADOWED: &[Bind<Do>] = &[
        bind(
            "top",
            Scope::Global,
            &[KeyMatcher::AnyMod(KeyCode::Char('g'))],
            Do::Top,
        )
        .help("x", "Top"),
        bind("group", Scope::Global, &[ctrl('g')], Do::Group).help("x", "Group"),
    ];
    // The other order resolves, but the two still overlap: the table's
    // order would decide, so it is refused too.
    const SHADOWING: &[Bind<Do>] = &[
        bind(
            "reveal",
            Scope::Global,
            &[KeyMatcher::Exact {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::Enter,
            }],
            Do::Reveal,
        )
        .help("x", "Reveal"),
        bind("open", Scope::Global, &[key(KeyCode::Enter)], Do::Open).help("x", "Open"),
    ];
    assert_eq!(
        conflicts(SHADOWED),
        vec![Conflict::Overlap {
            scope: Scope::Global,
            first: "top",
            second: "group",
            key: "Ctrl-G".to_string(),
        }]
    );
    let found = conflicts(SHADOWING);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].to_string(),
        "`reveal` and `open` both take `Shift-Enter` in the global scope, so `open` never fires on it"
    );
}

#[test]
fn a_binding_that_repeats_a_key_fails_the_build() {
    const TWICE: &[Bind<Do>] = &[bind(
        "open",
        LIST,
        &[key(KeyCode::Enter), exact(KeyCode::Enter)],
        Do::Open,
    )
    .help("x", "Open")];
    let found = conflicts(TWICE);
    assert_eq!(
        found[0].to_string(),
        "`open` binds `Enter` twice in the `list` pane's scope"
    );
}

#[test]
fn ids_are_unique_well_formed_and_every_binding_has_keys_and_help() {
    const BAD: &[Bind<Do>] = &[
        bind("Open Row", LIST, &[ch('o')], Do::Open).help("x", "Open"),
        bind("quit", Scope::Global, &[ch('q')], Do::Quit).help("x", "Quit"),
        bind("quit", Scope::Global, &[ch('x')], Do::Quit).help("x", "Quit"),
        bind("keyless", Scope::Global, &[], Do::Clear).help("x", "Clear"),
        bind("silent", Scope::Global, &[ch('s')], Do::Top),
    ];
    assert_eq!(
        conflicts(BAD),
        vec![
            Conflict::MalformedId("Open Row"),
            Conflict::DuplicateId("quit"),
            Conflict::NoKeys("keyless"),
            Conflict::Undescribed("silent"),
        ]
    );
}

#[test]
fn every_binding_is_found_by_its_stable_id() {
    // A later remap names a binding by its id, so the table need not
    // change: every id is the binding's, whatever its keys.
    let keymap = keymap();
    for binding in keymap.bindings() {
        assert_eq!(
            keymap.get(binding.id()).map(Binding::id),
            Some(binding.id())
        );
    }
    assert!(keymap.get("nothing").is_none());
}

#[test]
fn scopes_may_share_a_key_and_the_overrides_of_global_keys_are_listed() {
    let keymap = keymap();
    // The list and the steps pane both bind the arrows; neither takes a
    // global key.
    assert!(keymap.overrides().is_empty(), "{:?}", keymap.overrides());
    assert_eq!(
        found(&keymap, ActiveScopes::pane("list"), press(KeyCode::Up)),
        Some(Do::Up)
    );
    assert_eq!(
        found(&keymap, ActiveScopes::pane("steps"), press(KeyCode::Up)),
        Some(Do::Pane(Step::Previous))
    );
}

#[test]
fn a_controls_table_becomes_bindings_in_its_help_order() {
    let keymap = keymap();
    let steps: Vec<&Binding<Do, Model>> = keymap
        .bindings()
        .iter()
        .filter(|binding| binding.scope() == Scope::Pane("steps"))
        .collect();
    let ids: Vec<&str> = steps.iter().map(|binding| binding.id()).collect();
    // `Never` is bound by no row, so it has no binding.
    assert_eq!(ids, ["steps.next", "steps.previous", "steps.fold"]);
    // The shown key first, then the aliases; the shown row's verb and
    // rank.
    assert_eq!(steps[0].keys(), &[key(KeyCode::Down), ch('j')]);
    assert_eq!(steps[0].verb(), Some("move"));
    assert_eq!(steps[0].rank(), Some(4));
    assert_eq!(steps[1].keys(), &[key(KeyCode::Up), ch('k')]);
    assert_eq!(steps[1].rank(), None);
    assert_eq!(steps[2].verb(), Some("fold"));
}

#[test]
fn a_tables_unavailable_action_cannot_act() {
    let unavailable = crate::keys::with_availability(STEP_KEYS, |step| step != Step::Fold);
    let keymap: Keymap<Do> = Keymap::builder()
        .table(&unavailable, |step| {
            let id = match step {
                Step::Next => "next",
                Step::Previous => "previous",
                Step::Fold => "fold",
                Step::Never => "never",
            };
            bind(id, LIST, &[], Do::Pane(step)).help("Steps", "Step")
        })
        .build()
        .unwrap();
    assert!(
        keymap
            .get("next")
            .is_some_and(|binding| binding.can_act(&()))
    );
    assert!(
        keymap
            .get("fold")
            .is_some_and(|binding| !binding.can_act(&()))
    );
}

#[test]
fn a_binding_can_act_unless_its_predicate_says_otherwise() {
    let keymap = keymap();
    let open = keymap.get("list.open").expect("bound");
    assert!(!open.can_act(&Model { selected: false }));
    assert!(open.can_act(&Model { selected: true }));
    assert!(
        keymap
            .get("quit")
            .expect("bound")
            .can_act(&Model { selected: false })
    );
    // A key resolves whether or not it can act, so the host can say why.
    assert_eq!(
        found(&keymap, ActiveScopes::pane("list"), press(KeyCode::Enter)),
        Some(Do::Open)
    );
}

#[test]
fn a_bindings_first_key_reaches_it_only_where_its_scope_has_the_keys() {
    let keymap = keymap();
    let down = keymap.get("list.down").expect("bound");
    assert!(keymap.first_key_reaches(ActiveScopes::pane("list"), down));
    assert!(!keymap.first_key_reaches(ActiveScopes::pane("steps"), down));
    let quit = keymap.get("quit").expect("bound");
    assert!(keymap.first_key_reaches(ActiveScopes::pane("steps"), quit));
}

#[test]
fn matchers_overlap_where_one_event_fires_both() {
    let any = |c| KeyMatcher::AnyMod(KeyCode::Char(c));
    let unless_ctrl = KeyMatcher::AnyModExceptCtrl;
    let shift_enter = KeyMatcher::Exact {
        modifiers: KeyModifiers::SHIFT,
        code: KeyCode::Enter,
    };
    let cases = [
        (ch('j'), any('j'), Some(KeyModifiers::NONE)),
        (ch('j'), unless_ctrl('j'), Some(KeyModifiers::NONE)),
        (unless_ctrl('g'), ctrl('g'), None),
        (any('g'), ctrl('g'), Some(KeyModifiers::CONTROL)),
        (ch('G'), any('g'), None),
        (key(KeyCode::Enter), shift_enter, Some(KeyModifiers::SHIFT)),
        (exact(KeyCode::Enter), shift_enter, None),
        (
            exact(KeyCode::Enter),
            key(KeyCode::Enter),
            Some(KeyModifiers::NONE),
        ),
        (ctrl('r'), ctrl('r'), Some(KeyModifiers::CONTROL)),
        (ctrl('r'), unless_ctrl('r'), None),
        (key(KeyCode::Tab), key(KeyCode::BackTab), None),
    ];
    for (a, b, expected) in cases {
        let shared = a.overlap(&b).map(|event| event.modifiers);
        assert_eq!(shared, expected, "{a:?} and {b:?}");
        assert_eq!(
            b.overlap(&a).map(|event| event.modifiers),
            expected,
            "{b:?} and {a:?}"
        );
    }
}

#[test]
fn the_scopes_name_themselves_in_a_conflict() {
    assert_eq!(Scope::Global.to_string(), "the global scope");
    assert_eq!(LIST.to_string(), "the `list` pane's scope");
    assert_eq!(
        Scope::Overlay("select").to_string(),
        "the `select` overlay's scope"
    );
    let all = Conflicts(vec![Conflict::NoKeys("a"), Conflict::Undescribed("b")]);
    assert_eq!(
        all.to_string(),
        "`a` binds no key\n`b` has no help section or text"
    );
}
