//! The scoped keymap: a program's own keys in scopes (the program, a pane
//! with focus, an overlay mode), each binding with a stable id, the words
//! its hint and its help row say, and the checks that keep two keys of one
//! scope from ever matching one key event.

use std::fmt;

use ratatui::crossterm::event::KeyEvent;

use super::matcher::{KeyMatcher, key_label};
use super::table::{Hint, KeyRow, rows};
use crate::help::HelpAction;

/// Where a binding applies.
///
/// A key is looked up in the scopes that have the keys now
/// ([`ActiveScopes`]), most particular first: an overlay mode, then the
/// pane with focus, then the program. A key bound in a pane's scope means
/// that pane's thing while it has focus, and the program's everywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Wherever the keymap has the keys, whichever pane has focus.
    Global,
    /// While the pane of this name has focus.
    Pane(&'static str),
    /// While the overlay mode of this name is on: a mode that lets the
    /// program's keys through but gives some keys a meaning of its own. An
    /// overlay that owns every key keeps its own key table and resolves it
    /// before the keymap instead.
    Overlay(&'static str),
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Global => f.write_str("the global scope"),
            Scope::Pane(name) => write!(f, "the `{name}` pane's scope"),
            Scope::Overlay(name) => write!(f, "the `{name}` overlay's scope"),
        }
    }
}

/// The scopes that have the keys now, in the order a key is looked up:
/// the overlay mode that is on, the pane that has focus, then the global
/// scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveScopes {
    overlay: Option<&'static str>,
    pane: Option<&'static str>,
}

impl ActiveScopes {
    /// The global scope alone: no pane has focus and no overlay mode is on.
    pub const fn global() -> Self {
        Self {
            overlay: None,
            pane: None,
        }
    }

    /// The pane `name` has focus: its scope, then the global one.
    pub const fn pane(name: &'static str) -> Self {
        Self {
            overlay: None,
            pane: Some(name),
        }
    }

    /// These scopes with the overlay mode `name` on, looked up first.
    #[must_use]
    pub const fn overlay(self, name: &'static str) -> Self {
        Self {
            overlay: Some(name),
            pane: self.pane,
        }
    }

    /// The scopes, in the order a key is looked up.
    pub fn scopes(&self) -> impl Iterator<Item = Scope> + use<> {
        let overlay = self.overlay.map(Scope::Overlay);
        let pane = self.pane.map(Scope::Pane);
        overlay.into_iter().chain(pane).chain([Scope::Global])
    }

    /// Whether `scope` has the keys now.
    pub fn contains(&self, scope: Scope) -> bool {
        self.scopes().any(|active| active == scope)
    }
}

/// One binding of a keymap as a table declares it: a `const` row written
/// with [`bind`] and its builders.
///
/// A binding has a stable `id` (lower-case letters, digits, `.`, and
/// `-`: `tree.down`, `view.next`) by which a later remap or a help
/// annotation names it without touching the table; the `scope` it applies
/// in; its keys, the first of which a hint names and the rest aliases;
/// the `action` it stands for; the short verb its hint says (none leaves
/// the binding out of hints); the help section and text its help row
/// says; the rank its hint holds on at in a narrow status bar; and
/// whether it can act now, from the host's context `Cx`.
pub struct Bind<A: 'static, Cx: 'static = ()> {
    id: &'static str,
    scope: Scope,
    keys: &'static [KeyMatcher],
    action: A,
    verb: Option<&'static str>,
    section: &'static str,
    help: &'static str,
    rank: Option<u32>,
    available: Option<fn(&Cx) -> bool>,
}

// By hand: a derive would ask `Cx` to be `Clone` and `Debug` too.
impl<A: Copy, Cx> Clone for Bind<A, Cx> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Copy, Cx> Copy for Bind<A, Cx> {}

impl<A: fmt::Debug, Cx> fmt::Debug for Bind<A, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bind")
            .field("id", &self.id)
            .field("scope", &self.scope)
            .field("keys", &self.keys)
            .field("action", &self.action)
            .field("verb", &self.verb)
            .field("section", &self.section)
            .field("help", &self.help)
            .field("rank", &self.rank)
            .field("available", &self.available.is_some())
            .finish()
    }
}

