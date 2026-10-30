//! The explorer over a toy source: a world of things, each with a summary
//! of properties (a zone where a lost cursor does not land), a few more
//! properties shown only with every property, links to other things
//! under a rule (a zone that lands), and an `Other` section of
//! alternatives and links with no far end.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::*;
use crate::columns::Cut;
use crate::keys::{self, KeyRole, ctrl_c_first, loose_characters, mislabeled, resolve, unresolved};
use crate::property_grid::{DisclosureRow, LinkRow, PropertyRow, Tone};
use crate::scroll::ScrollWindow;
use crate::testkit::keys::{chord, press, typed};
use crate::theme::Theme;

/// A thing of the toy world.
#[derive(Clone, Debug, PartialEq)]
struct Thing {
    id: &'static str,
    /// The summary.
    props: Vec<(&'static str, &'static str)>,
    /// Shown only with every property.
    extra: Vec<(&'static str, &'static str)>,
    /// What the thing leads to. A far end starting with `?` is one the
    /// source cannot name.
    links: Vec<&'static str>,
    /// Alternatives, under the `Other` header.
    others: Vec<&'static str>,
    /// Links with no known far end, under the `Other` header.
    stubs: usize,
}

impl Thing {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            props: vec![("name", id)],
            extra: Vec::new(),
            links: Vec::new(),
            others: Vec::new(),
            stubs: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Key {
    Prop(&'static str),
    Link(&'static str),
    Other,
    Alt(&'static str),
    Stub(usize),
}

/// The toy model: things named by `&str`, rows keyed by [`Key`], the
/// host's row a word for its kind, and a number kept with each frame.
struct Toy;

impl ExplorerModel for Toy {
    type Id = &'static str;
    type Key = Key;
    type Row = &'static str;
    type Content = Thing;
    type Saved = u32;
}

/// The toy source.
struct World {
    things: Vec<Thing>,
    /// What `save` keeps with a frame.
    saved: u32,
    /// The key the grid's cut hint names.
    inspect: &'static str,
}

impl World {
    /// The world without `gone`.
    fn without(&self, gone: &[&str]) -> Self {
        Self {
            things: self
                .things
                .iter()
                .filter(|thing| !gone.contains(&thing.id))
                .cloned()
                .collect(),
            saved: self.saved,
            inspect: self.inspect,
        }
    }

    /// The world with `thing` in place of the thing of its id.
    fn with(&self, thing: Thing) -> Self {
        let mut world = self.without(&[thing.id]);
        world.things.push(thing);
        world
    }
}

/// A property row: `-` is a stand-in for no value, and a value past ten
/// cells is cut there.
fn property(label: &'static str, value: &'static str) -> PropertyRow {
    let row = PropertyRow::new(label, value);
    if value == "-" {
        row.placeholder()
    } else if value.len() > 10 {
        row.cut(Cut::End, 10)
    } else {
        row
    }
}

impl ExplorerSource for World {
    type Model = Toy;

    fn load(&self, id: &&'static str) -> Option<Thing> {
        self.things.iter().find(|thing| thing.id == *id).cloned()
    }

    fn id(&self, thing: &Thing) -> &'static str {
        thing.id
    }

    fn zones(&self, thing: &Thing, view: &FrameView<Key>) -> Vec<Zone<Toy>> {
        let extra: &[(&str, &str)] = if view.full { &thing.extra } else { &[] };
        let props = thing
            .props
            .iter()
            .chain(extra)
            .map(|&(label, value)| Entry::new(Key::Prop(label), property(label, value), "prop"))
            .collect();
        let mut links: Vec<Entry<Toy>> = thing
            .links
            .iter()
            .map(|&to| Entry::new(Key::Link(to), LinkRow::new("leads to", to), "link"))
            .collect();
        if !thing.others.is_empty() || thing.stubs > 0 {
            let open = view.is_open(&Key::Other);
            let header = DisclosureRow::new("Other", open).count(thing.others.len() + thing.stubs);
            links.push(Entry::new(Key::Other, header, "header"));
            if open {
                links.extend(thing.others.iter().map(|&to| {
                    let row = LinkRow::new("may lead to", to).indent(4).tone(Tone::Quiet);
                    Entry::new(Key::Alt(to), row, "alt").under(Key::Other)
                }));
                links.extend((0..thing.stubs).map(|n| {
                    let row = LinkRow::new("leads to", "unknown")
                        .indent(4)
                        .tone(Tone::Inert);
                    Entry::new(Key::Stub(n), row, "stub").under(Key::Other)
                }));
            }
        }
        let mut zones = vec![Zone::new(props)];
        if !links.is_empty() {
            zones.push(Zone::new(links).heading(SectionRule::new("Links")).lands());
        }
        zones
    }

    fn follow(&self, _: &Thing, entry: &Entry<Toy>) -> Option<&'static str> {
        match entry.key {
            Key::Link(to) | Key::Alt(to) if !to.starts_with('?') => Some(to),
            _ => None,
        }
    }

    fn tag(&self, thing: &Thing) -> String {
        format!("tag {}", thing.id)
    }

    fn save(&self) -> u32 {
        self.saved
    }

    fn grid(&self) -> PropertyGrid {
        PropertyGrid::new().inspect_key(self.inspect)
    }
}

/// `a`: three summary properties (one with no value, one cut), one more
/// with every property, links to `b`, `c`, and `gone` (not in the world),
/// and `d` and one stub under `Other`. `b` leads back to `a`; `c` leads
/// to a far end the source cannot name; `d` is a leaf; `e` has links and
/// no properties; `f` has properties and no links; `z` has no rows.
fn world() -> World {
    let mut a = Thing::new("a");
    a.props = vec![
        ("name", "alpha"),
        ("owner", "-"),
        ("note", "a long note that runs on"),
    ];
    a.extra = vec![("size", "3")];
    a.links = vec!["b", "c", "gone"];
    a.others = vec!["d"];
    a.stubs = 1;
    let mut b = Thing::new("b");
    b.links = vec!["a"];
    let mut c = Thing::new("c");
    c.links = vec!["?nowhere"];
    let d = Thing::new("d");
    let mut e = Thing::new("e");
    e.props = Vec::new();
    e.links = vec!["a"];
    let mut f = Thing::new("f");
    f.extra = vec![("size", "1")];
    let mut z = Thing::new("z");
    z.props = Vec::new();
    World {
        things: vec![a, b, c, d, e, f, z],
        saved: 7,
        inspect: "o",
    }
}

fn open(id: &'static str, world: &World) -> Explorer<Toy> {
    Explorer::open(&id, world).unwrap_or_else(|| panic!("{id} loads"))
}

fn keys_of(explorer: &Explorer<Toy>, world: &World) -> Vec<Key> {
    explorer
        .entries(world)
        .into_iter()
        .map(|entry| entry.key)
        .collect()
}

/// Put the cursor on the row keyed `key` with the keys, as a reader would.
fn walk_to(explorer: &mut Explorer<Toy>, world: &World, key: &Key) {
    explorer.handle(ExplorerKey::First, world);
    while explorer.cursor_key() != Some(key) {
        let before = explorer.cursor();
        explorer.handle(ExplorerKey::Down, world);
        assert_ne!(explorer.cursor(), before, "no row keyed {key:?}");
    }
}

// ---- rule 1: the cursor --------------------------------------------------

#[test]
fn open_puts_the_cursor_on_the_first_entry_whichever_zone_it_is_in() {
    let world = world();
    let a = open("a", &world);
    assert_eq!(a.cursor(), Some(0));
    assert_eq!(a.cursor_key(), Some(&Key::Prop("name")));
    assert_eq!(a.id(), &"a");
    assert_eq!(a.tag(), "tag a");
    assert_eq!(a.view(), &FrameView::default());
    assert!(a.frames().is_empty());
    assert!(!a.is_armed());
    // A thing with no properties starts on its first link.
    let e = open("e", &world);
    assert_eq!(e.cursor_key(), Some(&Key::Link("a")));
    // A thing the source cannot load opens nothing.
    assert!(Explorer::open(&"gone", &world).is_none());
}

#[test]
fn moves_stop_at_the_ends_and_answer_none() {
    let world = world();
    let mut a = open("a", &world);
    let last = keys_of(&a, &world).len() - 1;
    assert_eq!(a.handle(ExplorerKey::Up, &world), Outcome::None);
    assert_eq!(a.cursor(), Some(0), "Up stops at the first row");
    for _ in 0..=last + 3 {
        assert_eq!(a.handle(ExplorerKey::Down, &world), Outcome::None);
    }
    assert_eq!(a.cursor(), Some(last), "Down stops at the last row");
    assert_eq!(a.handle(ExplorerKey::First, &world), Outcome::None);
    assert_eq!(a.cursor(), Some(0));
    assert_eq!(a.handle(ExplorerKey::Last, &world), Outcome::None);
    assert_eq!(a.cursor(), Some(last));
    assert_eq!(a.cursor_key(), Some(&Key::Other));
}

#[test]
fn a_page_moves_by_the_rows_the_zone_showed_and_one_before_the_first_frame() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(a.handle(ExplorerKey::PageDown, &world), Outcome::None);
    assert_eq!(a.cursor(), Some(1), "no frame drawn yet: one row");
    a.keep_window(ScrollWindow::new(3, 9));
    assert_eq!(a.window(), ScrollWindow::new(3, 9));
    a.handle(ExplorerKey::PageDown, &world);
    assert_eq!(a.cursor(), Some(4));
    a.handle(ExplorerKey::PageDown, &world);
    assert_eq!(a.cursor(), Some(6), "a page stops at the last row");
    assert_eq!(a.handle(ExplorerKey::PageUp, &world), Outcome::None);
    assert_eq!(a.cursor(), Some(3));
    a.handle(ExplorerKey::PageUp, &world);
    assert_eq!(a.cursor(), Some(0), "and at the first");
}

#[test]
fn a_thing_with_no_rows_has_no_cursor_and_its_keys_do_nothing() {
    let world = world();
    let mut z = open("z", &world);
    assert_eq!(z.cursor(), None);
    assert!(z.cursor_entry(&world).is_none());
    for key in [
        ExplorerKey::Down,
        ExplorerKey::Last,
        ExplorerKey::Activate,
        ExplorerKey::Inspect,
        ExplorerKey::ToggleSection,
    ] {
        assert_eq!(z.handle(key, &world), Outcome::None, "{key:?}");
    }
    assert_eq!(z.cursor(), None);
}

// ---- rule 2: reseating after a rebuild -----------------------------------

#[test]
fn a_rebuild_keeps_the_cursor_on_its_key() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    let at = a.cursor();
    a.handle(ExplorerKey::ToggleSection, &world);
    assert_eq!(a.cursor_key(), Some(&Key::Other));
    assert_eq!(a.cursor(), at, "the rows that opened are below the header");
    walk_to(&mut a, &world, &Key::Link("c"));
    a.handle(ExplorerKey::ToggleFull, &world);
    assert_eq!(a.cursor_key(), Some(&Key::Link("c")));
    assert_eq!(
        a.cursor(),
        Some(5),
        "the extra property moved it down a row"
    );
}

