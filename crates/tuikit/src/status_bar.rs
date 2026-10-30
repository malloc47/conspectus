//! The status bar: one line at the foot of a screen that says what just
//! happened, or what the keys do now, generated from the key tables of
//! the scope that has them.
//!
//! This is a layer 1 control built on the layer 2 key model. A screen's
//! last row tells the reader one of two things:
//!
//! - **A message**: what just happened, a [`Feedback`] in its severity's
//!   style (a hint for the next key, a confirmation, a refusal, a
//!   failure), cut with `…` when it is wider than its room.
//! - **The keys**: what each key does in the scope that has them (the
//!   screen, a pane with focus, or the overlay on top), beside the host's
//!   own segments (settings in effect, a count), all on one
//!   [`PriorityLine`] that gives way lowest rank first.
//!
//! The keys are a [`KeyHintBar`]: hints generated from key tables and a
//! program's scoped keymap ([`KeyHintBar::keymap`]), never written by
//! hand, so a hint cannot name a key its scope does not bind.
//! Each shown row of a table is a hint of its label and verb in the
//! footer's grammar (`Enter open`), rows that share their words join
//! their labels (`↑/↓ move`), and hints keep the order of their tables.
//! A row's rank ([`KeyRow::rank`]) says how long its hint holds on when
//! the bar is narrow; a row with none takes the rank the host gives its
//! table.
//!
//! What a key does can depend on where the reader is: `Enter` opens one
//! row and folds another, `a` acts on a thing whose name the bar should
//! show, a key cannot act on the row under the cursor. The host says so
//! with an [`Annotation`] per row, from its own state, each time it builds
//! the bar: leave the row out, give it another verb, a detail after the
//! verb (`Enter/a open report.txt`), a reason it cannot act now (drawn
//! dimmed, `d delete: read-only`), or another rank. Rows that end up with
//! the same words still join. A control's own footer
//! ([`Overlay::hints`](crate::overlay::Overlay::hints)) goes in whole,
//! segment by segment.
//!
//! Around the line the host draws what must never move: a lead that shows
//! in both modes (counts that need attention, busy chips), a scope shown
//! with the keys only (a tag naming whose keys they are, a mark), and a
//! trail after them (chips that sit at the end). The line gets the cells
//! they leave.
//!
//! ```
//! use ratatui::crossterm::event::KeyCode;
//! use ratatui::text::Span;
//! use tuikit::feedback::Feedback;
//! use tuikit::keys::{KeyRow, ch, key, shown};
//! use tuikit::priority_line::Segment;
//! use tuikit::status_bar::{Annotation, KeyHintBar, StatusBar};
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! enum Key {
//!     Open,
//!     Edit,
//!     Delete,
//!     Quit,
//! }
//!
//! const KEYS: &[KeyRow<Key>] = &[
//!     shown(key(KeyCode::Enter), Key::Open, "Enter", "open").rank(5),
//!     shown(ch('e'), Key::Edit, "e", "edit").rank(5),
//!     shown(ch('d'), Key::Delete, "d", "delete").rank(2),
//!     shown(ch('q'), Key::Quit, "q", "quit").rank(9),
//! ];
//!
//! let theme = Theme::default();
//! // On a text file `Enter` and `e` both edit it; it is read-only.
//! let hints = KeyHintBar::new().annotated(KEYS, 1, |key| match key {
//!     Key::Open | Key::Edit => Some(Annotation::new().verb("edit").detail("notes.txt")),
//!     Key::Delete => Some(Annotation::new().because("read-only")),
//!     Key::Quit => Some(Annotation::new()),
//! });
//! assert_eq!(hints.text(), "Enter/e edit notes.txt · d delete: read-only · q quit");
//!
//! let bar = StatusBar::new(&theme)
//!     .scope([Span::raw("[files] ")])
//!     .segment(Segment::new("sort:name").rank(3))
//!     .hints(&hints);
//! let text = |room| bar.line(room).to_string();
//! assert_eq!(
//!     text(80),
//!     "[files] sort:name · Enter/e edit notes.txt · d delete: read-only · q quit"
//! );
//! // Narrower, the lowest ranks give way first and the bar says so; a
//! // narrower segment comes back where a wider one did not fit.
//! assert_eq!(text(60), "[files] sort:name · Enter/e edit notes.txt · q quit …");
//! assert_eq!(text(45), "[files] Enter/e edit notes.txt · q quit …");
//! assert_eq!(text(30), "[files] sort:name · q quit …");
//!
//! // A message takes the bar's place, in its severity's style.
//! let saved = Feedback::success("saved notes.txt");
//! assert_eq!(bar.message(Some(&saved)).line(40).to_string(), "saved notes.txt");
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::feedback::Feedback;
use crate::keys::{
    ActiveScopes, Footer, Hint, KeyMatcher, KeyRow, Keymap, key_label, rows, unavailable_style,
};
use crate::priority_line::{PriorityLine, Segment};
use crate::text;
use crate::theme::Theme;

/// How a host shows one row of a key table in a [`KeyHintBar`] now: the
/// row as the table writes it unless a field says otherwise.
///
/// ```
/// use tuikit::status_bar::Annotation;
///
/// let attach = Annotation::new().verb("attach").detail("build-01").rank(6);
/// assert_eq!(attach.words("open"), "attach build-01");
/// let refused = Annotation::new().because("nothing selected");
/// assert_eq!(refused.words("open"), "open: nothing selected");
/// assert!(!refused.is_available());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Annotation {
    verb: Option<String>,
    detail: Option<String>,
    reason: Option<String>,
    rank: Option<u32>,
    unavailable: bool,
}