/// A binding `id` of `keys` to `action` in `scope`, with no verb, no help
/// yet, no rank, and always able to act.
///
/// ```
/// use ratatui::crossterm::event::KeyCode;
/// use tuikit::keys::{Bind, Scope, bind, ch, key};
///
/// const DOWN: Bind<u8> = bind("list.down", Scope::Pane("list"), &[key(KeyCode::Down), ch('j')], 1)
///     .verb("move")
///     .help("Moving", "Move to the next row")
///     .rank(3);
/// assert_eq!(DOWN.id(), "list.down");
/// ```
pub const fn bind<A, Cx>(
    id: &'static str,
    scope: Scope,
    keys: &'static [KeyMatcher],
    action: A,
) -> Bind<A, Cx> {
    Bind {
        id,
        scope,
        keys,
        action,
        verb: None,
        section: "",
        help: "",
        rank: None,
        available: None,
    }
}

// `Copy` actions have no destructor, which a `const fn` that takes the
// binding by value needs; every action enum is `Copy`.
impl<A: Copy, Cx> Bind<A, Cx> {
    /// The binding with `verb` as what its hint says its first key does:
    /// a short imperative verb with no "to" (`move`, `attach`).
    #[must_use]
    pub const fn verb(self, verb: &'static str) -> Self {
        Self {
            verb: Some(verb),
            ..self
        }
    }

    /// The binding listed on a help page under the heading `section`,
    /// saying `text`: one sentence, as a label reads.
    #[must_use]
    pub const fn help(self, section: &'static str, text: &'static str) -> Self {
        Self {
            section,
            help: text,
            ..self
        }
    }

    /// The binding's hint ranked `rank` in a line of hints that gives way
    /// by priority: the lower the rank, the sooner it goes.
    #[must_use]
    pub const fn rank(self, rank: u32) -> Self {
        Self {
            rank: Some(rank),
            ..self
        }
    }

    /// The binding able to act only when `can_act` says so of the host's
    /// context. The key resolves either way, so the host can say why
    /// nothing happened; a status bar leaves out a binding that cannot act
    /// now, and a help page generated with a context dims it.
    #[must_use]
    pub const fn available_when(self, can_act: fn(&Cx) -> bool) -> Self {
        Self {
            available: Some(can_act),
            ..self
        }
    }

    /// The binding's stable id.
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

/// One binding of a built [`Keymap`]: a [`Bind`] with every key it binds,
/// those of a control's own table included.
pub struct Binding<A: 'static, Cx: 'static = ()> {
    id: &'static str,
    scope: Scope,
    keys: Vec<KeyMatcher>,
    action: A,
    verb: Option<&'static str>,
    section: &'static str,
    help: &'static str,
    rank: Option<u32>,
    available: Option<fn(&Cx) -> bool>,
}

impl<A: Clone, Cx> Clone for Binding<A, Cx> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            scope: self.scope,
            keys: self.keys.clone(),
            action: self.action.clone(),
            verb: self.verb,
            section: self.section,
            help: self.help,
            rank: self.rank,
            available: self.available,
        }
    }
}

impl<A: fmt::Debug, Cx> fmt::Debug for Binding<A, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Binding")
            .field("id", &self.id)
            .field("scope", &self.scope)
            .field("keys", &self.keys)
            .field("action", &self.action)
            .field("verb", &self.verb)
            .field("section", &self.section)
            .field("help", &self.help)
            .field("rank", &self.rank)
            .field("available", &self.available.is_some())
            .finish()
    }
}

impl<A: Copy, Cx> From<&Bind<A, Cx>> for Binding<A, Cx> {
    fn from(bind: &Bind<A, Cx>) -> Self {
        Self {
            id: bind.id,
            scope: bind.scope,
            keys: bind.keys.to_vec(),
            action: bind.action,
            verb: bind.verb,
            section: bind.section,
            help: bind.help,
            rank: bind.rank,
            available: bind.available,
        }
    }
}

impl<A: Copy, Cx> Binding<A, Cx> {
    /// The stable id: what a remap or a help annotation names.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// The scope the binding applies in.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Every key the binding binds; the first is the one a hint names.
    pub fn keys(&self) -> &[KeyMatcher] {
        &self.keys
    }

    /// What the binding stands for.
    pub fn action(&self) -> A {
        self.action
    }

    /// What its hint says its first key does; `None` keeps it out of
    /// hints.
    pub fn verb(&self) -> Option<&'static str> {
        self.verb
    }

    /// The heading of the help section it is listed under.
    pub fn section(&self) -> &'static str {
        self.section
    }

    /// What its help row says it does.
    pub fn help(&self) -> &'static str {
        self.help
    }

    /// The rank its hint holds on at, `None` when it has none of its own.
    pub fn rank(&self) -> Option<u32> {
        self.rank
    }

    /// Whether it can act now in the host's context `cx`: true unless its
    /// predicate says otherwise.
    pub fn can_act(&self, cx: &Cx) -> bool {
        self.available.is_none_or(|can_act| can_act(cx))
    }

    /// Whether `event` fires one of its keys.
    pub fn matches(&self, event: KeyEvent) -> bool {
        self.keys.iter().any(|key| key.matches_event(event))
    }
}