#[test]
fn a_cursor_whose_row_is_gone_lands_on_the_first_zone_that_lands_not_on_a_property() {
    let world = world();
    let mut a = open("a", &world);
    a.handle(ExplorerKey::ToggleFull, &world);
    walk_to(&mut a, &world, &Key::Prop("size"));
    a.handle(ExplorerKey::ToggleFull, &world);
    assert_eq!(
        a.cursor_key(),
        Some(&Key::Link("b")),
        "the summary zone does not land; the links do"
    );
    // A row under a section that closes lands on the zone's first row too.
    a.handle(ExplorerKey::Last, &world);
    a.handle(ExplorerKey::ToggleSection, &world);
    walk_to(&mut a, &world, &Key::Stub(0));
    let world = world.with({
        let mut a = world.load(&"a").expect("a");
        a.stubs = 0;
        a
    });
    assert_eq!(a.refresh(&world), Refreshed::Kept);
    assert_eq!(a.cursor_key(), Some(&Key::Link("b")));
}

#[test]
fn with_no_zone_that_lands_a_lost_cursor_goes_to_the_first_entry() {
    let world = world();
    let mut f = open("f", &world);
    f.handle(ExplorerKey::ToggleFull, &world);
    walk_to(&mut f, &world, &Key::Prop("size"));
    f.handle(ExplorerKey::ToggleFull, &world);
    assert_eq!(f.cursor(), Some(0));
    assert_eq!(f.cursor_key(), Some(&Key::Prop("name")));
}