impl Annotation {
    /// The row as its table writes it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Say `verb` instead of the table's verb: what the key does here.
    #[must_use]
    pub fn verb(mut self, verb: impl Into<String>) -> Self {
        self.verb = Some(verb.into());
        self
    }

    /// Say `detail` after the verb: what the key acts on here
    /// (`a open report.txt`).
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The key cannot act here, because `reason`: the hint is drawn dimmed
    /// with the reason after its words (`d delete: read-only`). The key
    /// still does what its scope does with it (say why, most often).
    #[must_use]
    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self.unavailable = true;
        self
    }

    /// The key cannot act here; the hint is drawn dimmed.
    #[must_use]
    pub fn unavailable(mut self) -> Self {
        self.unavailable = true;
        self
    }

    /// Hold on at `rank` instead of the row's rank.
    #[must_use]
    pub fn rank(mut self, rank: u32) -> Self {
        self.rank = Some(rank);
        self
    }

    /// Whether the key can act here.
    pub fn is_available(&self) -> bool {
        !self.unavailable
    }

    /// The hint's words for a row whose table verb is `verb`: the verb
    /// (this one's, or the table's), the detail, and the reason.
    pub fn words(&self, verb: &str) -> String {
        let mut words = self.verb.as_deref().unwrap_or(verb).to_string();
        if let Some(detail) = &self.detail {
            words.push(' ');
            words.push_str(detail);
        }
        if let Some(reason) = &self.reason {
            words.push_str(": ");
            words.push_str(reason);
        }
        words
    }
}

/// One hint of a [`KeyHintBar`]: the keys' labels and what they do, or a
/// segment taken whole from a control's footer, with the rank it holds on
/// at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHint {
    labels: Vec<String>,
    words: String,
    keys: Vec<KeyMatcher>,
    rank: u32,
    available: bool,
}

impl KeyHint {
    /// The hint as drawn: the labels joined with `/`, a space, and the
    /// words (`Enter/e edit`); a segment taken whole as it was.
    pub fn text(&self) -> String {
        if self.labels.is_empty() {
            self.words.clone()
        } else {
            format!("{} {}", self.labels.join("/"), self.words)
        }
    }

    /// The keys the hint names, in order: what a check presses to see
    /// that each does what the hint says. Empty for a note.
    pub fn keys(&self) -> &[KeyMatcher] {
        &self.keys
    }

    /// How long the hint holds on in a narrow bar, the higher the longer.
    pub fn rank(&self) -> u32 {
        self.rank
    }