/// What makes a keymap's bindings unusable together, found when it is
/// built ([`KeymapBuilder::build`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Two bindings share an id, so a remap or an annotation could not
    /// tell them apart.
    DuplicateId(&'static str),
    /// An id that is empty or holds a character other than a lower-case
    /// letter, a digit, `.`, or `-`.
    MalformedId(&'static str),
    /// A binding with no key to press.
    NoKeys(&'static str),
    /// A binding with no help section or text, which its help row would
    /// leave blank.
    Undescribed(&'static str),
    /// Two keys of one scope that one key event fires: whichever comes
    /// second never would. `first` and `second` are the bindings' ids (the
    /// same id twice when a binding repeats itself), `key` the event both
    /// take, as [`key_label`] writes it.
    Overlap {
        /// The scope both keys are bound in.
        scope: Scope,
        /// The binding that comes first.
        first: &'static str,
        /// The binding that comes second.
        second: &'static str,
        /// A key event both fire on.
        key: String,
    },
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Conflict::DuplicateId(id) => write!(f, "two bindings have the id `{id}`"),
            Conflict::MalformedId(id) => write!(
                f,
                "the id `{id}` is not lower-case letters, digits, `.`, and `-`"
            ),
            Conflict::NoKeys(id) => write!(f, "`{id}` binds no key"),
            Conflict::Undescribed(id) => write!(f, "`{id}` has no help section or text"),
            Conflict::Overlap {
                scope,
                first,
                second,
                key,
            } if first == second => write!(f, "`{first}` binds `{key}` twice in {scope}"),
            Conflict::Overlap {
                scope,
                first,
                second,
                key,
            } => write!(
                f,
                "`{first}` and `{second}` both take `{key}` in {scope}, so `{second}` never fires on it"
            ),
        }
    }
}

/// Every [`Conflict`] a keymap's bindings have, in the order they were
/// found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflicts(Vec<Conflict>);

impl Conflicts {
    /// The conflicts, one per problem.
    pub fn conflicts(&self) -> &[Conflict] {
        &self.0
    }
}

impl fmt::Display for Conflicts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, conflict) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{conflict}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Conflicts {}

/// A program's own keys: bindings in scopes, looked up in the scopes that
/// have the keys now, with no two keys of one scope that one key event
/// fires.
///
/// Because no key of a scope shadows another, the order of the bindings
/// decides nothing about what a key does: it is the order a help page
/// lists them in (sections in the order they first appear) and a status
/// bar names them in. Built once, with [`Keymap::builder`].
///
/// ```
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use tuikit::keys::{ActiveScopes, Bind, Keymap, Scope, bind, ch, key};
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// enum Do {
///     Quit,
///     Next,
///     Open,
/// }
///
/// const KEYS: &[Bind<Do>] = &[
///     bind("quit", Scope::Global, &[ch('q')], Do::Quit).help("Program", "Quit"),
///     bind("list.next", Scope::Pane("list"), &[key(KeyCode::Down), ch('j')], Do::Next)
///         .verb("move")
///         .help("List", "Move to the next row"),
///     bind("file.open", Scope::Pane("file"), &[key(KeyCode::Down)], Do::Open)
///         .help("File", "Open the next file"),
/// ];
/// let keymap = Keymap::builder().rows(KEYS).build().expect("no two keys collide");
///
/// // The focused pane's scope first, then the global one.
/// let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
/// let found = |active| keymap.resolve(active, down).map(|binding| binding.action());
/// assert_eq!(found(ActiveScopes::pane("list")), Some(Do::Next));
/// assert_eq!(found(ActiveScopes::pane("file")), Some(Do::Open));
/// let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
/// assert_eq!(keymap.resolve(ActiveScopes::pane("list"), q).map(|b| b.id()), Some("quit"));
///
/// // Two keys of one scope that one event fires fail the build.
/// const CLASH: &[Bind<Do>] = &[
///     bind("a", Scope::Global, &[key(KeyCode::Enter)], Do::Open).help("x", "Open"),
///     bind("b", Scope::Global, &[key(KeyCode::Enter)], Do::Next).help("x", "Next"),
/// ];
/// let conflicts = Keymap::builder().rows(CLASH).build().unwrap_err();
/// assert_eq!(
///     conflicts.to_string(),
///     "`a` and `b` both take `Enter` in the global scope, so `b` never fires on it"
/// );
/// ```
pub struct Keymap<A: 'static, Cx: 'static = ()> {
    bindings: Vec<Binding<A, Cx>>,
}