// ---- rule 3: sections ----------------------------------------------------

#[test]
fn toggle_section_folds_its_header_and_is_refused_elsewhere() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(
        a.handle(ExplorerKey::ToggleSection, &world),
        Outcome::Refused(Refusal::NotInSection),
        "a property is in no section"
    );
    walk_to(&mut a, &world, &Key::Other);
    assert_eq!(
        a.handle(ExplorerKey::ToggleSection, &world),
        Outcome::Toggle { open: true }
    );
    assert!(a.view().is_open(&Key::Other));
    assert!(keys_of(&a, &world).contains(&Key::Alt("d")));
    a.handle(ExplorerKey::Down, &world);
    assert_eq!(
        a.handle(ExplorerKey::ToggleSection, &world),
        Outcome::Refused(Refusal::NotInSection),
        "the fold key folds on the header"
    );
    walk_to(&mut a, &world, &Key::Other);
    assert_eq!(
        a.handle(ExplorerKey::ToggleSection, &world),
        Outcome::Toggle { open: false }
    );
    assert!(!keys_of(&a, &world).contains(&Key::Alt("d")));
}

#[test]
fn open_section_opens_a_closed_header_and_does_nothing_on_an_open_one() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    assert_eq!(
        a.handle(ExplorerKey::OpenSection, &world),
        Outcome::Toggle { open: true }
    );
    assert_eq!(a.handle(ExplorerKey::OpenSection, &world), Outcome::None);
    a.handle(ExplorerKey::Down, &world);
    assert_eq!(
        a.handle(ExplorerKey::OpenSection, &world),
        Outcome::None,
        "a row under an open section is in it already"
    );
    assert!(a.view().is_open(&Key::Other));
}