    /// Whether any of its keys can act now; a dimmed hint cannot.
    pub fn is_available(&self) -> bool {
        self.available
    }
}

/// The hints of the keys that act now, generated from key tables, in the
/// order the tables give them.
///
/// See the [module documentation](self) for how rows become hints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyHintBar {
    hints: Vec<KeyHint>,
}

impl KeyHintBar {
    /// A bar with no hints.
    pub fn new() -> Self {
        Self::default()
    }

    /// The bar with every shown row of `table` (included tables spliced
    /// in) and its notes as hints, as the table writes them. A row with
    /// no rank of its own holds on at `rank`.
    #[must_use]
    pub fn table<A>(self, table: &[KeyRow<A>], rank: u32) -> Self {
        self.annotated(table, rank, |_| Some(Annotation::new()))
    }

    /// The bar with the shown rows of `table` the host `annotate`s now as
    /// hints, and the table's notes. `annotate` returns `None` to leave a
    /// row out, or how to show it. A row with no rank of its own or in its
    /// annotation holds on at `rank`. A row whose words a hint of the bar
    /// already has joins that hint: its label after the others, the
    /// higher rank, available when either is.
    #[must_use]
    pub fn annotated<A>(
        mut self,
        table: &[KeyRow<A>],
        rank: u32,
        annotate: impl Fn(&A) -> Option<Annotation>,
    ) -> Self {
        for row in rows(table) {
            match row {
                KeyRow::Bind {
                    key,
                    action,
                    hint: Hint::Shown { label, verb },
                    available,
                    rank: own,
                } => {
                    let Some(annotation) = annotate(action) else {
                        continue;
                    };
                    let hint = KeyHint {
                        labels: vec![(*label).to_string()],
                        words: annotation.words(verb),
                        keys: vec![*key],
                        rank: annotation.rank.or(*own).unwrap_or(rank),
                        available: *available && annotation.is_available(),
                    };
                    self.join(hint);
                }
                KeyRow::Note(note) => self.hints.push(KeyHint {
                    labels: Vec::new(),
                    words: (*note).to_string(),
                    keys: Vec::new(),
                    rank,
                    available: true,
                }),
                _ => {}
            }
        }
        self
    }

    /// The bar with the bindings of `keymap` that act in the scopes
    /// `active` (the pane with focus and the program's) as hints, in the
    /// keymap's order, those the host `annotate`s now: each binding with a
    /// verb names its first key, as `label verb`. A binding is left out
    /// when `annotate` returns `None`, when it cannot act now in the
    /// host's context `cx` ([`Bind::available_when`]), and when a scope
    /// looked up before its own takes its key, so the bar names only keys
    /// that do what it says where the reader is. Ranks and joining are
    /// [`annotated`](Self::annotated)'s.
    ///
    /// [`Bind::available_when`]: crate::keys::Bind::available_when
    ///
    /// ```
    /// use ratatui::crossterm::event::KeyCode;
    /// use tuikit::keys::{ActiveScopes, Bind, Keymap, Scope, bind, ch, key};
    /// use tuikit::status_bar::{Annotation, KeyHintBar};
    ///
    /// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    /// enum Do {
    ///     Next,
    ///     Back,
    ///     Quit,
    /// }
    ///
    /// /// What the host knows: whether there is anything to go back to.
    /// struct Model {
    ///     history: usize,
    /// }
    ///
    /// const KEYS: &[Bind<Do, Model>] = &[
    ///     bind("list.next", Scope::Pane("list"), &[key(KeyCode::Down)], Do::Next)
    ///         .verb("move")
    ///         .help("List", "Move to the next row"),
    ///     bind("back", Scope::Global, &[key(KeyCode::Backspace)], Do::Back)
    ///         .verb("back")
    ///         .help("Program", "Go back")
    ///         .available_when(|model: &Model| model.history > 0),
    ///     bind("quit", Scope::Global, &[ch('q')], Do::Quit).verb("quit").help("Program", "Quit"),
    /// ];
    /// let keymap = Keymap::builder().rows(KEYS).build().unwrap();
    /// let bar = |active, model: &Model| {
    ///     KeyHintBar::new()
    ///         .keymap(&keymap, active, model, 1, |_| Some(Annotation::new()))
    ///         .text()
    /// };
    /// let fresh = Model { history: 0 };
    /// assert_eq!(bar(ActiveScopes::pane("list"), &fresh), "\u{2193} move \u{b7} q quit");
    /// assert_eq!(
    ///     bar(ActiveScopes::global(), &Model { history: 2 }),
    ///     "Backspace back \u{b7} q quit"
    /// );
    /// ```
    #[must_use]
    pub fn keymap<A: Copy + PartialEq, Cx>(
        mut self,
        keymap: &Keymap<A, Cx>,
        active: ActiveScopes,
        cx: &Cx,
        rank: u32,
        annotate: impl Fn(&A) -> Option<Annotation>,
    ) -> Self {
        for binding in keymap.bindings() {
            let Some(verb) = binding.verb() else {
                continue;
            };
            if !keymap.first_key_reaches(active, binding) || !binding.can_act(cx) {
                continue;
            }
            let Some(annotation) = annotate(&binding.action()) else {
                continue;
            };
            let key = binding.keys()[0];
            self.join(KeyHint {
                labels: vec![key_label(&key)],
                words: annotation.words(verb),
                keys: vec![key],
                rank: annotation.rank.or_else(|| binding.rank()).unwrap_or(rank),
                available: annotation.is_available(),
            });
        }
        self
    }

