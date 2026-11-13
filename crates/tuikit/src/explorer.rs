//! The explorer: a cursor over zones of rows its host supplies, a drill
//! stack that follows a link to the thing at its far end and back, and
//! one key table.
//!
//! This is a layer 1 control (house rules 1, 2, 4, 5, 7, 8, 10, 12, 13,
//! and 14). A pane that shows one thing, its properties above the links
//! that lead from it to other things, lets its reader move a cursor over
//! the rows, fold a section of them, show every property instead of a
//! summary, follow a link and come back. The explorer is that control
//! without the host's data:
//!
//! - **The host's data.** [`ExplorerModel`] names the kinds of value the
//!   explorer keeps: the `Id` of a thing it can show, the `Key` of a row
//!   (stable across rebuilds), the host's own `Row`, the `Content` shown
//!   for one thing, and what the host keeps (`Saved`) with a frame it
//!   drills away from. An [`ExplorerSource`] is built from the host's
//!   data for each call: it loads a thing, lays its rows out as [`Zone`]s
//!   of [`Entry`]s for a [`FrameView`], says where a link leads, tags a
//!   thing for a breadcrumb, and says what to keep with a frame. The
//!   explorer holds owned values only, so it lives in the host's model,
//!   and nothing it is handed is cached: the rows are asked for on every
//!   key and every frame.
//! - **The cursor** is an index into the entries of every zone in order
//!   and the key of the row there. It starts on the first entry, moves
//!   and stops at the ends (house rule 2), pages by the rows the zone
//!   showed in the last frame (house rule 12), and after every rebuild
//!   goes back to the row with its key, else to the first row of the
//!   first zone that [`lands`](Zone::lands), else to the first row.
//! - **Sections.** A section is a header row ([`Row::Disclosure`]) and the
//!   rows whose [`under`](Entry::under) names its key. The view lists the
//!   open ones; the fold keys open and close them.
//! - **Drilling.** `Enter` on a link asks the source where it leads and
//!   shows that thing, pushing a [`Frame`] that keeps what was shown, how,
//!   where its cursor stood, and what the host saved. `Back` returns to
//!   the newest frame that still loads; with no frame left it arms, and a
//!   second `Back` leaves the explorer to its host.
//! - **Keys answer with an [`Outcome`].** [`Explorer::handle`] takes an
//!   [`ExplorerKey`], does the explorer's part (moved, folded, drilled,
//!   gone back), and tells the host what is left for it: copy a value,
//!   open one in a reader, follow up a drill, leave, or word a refusal
//!   ([`Refusal`]). The host does every effect and writes every word.
//! - **The table.** [`EXPLORER_KEYS`] binds the keys once; a host
//!   resolves it before its own keymap while the explorer has focus and
//!   generates its help and its hint from it ([`Explorer::footer`]).
//! - **Drawing.** [`Explorer::lines`] lays every zone out in the source's
//!   property grid: each heading as a line that is no row, then the
//!   rows, the cursor in the active or the inactive look (house rules 1
//!   and 10). The host places the lines, in a
//!   [`StackedMasterDetail`](crate::master_detail::StackedMasterDetail)
//!   say, and hands back the window they drew in
//!   ([`Explorer::keep_window`]).
//!
//! ```
//! use std::collections::BTreeMap;
//!
//! use ratatui::crossterm::event::KeyCode;
//! use tuikit::explorer::{
//!     EXPLORER_KEYS, Entry, Explorer, ExplorerKey, ExplorerModel, ExplorerSource, FrameView,
//!     Outcome, Zone,
//! };
//! use tuikit::frame::SectionRule;
//! use tuikit::keys::resolve;
//! use tuikit::property_grid::{LinkRow, PropertyRow};
//! use tuikit::testkit::keys::press;
//! use tuikit::theme::Theme;
//!
//! /// A catalog of items, each naming the items it refers to.
//! struct Catalog;
//!
//! impl ExplorerModel for Catalog {
//!     type Id = String;
//!     type Key = String;
//!     type Row = ();
//!     type Content = (String, Vec<String>);
//!     type Saved = ();
//! }
//!
//! struct Items(BTreeMap<String, Vec<String>>);
//!
//! impl ExplorerSource for Items {
//!     type Model = Catalog;
//!
//!     fn load(&self, id: &String) -> Option<(String, Vec<String>)> {
//!         self.0.get(id).map(|refs| (id.clone(), refs.clone()))
//!     }
//!
//!     fn id(&self, item: &(String, Vec<String>)) -> String {
//!         item.0.clone()
//!     }
//!
//!     fn zones(&self, item: &(String, Vec<String>), _: &FrameView<String>) -> Vec<Zone<Catalog>> {
//!         let name = Entry::new("name".to_string(), PropertyRow::new("name", item.0.clone()), ());
//!         let links = item
//!             .1
//!             .iter()
//!             .map(|to| Entry::new(format!("to {to}"), LinkRow::new("refers to", to.clone()), ()))
//!             .collect();
//!         vec![
//!             Zone::new(vec![name]),
//!             Zone::new(links).heading(SectionRule::new("Links")).lands(),
//!         ]
//!     }
//!
//!     fn follow(&self, _: &(String, Vec<String>), entry: &Entry<Catalog>) -> Option<String> {
//!         entry.key.strip_prefix("to ").map(str::to_string)
//!     }
//!
//!     fn tag(&self, item: &(String, Vec<String>)) -> String {
//!         item.0.clone()
//!     }
//!
//!     fn save(&self) {}
//! }
//!
//! let items = Items(BTreeMap::from([
//!     ("a".to_string(), vec!["b".to_string()]),
//!     ("b".to_string(), Vec::new()),
//! ]));
//! let mut explorer = Explorer::open(&"a".to_string(), &items).expect("a loads");
//! assert_eq!(explorer.cursor(), Some(0), "the cursor starts on the first row");
//!
//! // The host resolves a key through the table and does what the outcome
//! // asks: here, nothing after a move, and its own follow-up of a drill.
//! let key = |code| resolve(EXPLORER_KEYS, press(code)).expect("bound");
//! assert_eq!(explorer.handle(key(KeyCode::Down), &items), Outcome::None);
//! let drill = explorer.handle(key(KeyCode::Enter), &items);
//! assert_eq!(drill, Outcome::Drill { to: "b".to_string() });
//! assert_eq!(explorer.frames().len(), 1);
//!
//! // Back returns to the link it left from; with no frame left it arms,
//! // and a second press leaves.
//! let back = explorer.handle(key(KeyCode::Backspace), &items);
//! assert_eq!(back, Outcome::Back { to: "a".to_string(), saved: () });
//! assert_eq!(explorer.cursor(), Some(1));
//! assert_eq!(explorer.handle(ExplorerKey::Back, &items), Outcome::Armed);
//! assert_eq!(explorer.handle(ExplorerKey::Back, &items), Outcome::Leave);
//!
//! // A property, the rule over the links, and a link.
//! let drawn = explorer.lines(&items, 40, true, &Theme::default());
//! assert_eq!(drawn.lines.len(), 3);
//! assert_eq!(drawn.cursor_line, Some(2));
//! ```
//!
//! # What a host keeps
//!
//! What its things are and how each is built, the rows' keys and their
//! content, the verbs and looks of its rows (through the property grid's
//! rows), the words of every hint and refusal, the effects (the
//! clipboard, the reader a value opens in, what a drill changes outside
//! the explorer, where focus goes when it leaves), and what it keeps with
//! a frame. It decides when its data changed: [`Explorer::refresh`] when
//! the thing it asked for is still the one shown, [`Explorer::open`] when
//! it asks for another.