#[test]
fn close_section_closes_from_its_header_or_a_row_under_it_and_lands_on_the_header() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    walk_to(&mut a, &world, &Key::Stub(0));
    assert_eq!(
        a.handle(ExplorerKey::CloseSection, &world),
        Outcome::Toggle { open: false }
    );
    assert_eq!(a.cursor_key(), Some(&Key::Other));
    assert!(!a.view().is_open(&Key::Other));
    assert_eq!(
        a.handle(ExplorerKey::CloseSection, &world),
        Outcome::None,
        "a closed header has nothing to close"
    );
    a.handle(ExplorerKey::OpenSection, &world);
    assert_eq!(
        a.handle(ExplorerKey::CloseSection, &world),
        Outcome::Toggle { open: false }
    );
    assert_eq!(a.cursor_key(), Some(&Key::Other));
}

#[test]
fn a_fold_key_away_from_a_section_is_refused() {
    let world = world();
    let mut a = open("a", &world);
    for row in [Key::Prop("name"), Key::Link("b")] {
        walk_to(&mut a, &world, &row);
        for key in [
            ExplorerKey::ToggleSection,
            ExplorerKey::OpenSection,
            ExplorerKey::CloseSection,
        ] {
            assert_eq!(
                a.handle(key, &world),
                Outcome::Refused(Refusal::NotInSection),
                "{key:?} on {row:?}"
            );
            assert_eq!(a.cursor_key(), Some(&row), "the cursor stays");
        }
    }
    assert!(a.view().open.is_empty());
}

// ---- rule 4: every property ----------------------------------------------

#[test]
fn toggle_full_lists_every_property_and_back_again() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(a.handle(ExplorerKey::ToggleFull, &world), Outcome::None);
    assert!(a.view().full);
    assert!(keys_of(&a, &world).contains(&Key::Prop("size")));
    assert_eq!(a.handle(ExplorerKey::ToggleFull, &world), Outcome::None);
    assert!(!a.view().full);
    assert!(!keys_of(&a, &world).contains(&Key::Prop("size")));
}

#[test]
fn toggle_full_with_nothing_more_is_refused_and_stays_flipped() {
    let world = world();
    let mut b = open("b", &world);
    assert_eq!(
        b.handle(ExplorerKey::ToggleFull, &world),
        Outcome::Refused(Refusal::NothingMore)
    );
    assert!(b.view().full, "the switch stays flipped");
    assert_eq!(b.cursor(), Some(0));
}

// ---- rule 5: drilling ------------------------------------------------------

#[test]
fn enter_on_a_link_drills_and_pushes_a_frame() {
    let world = world();
    let mut a = open("a", &world);
    a.handle(ExplorerKey::ToggleFull, &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    walk_to(&mut a, &world, &Key::Link("b"));
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Drill { to: "b" }
    );
    assert_eq!(a.id(), &"b");
    assert_eq!(a.tag(), "tag b");
    assert_eq!(
        a.frames(),
        &[Frame {
            id: "a",
            tag: "tag a".to_string(),
            view: FrameView {
                open: vec![Key::Other],
                full: true,
            },
            cursor: Some(Key::Link("b")),
            saved: 7,
        }]
    );
    assert_eq!(a.view(), &FrameView::default(), "a drill starts afresh");
    assert_eq!(a.cursor(), Some(0), "on the first row");
}

#[test]
fn a_link_whose_far_end_does_not_load_answers_follow_and_changes_nothing() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Link("gone"));
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Follow { to: "gone" }
    );
    assert_eq!(a.id(), &"a");
    assert!(a.frames().is_empty());
    assert_eq!(a.cursor_key(), Some(&Key::Link("gone")));
}

#[test]
fn a_link_with_no_far_end_is_refused() {
    let world = world();
    // An inert link: its row asks for nothing.
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    walk_to(&mut a, &world, &Key::Stub(0));
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Refused(Refusal::NoFarEnd)
    );
    // A link whose far end the source cannot name.
    let mut c = open("c", &world);
    walk_to(&mut c, &world, &Key::Link("?nowhere"));
    assert_eq!(
        c.handle(ExplorerKey::Activate, &world),
        Outcome::Refused(Refusal::NoFarEnd)
    );
    assert!(a.frames().is_empty() && c.frames().is_empty());
}