    /// The bar with `footer`'s segments as hints, each whole: a control's
    /// own footer ([`Overlay::hints`](crate::overlay::Overlay::hints)). A
    /// segment with no rank of its own holds on at `rank`, and a note,
    /// which names no key, a step below it, so a narrow bar gives up the
    /// note before any key wherever the footer puts it.
    #[must_use]
    pub fn footer(mut self, footer: &Footer, rank: u32) -> Self {
        self.hints
            .extend(footer.segments().iter().map(|segment| KeyHint {
                labels: Vec::new(),
                words: segment.text.clone(),
                keys: segment.keys.clone(),
                rank: segment.rank.unwrap_or(if segment.keys.is_empty() {
                    rank.saturating_sub(1)
                } else {
                    rank
                }),
                available: segment.available,
            }));
        self
    }

    /// Add `hint`, or join it to the keyed hint with the same words.
    fn join(&mut self, hint: KeyHint) {
        let existing = self
            .hints
            .iter_mut()
            .find(|other| !other.labels.is_empty() && other.words == hint.words);
        match existing {
            Some(other) => {
                for label in hint.labels {
                    if !other.labels.contains(&label) {
                        other.labels.push(label);
                    }
                }
                other.keys.extend(hint.keys);
                other.rank = other.rank.max(hint.rank);
                other.available |= hint.available;
            }
            None => self.hints.push(hint),
        }
    }

    /// The hints, in order.
    pub fn hints(&self) -> &[KeyHint] {
        &self.hints
    }

    /// Whether the bar names no key and has no note.
    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    /// Every key the bar names, in order.
    pub fn keys(&self) -> impl Iterator<Item = KeyMatcher> + '_ {
        self.hints.iter().flat_map(|hint| hint.keys.iter().copied())
    }

    /// The hints as one line of text, joined with
    /// [`HINT_SEPARATOR`](crate::keys::HINT_SEPARATOR), every one shown.
    pub fn text(&self) -> String {
        self.hints
            .iter()
            .map(KeyHint::text)
            .collect::<Vec<_>>()
            .join(crate::keys::HINT_SEPARATOR)
    }

    /// The hints as segments of a [`PriorityLine`], each with its rank:
    /// in the theme's `muted` look, a hint that cannot act now in
    /// [`unavailable_style`].
    pub fn segments(&self, theme: &Theme) -> Vec<Segment> {
        let muted = Style::default().add_modifier(theme.muted);
        let unavailable = unavailable_style(theme);
        self.hints
            .iter()
            .map(|hint| {
                let style = if hint.available { muted } else { unavailable };
                Segment::new(Span::styled(hint.text(), style)).rank(hint.rank)
            })
            .collect()
    }
}

