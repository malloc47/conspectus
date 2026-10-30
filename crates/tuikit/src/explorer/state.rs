//! The explorer's state and the rules its keys and moves follow.

use std::fmt;

use super::{
    Entry, ExplorerKey, ExplorerModel, ExplorerSource, Frame, FrameView, Outcome, Refreshed,
    Refusal, Zone,
};
use crate::keys::{Footer, Hint, KeyRow, Segment, rows};
use crate::property_grid::{GridLines, Intent, Row};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;
use crate::theme::Theme;

/// An explorer over the host's model `M`: the thing it shows, how, where
/// its cursor is, the drill frames behind it, whether `Back` is armed,
/// and the window its zone drew.
///
/// It reads the host's data only through an [`ExplorerSource`] the host
/// builds for each call, so it holds owned values and lives in the host's
/// model. See the [module documentation](super) for the rules.
pub struct Explorer<M: ExplorerModel> {
    content: M::Content,
    id: M::Id,
    tag: String,
    view: FrameView<M::Key>,
    /// The cursor's place among the entries and the key of its row.
    cursor: Option<(usize, M::Key)>,
    /// Drill history, oldest first. The frame shown is not in it.
    frames: Vec<Frame<M>>,
    armed: bool,
    window: ScrollWindow,
}

/// Which way a fold key folds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fold {
    Toggle,
    Open,
    Close,
}

impl<M: ExplorerModel> Explorer<M> {
    // ---- opening and reading -------------------------------------------

    /// An explorer showing `id` with the default view (every section
    /// closed, the summary of properties), the cursor on the first row
    /// whichever zone it is in, no frames, and an empty window; `None`
    /// when the source cannot load `id`.
    pub fn open<S: ExplorerSource<Model = M>>(id: &M::Id, source: &S) -> Option<Self> {
        let content = source.load(id)?;
        let mut explorer = Self {
            id: source.id(&content),
            tag: source.tag(&content),
            content,
            view: FrameView::default(),
            cursor: None,
            frames: Vec::new(),
            armed: false,
            window: ScrollWindow::default(),
        };
        explorer.place_first(source);
        Some(explorer)
    }

    /// What is shown.
    pub const fn content(&self) -> &M::Content {
        &self.content
    }

    /// The identity of what is shown.
    pub const fn id(&self) -> &M::Id {
        &self.id
    }

    /// The breadcrumb tag of what is shown, as the source gave it.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// How it is shown: the open sections and the every-property switch.
    pub const fn view(&self) -> &FrameView<M::Key> {
        &self.view
    }

    /// The cursor's place among the entries of every zone, in order;
    /// `None` when there are none.
    pub fn cursor(&self) -> Option<usize> {
        self.cursor.as_ref().map(|(index, _)| *index)
    }

    /// The key of the cursor's row.
    pub fn cursor_key(&self) -> Option<&M::Key> {
        self.cursor.as_ref().map(|(_, key)| key)
    }

    /// The drill frames, oldest first: one per drill that led here.
    pub fn frames(&self) -> &[Frame<M>] {
        &self.frames
    }

    /// Whether `Back` is armed: the last key was a `Back` with no frame,
    /// and another leaves.
    pub const fn is_armed(&self) -> bool {
        self.armed
    }

    /// The zones the source lays out for what is shown, as it is shown.
    pub fn zones<S: ExplorerSource<Model = M>>(&self, source: &S) -> Vec<Zone<M>> {
        source.zones(&self.content, &self.view)
    }

    /// The entries of every zone, in order: what the cursor moves over.
    pub fn entries<S: ExplorerSource<Model = M>>(&self, source: &S) -> Vec<Entry<M>> {
        flatten(self.zones(source))
    }

    /// The cursor's entry.
    pub fn cursor_entry<S: ExplorerSource<Model = M>>(&self, source: &S) -> Option<Entry<M>> {
        let index = self.cursor()?;
        self.entries(source).into_iter().nth(index)
    }

    // ---- keys, and the moves a host makes without one -------------------