#[test]
fn drill_to_is_the_same_move_by_id() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(a.drill_to("d", &world), Outcome::Drill { to: "d" });
    assert_eq!(a.frames().len(), 1);
    assert_eq!(a.frames()[0].cursor, Some(Key::Prop("name")));
    assert_eq!(a.drill_to("gone", &world), Outcome::Follow { to: "gone" });
    assert_eq!(a.id(), &"d");
    assert_eq!(a.frames().len(), 1);
}

#[test]
fn an_alternative_drills_like_a_link() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::Activate, &world);
    walk_to(&mut a, &world, &Key::Alt("d"));
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Drill { to: "d" }
    );
}

// ---- rule 6: back --------------------------------------------------------

#[test]
fn back_restores_the_frames_view_and_cursor_and_answers_its_saved_value() {
    let mut world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    walk_to(&mut a, &world, &Key::Alt("d"));
    a.handle(ExplorerKey::Activate, &world);
    world.saved = 8;
    walk_to(&mut a, &world, &Key::Prop("name"));
    a.handle(ExplorerKey::ToggleFull, &world);
    assert_eq!(
        a.handle(ExplorerKey::Back, &world),
        Outcome::Back { to: "a", saved: 7 }
    );
    assert_eq!(a.id(), &"a");
    assert!(a.frames().is_empty());
    assert!(a.view().is_open(&Key::Other));
    assert!(
        !a.view().full,
        "the frame's own switch, not the drilled one's"
    );
    assert_eq!(a.cursor_key(), Some(&Key::Alt("d")));
}

#[test]
fn back_skips_a_frame_whose_thing_is_gone_in_the_same_press() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Link("b"));
    a.handle(ExplorerKey::Activate, &world);
    a.handle(ExplorerKey::Down, &world);
    a.handle(ExplorerKey::Activate, &world);
    assert_eq!(a.id(), &"a");
    assert_eq!(a.frames().len(), 2);
    let without_b = world.without(&["b"]);
    assert_eq!(
        a.handle(ExplorerKey::Back, &without_b),
        Outcome::Back { to: "a", saved: 7 },
        "b is gone, so the press goes on to the frame before it"
    );
    assert!(a.frames().is_empty());
    assert_eq!(a.cursor_key(), Some(&Key::Link("b")));
}

#[test]
fn back_with_every_frame_gone_is_refused_and_keeps_what_is_shown() {
    let world = world();
    let mut a = open("a", &world);
    a.drill_to("b", &world);
    a.drill_to("d", &world);
    let without = world.without(&["a", "b"]);
    assert_eq!(
        a.handle(ExplorerKey::Back, &without),
        Outcome::Refused(Refusal::Gone)
    );
    assert_eq!(a.id(), &"d");
    assert!(
        a.frames().is_empty(),
        "the frames that are gone are dropped"
    );
    assert_eq!(a.handle(ExplorerKey::Back, &without), Outcome::Armed);
}

#[test]
fn back_drops_a_section_that_is_gone_from_the_frame() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    a.drill_to("d", &world);
    let world = world.with({
        let mut a = world.load(&"a").expect("a");
        a.others.clear();
        a.stubs = 0;
        a
    });
    a.handle(ExplorerKey::Back, &world);
    assert!(a.view().open.is_empty());
    assert_eq!(
        a.cursor_key(),
        Some(&Key::Link("b")),
        "the header is gone; the cursor lands on the links"
    );
}

// ---- rule 7: the armed back ------------------------------------------------

#[test]
fn back_with_no_frames_arms_then_leaves() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Armed);
    assert!(a.is_armed());
    assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Leave);
    assert!(!a.is_armed());
    assert_eq!(
        a.handle(ExplorerKey::Back, &world),
        Outcome::Armed,
        "afresh"
    );
}

#[test]
fn every_other_key_disarms() {
    let world = world();
    let mut a = open("a", &world);
    for key in [
        ExplorerKey::Down,
        ExplorerKey::Up,
        ExplorerKey::PageDown,
        ExplorerKey::First,
        ExplorerKey::Last,
        ExplorerKey::Activate,
        ExplorerKey::Inspect,
        ExplorerKey::ToggleSection,
        ExplorerKey::OpenSection,
        ExplorerKey::CloseSection,
        ExplorerKey::ToggleFull,
    ] {
        assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Armed);
        a.handle(key, &world);
        assert!(!a.is_armed(), "{key:?} disarms");
        a.handle(ExplorerKey::First, &world);
    }
}

#[test]
fn the_hosts_disarm_forgets_the_arm() {
    let world = world();
    let mut a = open("a", &world);
    a.handle(ExplorerKey::Back, &world);
    a.disarm();
    assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Armed);
}