impl<A: fmt::Debug, Cx> fmt::Debug for Keymap<A, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keymap")
            .field("bindings", &self.bindings)
            .finish()
    }
}

impl<A: Copy + PartialEq, Cx> Keymap<A, Cx> {
    /// A builder with no bindings yet.
    pub fn builder() -> KeymapBuilder<A, Cx> {
        KeymapBuilder {
            bindings: Vec::new(),
        }
    }

    /// Every binding, in the order they were declared.
    pub fn bindings(&self) -> &[Binding<A, Cx>] {
        &self.bindings
    }

    /// The binding with the id `id`.
    pub fn get(&self, id: &str) -> Option<&Binding<A, Cx>> {
        self.bindings.iter().find(|binding| binding.id == id)
    }

    /// The binding `event` fires in the scopes that have the keys: the
    /// first scope of `active` that binds it decides. `None` for a key no
    /// active scope binds. Availability does not change the answer.
    pub fn resolve(&self, active: ActiveScopes, event: KeyEvent) -> Option<&Binding<A, Cx>> {
        active.scopes().find_map(|scope| {
            self.bindings
                .iter()
                .find(|binding| binding.scope == scope && binding.matches(event))
        })
    }

    /// Whether `binding`'s first key resolves to it in `active`: the key
    /// its hint names does what the hint says there, and no scope looked
    /// up before its own takes the key.
    pub fn first_key_reaches(&self, active: ActiveScopes, binding: &Binding<A, Cx>) -> bool {
        let Some(first) = binding.keys.first() else {
            return false;
        };
        active.contains(binding.scope)
            && self
                .resolve(active, first.example())
                .is_some_and(|found| found.id == binding.id)
    }

    /// The pairs of bindings in which a pane's or an overlay mode's binding
    /// takes a key of a global one while its scope is active, as `(the
    /// scoped binding's id, the global one's id, the key)`. Scopes take
    /// keys by design; a host lists the ones it means, so a new one is a
    /// choice and not an accident.
    pub fn overrides(&self) -> Vec<(&'static str, &'static str, String)> {
        let mut found = Vec::new();
        for scoped in self.bindings.iter().filter(|b| b.scope != Scope::Global) {
            for global in self.bindings.iter().filter(|b| b.scope == Scope::Global) {
                if let Some(event) = first_overlap(&scoped.keys, &global.keys) {
                    found.push((scoped.id, global.id, event_label(event)));
                }
            }
        }
        found
    }

    /// The keys that bind a character but ignore its Control or Alt
    /// chords, against the overlay rule for characters (see
    /// [`KeyMatcher`]), as `(the binding's id, the key)`: the keymap's
    /// [`loose_characters`](super::loose_characters). Empty when every
    /// character binds as typed and every chord is spelled out, which a
    /// host that holds its keymap to the overlays' rule checks in a test.
    pub fn loose_characters(&self) -> Vec<(&'static str, String)> {
        self.bindings
            .iter()
            .flat_map(|binding| {
                binding
                    .keys
                    .iter()
                    .filter(|key| key.ignores_chords_of_a_character())
                    .map(|key| (binding.id, key_label(key)))
            })
            .collect()
    }
}

/// The bindings of a [`Keymap`] being put together, in order.
pub struct KeymapBuilder<A: 'static, Cx: 'static = ()> {
    bindings: Vec<Binding<A, Cx>>,
}

impl<A: fmt::Debug, Cx> fmt::Debug for KeymapBuilder<A, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeymapBuilder")
            .field("bindings", &self.bindings)
            .finish()
    }
}

impl<A: Copy + PartialEq, Cx> KeymapBuilder<A, Cx> {
    /// The bindings `rows` declares, after those so far.
    #[must_use]
    pub fn rows(mut self, rows: &[Bind<A, Cx>]) -> Self {
        self.bindings.extend(rows.iter().map(Binding::from));
        self
    }