use std::fmt;

use crate::frame::SectionRule;
use crate::property_grid::{PropertyGrid, Row};

mod keys;
mod outcome;
mod state;

pub use keys::{EXPLORER_KEYS, ExplorerKey, hint};
pub use outcome::{Outcome, Refreshed, Refusal};
pub use state::Explorer;

/// The kinds of value an explorer keeps, named once by the host.
///
/// The host's model is usually a unit struct that only names these; the
/// explorer's types implement `Clone`, `Debug`, and `PartialEq` over the
/// associated types, so the model needs no derives of its own.
pub trait ExplorerModel {
    /// A thing the explorer can show. A drill frame keeps one.
    type Id: Clone + PartialEq + fmt::Debug;
    /// A row's identity across rebuilds: what the cursor keeps, what a
    /// frame restores, and how an open section is named (by its header's
    /// key). Two rows of one content never share a key.
    type Key: Clone + PartialEq + fmt::Debug;
    /// The host's own row, handed back with the cursor's entry, so the
    /// host can read what it built (a property's typed value, a link's
    /// far end) without looking the row up again.
    type Row: Clone + fmt::Debug;
    /// What the explorer shows for one thing, built by the source.
    type Content;
    /// What the host keeps with a frame it drills away from, handed back
    /// when `Back` returns to it.
    type Saved: Clone + PartialEq + fmt::Debug;
}