#[test]
fn back_after_a_drill_unwinds_then_arms_then_leaves() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Link("b"));
    a.handle(ExplorerKey::Activate, &world);
    assert_eq!(
        a.handle(ExplorerKey::Back, &world),
        Outcome::Back { to: "a", saved: 7 }
    );
    assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Armed);
    assert_eq!(a.handle(ExplorerKey::Back, &world), Outcome::Leave);
}

// ---- rule 8: refresh -----------------------------------------------------

#[test]
fn refresh_keeps_the_frames_the_view_and_the_cursor() {
    let world = world();
    let mut a = open("a", &world);
    a.drill_to("b", &world);
    a.drill_to("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    a.handle(ExplorerKey::ToggleFull, &world);
    walk_to(&mut a, &world, &Key::Alt("d"));
    let frames = a.frames().to_vec();
    let view = a.view().clone();
    assert_eq!(a.refresh(&world), Refreshed::Kept);
    assert_eq!(a.frames(), frames.as_slice());
    assert_eq!(a.view(), &view);
    assert_eq!(a.cursor_key(), Some(&Key::Alt("d")));
}

#[test]
fn refresh_puts_the_cursor_back_by_its_key_and_reseats_a_lost_one() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Link("gone"));
    // `b` goes from the links: the cursor keeps its row, which moved up.
    let fewer = world.with({
        let mut a = world.load(&"a").expect("a");
        a.links = vec!["c", "gone"];
        a
    });
    assert_eq!(a.refresh(&fewer), Refreshed::Kept);
    assert_eq!(a.cursor_key(), Some(&Key::Link("gone")));
    assert_eq!(a.cursor(), Some(4));
    // Its own row goes: it lands on the first link.
    let none = world.with({
        let mut a = world.load(&"a").expect("a");
        a.links = vec!["c"];
        a
    });
    a.refresh(&none);
    assert_eq!(a.cursor_key(), Some(&Key::Link("c")));
}

#[test]
fn refresh_drops_a_section_that_is_gone() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    a.handle(ExplorerKey::OpenSection, &world);
    let world = world.with({
        let mut a = world.load(&"a").expect("a");
        a.others.clear();
        a.stubs = 0;
        a
    });
    a.refresh(&world);
    assert!(a.view().open.is_empty());
    assert_eq!(a.cursor_key(), Some(&Key::Link("b")));
}

#[test]
fn refresh_of_a_thing_that_is_gone_answers_gone_and_changes_nothing() {
    let world = world();
    let mut a = open("a", &world);
    a.drill_to("b", &world);
    let gone = world.without(&["b"]);
    assert_eq!(a.refresh(&gone), Refreshed::Gone);
    assert_eq!(a.id(), &"b");
    assert_eq!(a.frames().len(), 1);
}

#[test]
fn refresh_does_not_load_the_frames() {
    let world = world();
    let mut a = open("a", &world);
    a.drill_to("b", &world);
    let without_a = world.without(&["a"]);
    assert_eq!(
        a.refresh(&without_a),
        Refreshed::Kept,
        "a frame is loaded only when Back reaches it"
    );
    assert_eq!(a.frames().len(), 1);
}

// ---- rule 9: the window --------------------------------------------------

#[test]
fn the_window_is_kept_until_the_host_keeps_another_through_drills_and_backs() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(a.window(), ScrollWindow::default());
    let drawn = ScrollWindow::new(4, 12).at(2);
    a.keep_window(drawn);
    a.drill_to("b", &world);
    assert_eq!(a.window(), drawn);
    a.handle(ExplorerKey::Back, &world);
    assert_eq!(a.window(), drawn);
}

// ---- rule 10: drawing ----------------------------------------------------

#[test]
fn lines_draw_each_heading_as_a_line_that_is_no_row() {
    let world = world();
    let theme = Theme::default();
    let mut a = open("a", &world);
    let drawn = a.lines(&world, 40, true, &theme);
    let text: Vec<String> = drawn.lines.iter().map(ToString::to_string).collect();
    assert_eq!(
        text.len(),
        8,
        "three properties, the rule, three links, Other"
    );
    assert!(text[3].contains("Links"), "{text:?}");
    assert_eq!(drawn.cursor_line, Some(0));
    walk_to(&mut a, &world, &Key::Link("b"));
    assert_eq!(a.lines(&world, 40, true, &theme).cursor_line, Some(4));
    assert!(
        text[2].contains("(truncated · o)"),
        "the source's grid: {text:?}"
    );
}