    /// Do what `key` asks and say what is left for the host. Every key but
    /// `Back` disarms the explorer.
    ///
    /// | Key | Does | Answers |
    /// | --- | --- | --- |
    /// | `Down`, `Up`, `PageDown`, `PageUp`, `First`, `Last` | moves the cursor, stopping at the ends | `None` |
    /// | `Activate` on a property with a value | nothing | `Copy` |
    /// | `Activate` on a property with none | nothing | `None` |
    /// | `Activate` on a section header | folds it | `Toggle` |
    /// | `Activate` on a link whose far end loads | drills ([`drill_to`](Self::drill_to)) | `Drill` |
    /// | `Activate` on a link whose far end does not load | nothing | `Follow` |
    /// | `Activate` on a link with no far end | nothing | `Refused(NoFarEnd)` |
    /// | `Inspect` on a cut property | nothing | `Inspect` |
    /// | `Inspect` elsewhere | nothing | `Refused(NothingToInspect)` |
    /// | `ToggleSection` on a header | folds it | `Toggle` |
    /// | `OpenSection` on a closed header | opens it | `Toggle { open: true }` |
    /// | `CloseSection` on an open header or a row under one | closes it, the cursor on its header | `Toggle { open: false }` |
    /// | a fold key with nothing to fold (an open section opened, a closed one closed) | nothing | `None` |
    /// | a fold key on a row in no section | nothing | `Refused(NotInSection)` |
    /// | `ToggleFull` | flips the switch, the cursor kept by key | `None`, or `Refused(NothingMore)` when no row changed |
    /// | `Back` | see [`back`](Self::back) | `Back`, `Armed`, `Leave`, or `Refused(Gone)` |
    pub fn handle<S: ExplorerSource<Model = M>>(
        &mut self,
        key: ExplorerKey,
        source: &S,
    ) -> Outcome<M> {
        if key != ExplorerKey::Back {
            self.armed = false;
        }
        match key {
            ExplorerKey::Down => self.step(source, 1),
            ExplorerKey::Up => self.step(source, -1),
            ExplorerKey::PageDown => self.step(source, self.page()),
            ExplorerKey::PageUp => self.step(source, -self.page()),
            ExplorerKey::First => self.step(source, i32::MIN),
            ExplorerKey::Last => self.step(source, i32::MAX),
            ExplorerKey::Activate => self.activate(source),
            ExplorerKey::Inspect => self.inspect(source),
            ExplorerKey::ToggleSection => self.fold(source, Fold::Toggle),
            ExplorerKey::OpenSection => self.fold(source, Fold::Open),
            ExplorerKey::CloseSection => self.fold(source, Fold::Close),
            ExplorerKey::ToggleFull => self.toggle_full(source),
            ExplorerKey::Back => self.back(source),
        }
    }

    /// Drill into `id`, as `Enter` on a link to it does: push a frame (what
    /// is shown, its tag, its view, the cursor's key, and
    /// [`ExplorerSource::save`]), show `id` with the default view and the
    /// cursor on its first row, and answer `Drill`. When `id` does not
    /// load, nothing changes and the answer is `Follow`. The window stays.
    pub fn drill_to<S: ExplorerSource<Model = M>>(&mut self, id: M::Id, source: &S) -> Outcome<M> {
        self.armed = false;
        let Some(content) = source.load(&id) else {
            return Outcome::Follow { to: id };
        };
        let frame = Frame {
            id: std::mem::replace(&mut self.id, source.id(&content)),
            tag: std::mem::replace(&mut self.tag, source.tag(&content)),
            view: std::mem::take(&mut self.view),
            cursor: self.cursor_key().cloned(),
            saved: source.save(),
        };
        self.frames.push(frame);
        self.content = content;
        self.place_first(source);
        Outcome::Drill {
            to: self.id.clone(),
        }
    }

    /// Go back one level.
    ///
    /// With frames, the newest is popped and loaded, its view restored
    /// (less the sections that are gone), and the cursor put back on its
    /// key by the reseat rule; the answer is `Back` with what the host
    /// saved. A frame whose thing no longer loads is dropped and the next
    /// older one tried in the same press; when none loads, what is shown
    /// stays and the answer is `Refused(Gone)`.
    ///
    /// With no frames, a first press arms the explorer (`Armed`) and a
    /// second leaves (`Leave`). The host disarms it on anything else it
    /// handles ([`disarm`](Self::disarm)), so only two presses in a row
    /// leave.
    pub fn back<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Outcome<M> {
        if self.frames.is_empty() {
            self.armed = !self.armed;
            return if self.armed {
                Outcome::Armed
            } else {
                Outcome::Leave
            };
        }
        self.armed = false;
        while let Some(frame) = self.frames.pop() {
            let Some(content) = source.load(&frame.id) else {
                continue;
            };
            self.id = source.id(&content);
            self.tag = source.tag(&content);
            self.content = content;
            self.view = frame.view;
            let zones = self.zones(source);
            self.prune(&zones);
            let zones = self.zones(source);
            self.reseat(&zones, frame.cursor);
            return Outcome::Back {
                to: self.id.clone(),
                saved: frame.saved,
            };
        }
        Outcome::Refused(Refusal::Gone)
    }