/// What the host supplies. A source is built for each call from the
/// host's data; the explorer keeps nothing it borrows.
pub trait ExplorerSource {
    /// The model whose values the source deals in.
    type Model: ExplorerModel;

    /// The content for `id`, or `None` when the data no longer has it.
    fn load(&self, id: &IdOf<Self>) -> Option<ContentOf<Self>>;

    /// The identity `content` shows.
    fn id(&self, content: &ContentOf<Self>) -> IdOf<Self>;

    /// The rows of `content`, zone by zone, as `view` asks: a closed
    /// section lists its header and none of its rows, and `view.full`
    /// lists every property instead of the summary. Called for every key
    /// and every frame; nothing is cached.
    fn zones(
        &self,
        content: &ContentOf<Self>,
        view: &FrameView<KeyOf<Self>>,
    ) -> Vec<Zone<Self::Model>>;

    /// Where `entry`, a link row, leads; `None` for a link with no far
    /// end the host can name.
    fn follow(&self, content: &ContentOf<Self>, entry: &Entry<Self::Model>) -> Option<IdOf<Self>>;

    /// The tag a breadcrumb gives `content`. The frame that leaves it
    /// keeps the tag.
    fn tag(&self, content: &ContentOf<Self>) -> String;

    /// What the host keeps with the frame being drilled away from.
    fn save(&self) -> SavedOf<Self>;

    /// An empty grid laid out the host's way (its label and verb
    /// columns, the inspect key its cut hint names, its tones). The
    /// explorer draws its rows in it.
    fn grid(&self) -> PropertyGrid {
        PropertyGrid::new()
    }
}

/// The `Id` of a source's model.
pub type IdOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Id;
/// The `Key` of a source's model.
pub type KeyOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Key;
/// The `Content` of a source's model.
pub type ContentOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Content;
/// The `Saved` of a source's model.
pub type SavedOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Saved;

/// A run of rows under an optional rule.
pub struct Zone<M: ExplorerModel> {
    /// A rule over the zone (a label and a summary) that the cursor never
    /// visits.
    pub heading: Option<SectionRule>,
    /// Whether a cursor whose row is gone after a rebuild may land on this
    /// zone's first row. A summary of properties usually does not, so a
    /// cursor that stood among the links stays among them.
    pub lands: bool,
    /// The rows, top to bottom.
    pub entries: Vec<Entry<M>>,
}

impl<M: ExplorerModel> Zone<M> {
    /// A zone of `entries` with no heading, where a lost cursor does not
    /// land.
    pub fn new(entries: Vec<Entry<M>>) -> Self {
        Self {
            heading: None,
            lands: false,
            entries,
        }
    }

    /// This zone under `rule`.
    #[must_use]
    pub fn heading(mut self, rule: SectionRule) -> Self {
        self.heading = Some(rule);
        self
    }