#[test]
fn lines_draw_the_cursor_in_the_inactive_look_without_focus() {
    let world = world();
    let theme = Theme::default();
    let a = open("a", &world);
    let focused = a.lines(&world, 40, true, &theme);
    let unfocused = a.lines(&world, 40, false, &theme);
    assert_ne!(focused.lines[0], unfocused.lines[0], "the cursor's row");
    assert_eq!(focused.lines[1..], unfocused.lines[1..], "every other row");
    assert_eq!(focused.cursor_line, unfocused.cursor_line);
}

// ---- the outcomes of table 3.2 that the rules above do not name ----------

#[test]
fn enter_on_a_property_with_a_value_asks_for_a_copy_of_the_whole_value() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Copy {
            label: "name".to_string(),
            text: "alpha".to_string(),
        }
    );
    walk_to(&mut a, &world, &Key::Prop("note"));
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Copy {
            label: "note".to_string(),
            text: "a long note that runs on".to_string(),
        },
        "the whole value, though the row draws it cut"
    );
}

#[test]
fn enter_on_a_property_with_no_value_does_nothing() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Prop("owner"));
    assert_eq!(a.handle(ExplorerKey::Activate, &world), Outcome::None);
}

#[test]
fn enter_on_a_section_header_folds_it() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Other);
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Toggle { open: true }
    );
    assert_eq!(
        a.handle(ExplorerKey::Activate, &world),
        Outcome::Toggle { open: false }
    );
    assert_eq!(a.cursor_key(), Some(&Key::Other));
}

#[test]
fn inspect_reads_a_cut_property_whole_and_is_refused_elsewhere() {
    let world = world();
    let mut a = open("a", &world);
    assert_eq!(
        a.handle(ExplorerKey::Inspect, &world),
        Outcome::Refused(Refusal::NothingToInspect),
        "nothing is cut"
    );
    walk_to(&mut a, &world, &Key::Prop("note"));
    assert_eq!(
        a.handle(ExplorerKey::Inspect, &world),
        Outcome::Inspect {
            label: "note".to_string(),
            text: "a long note that runs on".to_string(),
        }
    );
    for row in [Key::Link("b"), Key::Other] {
        walk_to(&mut a, &world, &row);
        assert_eq!(
            a.handle(ExplorerKey::Inspect, &world),
            Outcome::Refused(Refusal::NothingToInspect),
            "{row:?}"
        );
    }
}

// ---- seeking -------------------------------------------------------------

#[test]
fn seek_puts_the_cursor_on_the_first_entry_it_finds() {
    let world = world();
    let mut a = open("a", &world);
    assert!(a.seek(&world, |_, entry| entry.key == Key::Link("c")));
    assert_eq!(a.cursor_key(), Some(&Key::Link("c")));
    assert!(
        a.seek(&world, |thing, entry| thing.id == "a"
            && entry.host == "link"),
        "it sees what is shown and each entry"
    );
    assert_eq!(a.cursor_key(), Some(&Key::Link("b")));
}

#[test]
fn seek_opens_a_closed_section_that_holds_what_it_looks_for() {
    let world = world();
    let mut a = open("a", &world);
    assert!(a.seek(&world, |_, entry| entry.key == Key::Alt("d")));
    assert!(a.view().is_open(&Key::Other));
    assert_eq!(a.cursor_key(), Some(&Key::Alt("d")));
}

#[test]
fn seek_finding_nothing_leaves_the_cursor_and_the_sections_as_they_were() {
    let world = world();
    let mut a = open("a", &world);
    walk_to(&mut a, &world, &Key::Link("b"));
    assert!(!a.seek(&world, |_, entry| entry.key == Key::Link("nothing")));
    assert!(a.view().open.is_empty());
    assert_eq!(a.cursor_key(), Some(&Key::Link("b")));
}

// ---- the footer ----------------------------------------------------------

#[test]
fn the_footer_names_moving_enter_with_its_verb_on_the_row_folding_and_going_back() {
    let world = world();
    let mut a = open("a", &world);
    let footer = |a: &Explorer<Toy>| a.footer(EXPLORER_KEYS, &world);
    assert_eq!(
        footer(&a).text(),
        "↑/↓ move · Enter copy · e fold · Backspace back"
    );
    for (row, verb) in [(Key::Link("b"), "follow"), (Key::Other, "fold")] {
        walk_to(&mut a, &world, &row);
        assert_eq!(
            footer(&a).text(),
            format!("↑/↓ move · Enter {verb} · e fold · Backspace back")
        );
        assert!(
            footer(&a)
                .segments()
                .iter()
                .all(|segment| segment.available)
        );
    }
    // Where Enter does nothing, it keeps the table's verb, unavailable.
    walk_to(&mut a, &world, &Key::Prop("owner"));
    let footer = footer(&a);
    assert_eq!(
        footer.text(),
        "↑/↓ move · Enter open · e fold · Backspace back"
    );
    assert!(!footer.segments()[1].available);
    assert!(footer.segments()[0].available);
}