    /// Put the cursor on the first entry `find` accepts, opening a closed
    /// section that holds one; say whether one was found. `find` sees what
    /// is shown and each entry. A section opened to look in is closed
    /// again when it held none.
    pub fn seek<S: ExplorerSource<Model = M>>(
        &mut self,
        source: &S,
        find: impl Fn(&M::Content, &Entry<M>) -> bool,
    ) -> bool {
        let entries = self.entries(source);
        if let Some(index) = entries.iter().position(|entry| find(&self.content, entry)) {
            self.place(&entries, index);
            return true;
        }
        let closed: Vec<M::Key> = entries
            .iter()
            .filter(|entry| matches!(entry.row, Row::Disclosure(_)))
            .map(|entry| entry.key.clone())
            .filter(|key| !self.view.is_open(key))
            .collect();
        for header in closed {
            self.view.open.push(header.clone());
            let entries = self.entries(source);
            if let Some(index) = entries.iter().position(|entry| find(&self.content, entry)) {
                self.place(&entries, index);
                return true;
            }
            self.view.open.retain(|open| open != &header);
        }
        false
    }

    /// Forget an armed `Back`: the host calls it for everything else it
    /// handles, so the arm lasts only across two presses in a row.
    pub fn disarm(&mut self) {
        self.armed = false;
    }

    // ---- rebuilds and frames ---------------------------------------------

    /// Load what is shown again from `source`, whose data changed: keep the
    /// frames, the view (less the sections that are gone), and the cursor,
    /// put back by its key, and answer `Kept`; answer `Gone`, changing
    /// nothing, when it no longer loads. Frames are loaded only when
    /// `Back` reaches them.
    ///
    /// A host calls this when its data changed and its own selection did
    /// not, and [`open`](Self::open) when its selection names something
    /// else.
    pub fn refresh<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Refreshed {
        let Some(content) = source.load(&self.id) else {
            return Refreshed::Gone;
        };
        let key = self.cursor_key().cloned();
        self.id = source.id(&content);
        self.tag = source.tag(&content);
        self.content = content;
        let zones = self.zones(source);
        self.prune(&zones);
        let zones = self.zones(source);
        self.reseat(&zones, key);
        Refreshed::Kept
    }

    /// Every zone laid out in the source's grid at `width` cells: each
    /// heading as a line that is no row, then the zone's rows, the cursor
    /// in the active look when `focused` and the inactive one when not.
    pub fn lines<S: ExplorerSource<Model = M>>(
        &self,
        source: &S,
        width: usize,
        focused: bool,
        theme: &Theme,
    ) -> GridLines {
        let mut grid = source.grid();
        for zone in self.zones(source) {
            if let Some(heading) = &zone.heading {
                grid.push_line(heading.line(width, theme));
            }
            for entry in zone.entries {
                grid.push(entry.row);
            }
        }
        grid.lines(width, self.cursor(), focused, theme)
    }

    /// The window the zone drew in the last frame, which the next frame
    /// starts from. Empty before the first frame.
    pub const fn window(&self) -> ScrollWindow {
        self.window
    }

    /// Keep the window a frame drew (what a
    /// [`StackedMasterDetail`](crate::master_detail::StackedMasterDetail)
    /// returns): the next frame starts from it and a page key moves the
    /// cursor by the rows it showed. A drill and a `Back` keep it.
    pub fn keep_window(&mut self, window: ScrollWindow) {
        self.window = window;
    }