    /// This zone as one a lost cursor lands on ([`Zone::lands`]).
    #[must_use]
    pub fn lands(mut self) -> Self {
        self.lands = true;
        self
    }
}

impl<M: ExplorerModel> Clone for Zone<M> {
    fn clone(&self) -> Self {
        Self {
            heading: self.heading.clone(),
            lands: self.lands,
            entries: self.entries.clone(),
        }
    }
}

impl<M: ExplorerModel> fmt::Debug for Zone<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Zone")
            .field("heading", &self.heading)
            .field("lands", &self.lands)
            .field("entries", &self.entries)
            .finish()
    }
}

/// One row: its key, how it draws and what it asks for, the section it
/// sits in, and the host's row.
pub struct Entry<M: ExplorerModel> {
    /// The row's identity across rebuilds.
    pub key: M::Key,
    /// A property, link, or section header row of the property grid. It
    /// draws the row, and its intents say what `Enter` and the inspect
    /// key mean on it.
    pub row: Row,
    /// The key of the section header this row sits under, so the fold
    /// keys can close the section from inside it.
    pub under: Option<M::Key>,
    /// The host's own row.
    pub host: M::Row,
}

impl<M: ExplorerModel> Entry<M> {
    /// A row keyed `key`, drawn as `row`, carrying the host's `host`, in
    /// no section.
    pub fn new(key: M::Key, row: impl Into<Row>, host: M::Row) -> Self {
        Self {
            key,
            row: row.into(),
            under: None,
            host,
        }
    }

    /// This row in the section headed by the row keyed `header`.
    #[must_use]
    pub fn under(mut self, header: M::Key) -> Self {
        self.under = Some(header);
        self
    }
}

impl<M: ExplorerModel> Clone for Entry<M> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            row: self.row.clone(),
            under: self.under.clone(),
            host: self.host.clone(),
        }
    }
}

impl<M: ExplorerModel> fmt::Debug for Entry<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("key", &self.key)
            .field("row", &self.row)
            .field("under", &self.under)
            .field("host", &self.host)
            .finish()
    }
}

/// How a frame shows its content.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameView<K> {
    /// The section headers that are open, by key.
    pub open: Vec<K>,
    /// Whether property zones list every property rather than their
    /// summary.
    pub full: bool,
}

impl<K> Default for FrameView<K> {
    fn default() -> Self {
        Self {
            open: Vec::new(),
            full: false,
        }
    }
}

impl<K: PartialEq> FrameView<K> {
    /// Whether the section headed by the row keyed `header` is open.
    pub fn is_open(&self, header: &K) -> bool {
        self.open.contains(header)
    }
}

/// A frame of drill history: the thing shown before a drill, how it was
/// shown, where its cursor stood, and what the host kept with it.
pub struct Frame<M: ExplorerModel> {
    /// The thing shown.
    pub id: M::Id,
    /// Its breadcrumb tag ([`ExplorerSource::tag`]).
    pub tag: String,
    /// How it was shown.
    pub view: FrameView<M::Key>,
    /// The key of the cursor's row, which `Back` puts the cursor on again.
    pub cursor: Option<M::Key>,
    /// What the host kept ([`ExplorerSource::save`]).
    pub saved: M::Saved,
}

impl<M: ExplorerModel> Clone for Frame<M> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            tag: self.tag.clone(),
            view: self.view.clone(),
            cursor: self.cursor.clone(),
            saved: self.saved.clone(),
        }
    }
}

impl<M: ExplorerModel> fmt::Debug for Frame<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("id", &self.id)
            .field("tag", &self.tag)
            .field("view", &self.view)
            .field("cursor", &self.cursor)
            .field("saved", &self.saved)
            .finish()
    }
}

impl<M: ExplorerModel> PartialEq for Frame<M> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.tag == other.tag
            && self.view == other.view
            && self.cursor == other.cursor
            && self.saved == other.saved
    }
}

#[cfg(test)]
#[path = "explorer_tests.rs"]
mod tests;