    /// A control's own key `table` taken in, after the bindings so far:
    /// one binding per action of the control the table binds, in the
    /// order its help lists them ([`HelpAction::LISTED`]). `describe`
    /// gives each its id, scope, the program's action, and its help (a
    /// [`Bind`] whose keys, if any, come first); the table gives every key
    /// bound to it, the keys it shows before its aliases, and, where
    /// `describe` gives none, the verb and rank of its first shown row. An
    /// action none of whose rows is available cannot act.
    ///
    /// This is how a pane control's standard table becomes the pane's
    /// scope without a copy of its rows.
    #[must_use]
    pub fn table<B: HelpAction>(
        mut self,
        table: &[KeyRow<B>],
        describe: impl Fn(B) -> Bind<A, Cx>,
    ) -> Self {
        let mut seen: Vec<B> = Vec::new();
        for &action in B::LISTED {
            if seen.contains(&action) {
                continue;
            }
            seen.push(action);
            let mut shown = Vec::new();
            let mut aliases = Vec::new();
            let mut verb = None;
            let mut rank = None;
            let mut available = false;
            for row in rows(table) {
                let KeyRow::Bind {
                    key,
                    action: bound,
                    hint,
                    available: now,
                    rank: own,
                } = row
                else {
                    continue;
                };
                if *bound != action {
                    continue;
                }
                available |= *now;
                match hint {
                    Hint::Shown { verb: words, .. } => {
                        if verb.is_none() {
                            verb = Some(*words);
                            rank = *own;
                        }
                        shown.push(*key);
                    }
                    Hint::Alias => aliases.push(*key),
                }
            }
            if shown.is_empty() && aliases.is_empty() {
                continue;
            }
            let mut binding = Binding::from(&describe(action));
            for key in shown.into_iter().chain(aliases) {
                if !binding.keys.contains(&key) {
                    binding.keys.push(key);
                }
            }
            binding.verb = binding.verb.or(verb);
            binding.rank = binding.rank.or(rank);
            if !available && binding.available.is_none() {
                binding.available = Some(never::<Cx>);
            }
            self.bindings.push(binding);
        }
        self
    }

    /// The keymap, or every conflict its bindings have: an id two bindings
    /// share or that is malformed, a binding with no key or no help, and
    /// any two keys of one scope that one key event fires (whichever
    /// comes second would be shadowed, so neither order is allowed).
    pub fn build(self) -> Result<Keymap<A, Cx>, Conflicts> {
        let mut found = Vec::new();
        for (index, binding) in self.bindings.iter().enumerate() {
            if !well_formed(binding.id) {
                found.push(Conflict::MalformedId(binding.id));
            }
            if self.bindings[..index]
                .iter()
                .any(|earlier| earlier.id == binding.id)
            {
                found.push(Conflict::DuplicateId(binding.id));
            }
            if binding.keys.is_empty() {
                found.push(Conflict::NoKeys(binding.id));
            }
            if binding.section.trim().is_empty() || binding.help.trim().is_empty() {
                found.push(Conflict::Undescribed(binding.id));
            }
        }
        // Every key of a scope, in order, with the binding it belongs to.
        let keys: Vec<(Scope, &'static str, KeyMatcher)> = self
            .bindings
            .iter()
            .flat_map(|binding| {
                binding
                    .keys
                    .iter()
                    .map(move |key| (binding.scope, binding.id, *key))
            })
            .collect();
        for (index, (scope, second, key)) in keys.iter().enumerate() {
            for (other_scope, first, earlier) in &keys[..index] {
                if other_scope != scope {
                    continue;
                }
                if let Some(event) = earlier.overlap(key) {
                    found.push(Conflict::Overlap {
                        scope: *scope,
                        first,
                        second,
                        key: event_label(event),
                    });
                }
            }
        }
        if found.is_empty() {
            Ok(Keymap {
                bindings: self.bindings,
            })
        } else {
            Err(Conflicts(found))
        }
    }
}

/// Whether `id` is lower-case letters, digits, `.`, and `-`, and not
/// empty: a name a configuration file can write as it is.
fn well_formed(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

/// The predicate of a binding that can never act: a control's action whose
/// every row is unavailable.
fn never<Cx>(_: &Cx) -> bool {
    false
}

/// A key event some key of `a` and some key of `b` both fire on.
fn first_overlap(a: &[KeyMatcher], b: &[KeyMatcher]) -> Option<KeyEvent> {
    a.iter()
        .find_map(|left| b.iter().find_map(|right| left.overlap(right)))
}

/// `event` as [`key_label`] writes a key.
fn event_label(event: KeyEvent) -> String {
    key_label(&KeyMatcher::Exact {
        modifiers: event.modifiers,
        code: event.code,
    })
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