    /// The keys a status line names for the explorer, from `table` (the
    /// explorer's table, or a host table that includes it): moving,
    /// `Enter` with the verb of what it does on the cursor's row (`copy`,
    /// `follow`, `fold`; the table's own verb, unavailable, on a row where
    /// it does nothing), folding, and going back.
    pub fn footer<S: ExplorerSource<Model = M>>(
        &self,
        table: &[KeyRow<ExplorerKey>],
        source: &S,
    ) -> Footer {
        let verb = self
            .cursor_entry(source)
            .and_then(|entry| entry.row.activate())
            .map(|intent| match intent {
                Intent::Copy { .. } => "copy",
                Intent::Inspect { .. } => "inspect",
                Intent::Toggle => "fold",
                Intent::Follow => "follow",
            });
        let activate = rows(table).find_map(|row| match row {
            KeyRow::Bind {
                action: ExplorerKey::Activate,
                hint: Hint::Shown { label, verb },
                ..
            } => Some(format!("{label} {verb}")),
            _ => None,
        });
        let shown = Footer::only(table, |key| {
            matches!(
                key,
                ExplorerKey::Up
                    | ExplorerKey::Down
                    | ExplorerKey::Activate
                    | ExplorerKey::ToggleSection
                    | ExplorerKey::Back
            )
        });
        shown
            .segments()
            .iter()
            .map(|segment| match (&activate, verb) {
                (Some(text), Some(verb)) if *text == segment.text => {
                    let label = text
                        .rsplit_once(' ')
                        .map_or(text.as_str(), |(label, _)| label);
                    Segment {
                        text: format!("{label} {verb}"),
                        available: segment.available,
                    }
                }
                (Some(text), None) if *text == segment.text => Segment {
                    text: segment.text.clone(),
                    available: false,
                },
                _ => segment.clone(),
            })
            .collect()
    }

    // ---- the rules -----------------------------------------------------

    /// How far a page key moves: the rows the zone showed, at least one.
    fn page(&self) -> i32 {
        i32::try_from(self.window.page()).unwrap_or(i32::MAX)
    }

    /// Put the cursor on the entry at `index`, or on none past the end.
    fn place(&mut self, entries: &[Entry<M>], index: usize) {
        self.cursor = entries.get(index).map(|entry| (index, entry.key.clone()));
    }

    /// Put the cursor on the first entry, whichever zone it is in.
    fn place_first<S: ExplorerSource<Model = M>>(&mut self, source: &S) {
        let entries = self.entries(source);
        self.place(&entries, 0);
    }

    /// Move the cursor `delta` entries, stopping at the ends.
    fn step<S: ExplorerSource<Model = M>>(&mut self, source: &S, delta: i32) -> Outcome<M> {
        let entries = self.entries(source);
        if entries.is_empty() {
            self.cursor = None;
            return Outcome::None;
        }
        let from = self.cursor().unwrap_or(0).min(entries.len() - 1);
        let to = Ends::Clamp.step(from, entries.len(), delta);
        self.place(&entries, to);
        Outcome::None
    }

    /// Put the cursor back after a rebuild: on the entry keyed `key`, else
    /// on the first entry of the first zone that lands, else on the first
    /// entry, else on none.
    fn reseat(&mut self, zones: &[Zone<M>], key: Option<M::Key>) {
        let mut same = None;
        let mut landing = None;
        let mut at = 0;
        for zone in zones {
            if landing.is_none() && zone.lands && !zone.entries.is_empty() {
                landing = Some(at);
            }
            for entry in &zone.entries {
                if same.is_none() && key.as_ref() == Some(&entry.key) {
                    same = Some(at);
                }
                at += 1;
            }
        }
        let entries = flatten(zones.to_vec());
        let index = same.or(landing).unwrap_or(0);
        self.place(&entries, index);
    }

    /// Rebuild with the view as it is now and reseat the cursor by its key.
    fn rebuild<S: ExplorerSource<Model = M>>(&mut self, source: &S, key: Option<M::Key>) {
        let zones = self.zones(source);
        self.reseat(&zones, key);
    }

    /// Drop the open sections whose header `zones` no longer list.
    fn prune(&mut self, zones: &[Zone<M>]) {
        let headers: Vec<&M::Key> = zones
            .iter()
            .flat_map(|zone| &zone.entries)
            .filter(|entry| matches!(entry.row, Row::Disclosure(_)))
            .map(|entry| &entry.key)
            .collect();
        self.view.open.retain(|key| headers.contains(&key));
    }