/// The bottom row of a screen: a message, or the host's segments and the
/// keys that act now, with the host's fixed spans around them.
///
/// See the [module documentation](self) for its parts.
#[derive(Clone, Debug)]
pub struct StatusBar<'t> {
    theme: &'t Theme,
    lead: Vec<Span<'static>>,
    scope: Vec<Span<'static>>,
    line: PriorityLine,
    trail: Vec<Span<'static>>,
    message: Option<Span<'static>>,
}

impl<'t> StatusBar<'t> {
    /// An empty bar drawn with `theme`: its line is a [`PriorityLine`]
    /// with the default separator and mark.
    pub fn new(theme: &'t Theme) -> Self {
        Self {
            theme,
            lead: Vec::new(),
            scope: Vec::new(),
            line: PriorityLine::new(theme),
            trail: Vec::new(),
            message: None,
        }
    }

    /// Lead the bar with `spans` in both modes: what must stay in view
    /// whatever the bar says (a count that needs attention, a busy chip).
    #[must_use]
    pub fn lead(mut self, spans: impl IntoIterator<Item = Span<'static>>) -> Self {
        self.lead.extend(spans);
        self
    }

    /// Draw `spans` after the lead when the bar shows the keys, not a
    /// message: a tag naming the scope whose keys they are, a mark that
    /// must not give way.
    #[must_use]
    pub fn scope(mut self, spans: impl IntoIterator<Item = Span<'static>>) -> Self {
        self.scope.extend(spans);
        self
    }

    /// Add one of the host's segments to the line, after the others: a
    /// setting in effect, with the rank it holds on at.
    #[must_use]
    pub fn segment(mut self, segment: Segment) -> Self {
        self.line.push(segment);
        self
    }

    /// Add `hints` to the line, after the segments so far.
    #[must_use]
    pub fn hints(mut self, hints: &KeyHintBar) -> Self {
        for segment in hints.segments(self.theme) {
            self.line.push(segment);
        }
        self
    }

    /// End the bar with `spans` when it shows the keys: chips that sit at
    /// the end of the line.
    #[must_use]
    pub fn trail(mut self, spans: impl IntoIterator<Item = Span<'static>>) -> Self {
        self.trail.extend(spans);
        self
    }

    /// Show `message` instead of the scope, the line, and the trail, in
    /// its severity's style; `None` shows the keys.
    #[must_use]
    pub fn message(mut self, message: Option<&Feedback>) -> Self {
        self.message = message.map(|feedback| feedback.span(self.theme));
        self
    }

    /// The line the bar draws: whether it shows a message.
    pub fn shows_message(&self) -> bool {
        self.message.is_some()
    }

    /// The bar's spans in `width` cells: the lead, then the message cut
    /// with `…` to the cells the lead leaves; or the lead, the scope, the
    /// line in the cells the fixed spans leave, and the trail. Only a
    /// lead and scope wider than `width` make it wider.
    pub fn spans(&self, width: usize) -> Vec<Span<'static>> {
        let mut spans = self.lead.clone();
        let lead = text::spans_width(&self.lead);
        if let Some(message) = &self.message {
            let room = width.saturating_sub(lead);
            spans.extend(text::fit_spans(vec![message.clone()], room));
            return spans;
        }
        spans.extend(self.scope.iter().cloned());
        let fixed = lead + text::spans_width(&self.scope) + text::spans_width(&self.trail);
        spans.extend(self.line.spans(width.saturating_sub(fixed)));
        spans.extend(self.trail.iter().cloned());
        spans
    }

    /// The bar as one line in `width` cells: [`spans`](Self::spans).
    pub fn line(&self, width: usize) -> Line<'static> {
        Line::from(self.spans(width))
    }

    /// Draw the bar on the first row of `area`, clipped to it.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.line(usize::from(area.width))).render(area, buf);
    }
}

#[cfg(test)]
#[path = "status_bar_tests.rs"]
mod tests;