#[test]
fn the_hint_keeps_moving_enter_folding_and_going_back_with_the_tables_verbs() {
    // A host table that includes the explorer's hints the same keys.
    const HOST: &[keys::KeyRow<ExplorerKey>] = &[keys::include(EXPLORER_KEYS)];
    assert_eq!(
        hint(EXPLORER_KEYS).text(),
        "↑/↓ move · Enter open · e fold · Backspace back"
    );
    assert_eq!(hint(HOST), hint(EXPLORER_KEYS));
}

// ---- the table -----------------------------------------------------------

#[test]
fn the_table_resolves_each_key_to_its_meaning() {
    use ExplorerKey as K;
    let none = KeyModifiers::NONE;
    for (event, action) in [
        (press(KeyCode::Up), K::Up),
        (typed('k'), K::Up),
        (press(KeyCode::Down), K::Down),
        (typed('j'), K::Down),
        (press(KeyCode::PageUp), K::PageUp),
        (press(KeyCode::PageDown), K::PageDown),
        (press(KeyCode::Home), K::First),
        (typed('g'), K::First),
        (press(KeyCode::End), K::Last),
        (typed('G'), K::Last),
        (press(KeyCode::Enter), K::Activate),
        (typed('e'), K::ToggleSection),
        (typed('l'), K::OpenSection),
        (press(KeyCode::Right), K::OpenSection),
        (typed('h'), K::CloseSection),
        (press(KeyCode::Left), K::CloseSection),
        (typed('o'), K::Inspect),
        (typed('F'), K::ToggleFull),
        (press(KeyCode::Backspace), K::Back),
        (press(KeyCode::Esc), K::Back),
        (chord(none, KeyCode::Char('G')), K::Last),
    ] {
        assert_eq!(resolve(EXPLORER_KEYS, event), Some(action), "{event:?}");
    }
}

#[test]
fn enter_binds_with_no_modifier_so_its_chords_reach_the_host() {
    for modifiers in [
        KeyModifiers::SHIFT,
        KeyModifiers::ALT,
        KeyModifiers::CONTROL,
    ] {
        assert_eq!(
            resolve(EXPLORER_KEYS, chord(modifiers, KeyCode::Enter)),
            None,
            "{modifiers:?}"
        );
    }
}

#[test]
fn characters_bind_as_typed_and_never_on_their_chords() {
    for modifiers in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
        for c in ['j', 'k', 'g', 'G', 'e', 'h', 'l', 'o', 'F'] {
            assert_eq!(
                resolve(EXPLORER_KEYS, chord(modifiers, KeyCode::Char(c))),
                None,
                "{modifiers:?} {c}"
            );
        }
    }
    assert!(loose_characters(EXPLORER_KEYS).is_empty());
}

#[test]
fn the_table_passes_the_checks_a_pane_table_takes() {
    assert!(mislabeled(EXPLORER_KEYS).is_empty());
    assert!(unresolved(EXPLORER_KEYS).is_empty());
    assert!(loose_characters(EXPLORER_KEYS).is_empty());
    // A pane leaves its program's quit keys alone.
    assert!(!ctrl_c_first(EXPLORER_KEYS));
    assert_eq!(resolve(EXPLORER_KEYS, typed('q')), None);
    assert_eq!(
        resolve(
            EXPLORER_KEYS,
            chord(KeyModifiers::CONTROL, KeyCode::Char('c'))
        ),
        None
    );
    assert_eq!(
        keys::footer(EXPLORER_KEYS),
        "↑/↓ move · Enter open · e fold · o inspect · F show all · Backspace back"
    );
}

#[test]
fn every_key_says_what_it_is_for_and_what_it_does() {
    use crate::keys::KeyAction;
    use ExplorerKey as K;
    assert!(crate::help::undescribed(EXPLORER_KEYS).is_empty());
    for (key, role) in [
        (K::Down, KeyRole::Move),
        (K::First, KeyRole::Move),
        (K::PageDown, KeyRole::Page),
        (K::Activate, KeyRole::Apply),
        (K::ToggleSection, KeyRole::Change),
        (K::CloseSection, KeyRole::Change),
        (K::ToggleFull, KeyRole::Change),
        (K::Back, KeyRole::Leave),
        (K::Inspect, KeyRole::Other),
    ] {
        assert_eq!(key.role(), role, "{key:?}");
    }
    let sections = crate::help::sections_for(EXPLORER_KEYS);
    let titles: Vec<&str> = sections.iter().map(|s| s.title.as_ref()).collect();
    assert_eq!(titles, ["Moving", "Rows", "Sections", "Going back"]);
}