    /// What `Enter` does on the cursor's row.
    fn activate<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Outcome<M> {
        let Some(entry) = self.cursor_entry(source) else {
            return Outcome::None;
        };
        match entry.row.activate() {
            Some(Intent::Copy { label, text }) => Outcome::Copy { label, text },
            Some(Intent::Inspect { label, text }) => Outcome::Inspect { label, text },
            Some(Intent::Toggle) => self.flip(source, entry.key),
            Some(Intent::Follow) => match source.follow(&self.content, &entry) {
                Some(to) => self.drill_to(to, source),
                None => Outcome::Refused(Refusal::NoFarEnd),
            },
            None if matches!(entry.row, Row::Link(_)) => Outcome::Refused(Refusal::NoFarEnd),
            None => Outcome::None,
        }
    }

    /// What the inspect key does on the cursor's row.
    fn inspect<S: ExplorerSource<Model = M>>(&self, source: &S) -> Outcome<M> {
        let Some(entry) = self.cursor_entry(source) else {
            return Outcome::None;
        };
        match entry.row.inspect() {
            Some(Intent::Inspect { label, text }) => Outcome::Inspect { label, text },
            _ => Outcome::Refused(Refusal::NothingToInspect),
        }
    }

    /// What a fold key does on the cursor's row.
    fn fold<S: ExplorerSource<Model = M>>(&mut self, source: &S, fold: Fold) -> Outcome<M> {
        let Some(entry) = self.cursor_entry(source) else {
            return Outcome::None;
        };
        let header = matches!(entry.row, Row::Disclosure(_)).then_some(entry.key);
        match (fold, header, entry.under) {
            (Fold::Toggle, Some(header), _) => self.flip(source, header),
            (Fold::Open, Some(header), _) if !self.view.is_open(&header) => {
                self.flip(source, header)
            }
            (Fold::Close, Some(header), _) | (Fold::Close, None, Some(header))
                if self.view.is_open(&header) =>
            {
                self.view.open.retain(|open| open != &header);
                self.rebuild(source, Some(header));
                Outcome::Toggle { open: false }
            }
            (Fold::Open | Fold::Close, Some(_), _) | (Fold::Open | Fold::Close, None, Some(_)) => {
                Outcome::None
            }
            (_, None, _) => Outcome::Refused(Refusal::NotInSection),
        }
    }

    /// Open the section headed `header` if it is closed and close it if
    /// it is open, the cursor kept by its key.
    fn flip<S: ExplorerSource<Model = M>>(&mut self, source: &S, header: M::Key) -> Outcome<M> {
        let key = self.cursor_key().cloned();
        let open = !self.view.is_open(&header);
        if open {
            self.view.open.push(header);
        } else {
            self.view.open.retain(|key| key != &header);
        }
        self.rebuild(source, key);
        Outcome::Toggle { open }
    }

    /// Flip the every-property switch, the cursor kept by its key; refuse
    /// with `NothingMore` (the switch left flipped) when no row changed.
    fn toggle_full<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Outcome<M> {
        let before = keys_of(&self.entries(source));
        let key = self.cursor_key().cloned();
        self.view.full = !self.view.full;
        let zones = self.zones(source);
        self.reseat(&zones, key);
        if keys_of(&flatten(zones)) == before {
            Outcome::Refused(Refusal::NothingMore)
        } else {
            Outcome::None
        }
    }
}

/// The entries of `zones`, in order.
fn flatten<M: ExplorerModel>(zones: Vec<Zone<M>>) -> Vec<Entry<M>> {
    zones.into_iter().flat_map(|zone| zone.entries).collect()
}

/// The keys of `entries`, in order.
fn keys_of<M: ExplorerModel>(entries: &[Entry<M>]) -> Vec<M::Key> {
    entries.iter().map(|entry| entry.key.clone()).collect()
}

impl<M: ExplorerModel> Clone for Explorer<M>
where
    M::Content: Clone,
{
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            id: self.id.clone(),
            tag: self.tag.clone(),
            view: self.view.clone(),
            cursor: self.cursor.clone(),
            frames: self.frames.clone(),
            armed: self.armed,
            window: self.window,
        }
    }
}

impl<M: ExplorerModel> fmt::Debug for Explorer<M>
where
    M::Content: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Explorer")
            .field("content", &self.content)
            .field("id", &self.id)
            .field("tag", &self.tag)
            .field("view", &self.view)
            .field("cursor", &self.cursor)
            .field("frames", &self.frames)
            .field("armed", &self.armed)
            .field("window", &self.window)
            .finish()
    }
}
