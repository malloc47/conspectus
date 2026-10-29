//! The path field's state: the line, what its source said about it, and
//! the completions ranked from that.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ratatui::crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::style::Style;

use super::source::{Entry, PathKind, PathSource, collapse_home, expand_home};
use super::view::PathFieldView;
use crate::feedback::Severity;
use crate::keys::{PATH_FIELD_KEYS, PathKey, resolve};
use crate::text_edit::{Edit, LineEdit};
use crate::theme::Theme;

/// What the mark of a field whose path has not been looked at shows.
const UNKNOWN_MARK: &str = "?";

/// A path the host knows and would offer before anything the source
/// lists: a recent directory, the one the user is working in.
///
/// A higher `priority` comes first; candidates of equal priority come in
/// path order. A candidate may be written with `~`. Its kind is not
/// checked: the host offers what it knows to be the right kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The path, absolute or starting with `~`.
    pub path: String,
    /// Where it ranks among the host's candidates: higher first.
    pub priority: i32,
}

impl Candidate {
    /// A candidate for `path` with `priority`.
    pub fn new(path: impl Into<String>, priority: i32) -> Self {
        Self {
            path: path.into(),
            priority,
        }
    }
}

/// One way to complete the typed path: the path the field would hold, of
/// which the typed text is the start.
///
/// A completion only ever adds to what was typed, so what `Tab` does and
/// what the hint shows are one thing: [`remainder`](Self::remainder).
/// Typed text that starts with `~` completes in that form (`~/src`), the
/// rest as the paths are written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    value: String,
    typed: usize,
}

impl Completion {
    /// The path the field holds once completed.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// What completing adds to the typed text.
    pub fn remainder(&self) -> &str {
        &self.value[self.typed..]
    }
}

/// What the field's last look at its source found for the typed path.
///
/// Advisory: the field takes any text, and a host commits a path that is
/// not there yet if it wants to. The validity is a mark beside the field,
/// not a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Validity {
    /// Nothing is typed, so there is nothing to check.
    Unknown,
    /// The path is there and is the kind the field asks for.
    Exists,
    /// Something is typed, but nothing is there, or what is there is the
    /// other kind (a file where the field asks for a directory).
    Advisory,
}

impl Validity {
    /// The mark drawn beside the field: `?` for nothing typed, and the
    /// success and warning marks of [`Severity`] for a path that is there
    /// and one that is not.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Unknown => UNKNOWN_MARK,
            Self::Exists => Severity::Success.mark(),
            Self::Advisory => Severity::Warning.mark(),
        }
    }

    /// The mark's style: `inert` for nothing typed, and the `success` and
    /// `warning` colors.
    pub fn style(self, theme: &Theme) -> Style {
        match self {
            Self::Unknown => Style::default().fg(theme.inert),
            Self::Exists => Severity::Success.style(theme),
            Self::Advisory => Severity::Warning.style(theme),
        }
    }
}

/// The directory the field last listed, and the entries of the kind it
/// completes with, in name order.
#[derive(Clone, Debug)]
struct Listing {
    /// The directory as the expanded text names it, ending in `/`.
    dir: String,
    entries: Vec<Entry>,
}

/// A text field for a path: a [`LineEdit`] that completes the path it
/// holds and says whether it names something of the kind it asks for.
///
/// - **A source.** The field reads the file system only through the
///   [`PathSource`] its host gives it (shared, so several fields can
///   share one), never through the process's own.
/// - **A kind.** A field asks for a [`PathKind`]: a directory field
///   completes with directories only, and a file there is
///   [`Advisory`](Validity::Advisory), not [`Exists`](Validity::Exists).
///   A file field completes with files and with the directories that lead
///   to them.
/// - **Completion.** The [candidates](Self::candidates) are every path
///   that extends the typed text: the host's known [`Candidate`]s first,
///   by priority, then the source's entries of the typed path's directory
///   whose names start with its last component, in name order. `Tab`
///   ([`PATH_FIELD_KEYS`]) takes the first: the field adds its
///   [remainder](Completion::remainder). Nothing is typed: every known
///   candidate completes, and the source lists nothing.
/// - **`~`.** A leading `~` is the source's home directory
///   ([`expand_home`]), for checking, for completing (typed text that
///   starts with `~` completes in that form), and in
///   [`expanded_value`](Self::expanded_value), which is what a host
///   commits.
/// - **Looked at once.** The field asks its source when it is built, when
///   its text changes, and when told to [`refresh`](Self::refresh), and
///   keeps the answers: what is at the path, the home directory, and the
///   listing of the directory being typed in (kept while typing in that
///   directory's names, so only a change of directory lists again).
///   Reading the validity, the candidates, or the expanded value, and
///   drawing ([`view`](Self::view)), read only what it kept.
///
/// ```
/// use std::path::{Path, PathBuf};
/// use std::sync::Arc;
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use tuikit::path_field::{Entry, PathField, PathKind, PathSource, Validity};
///
/// /// A home with two directories and a file in it.
/// struct Home;
///
/// impl PathSource for Home {
///     fn entries(&self, dir: &Path) -> Vec<Entry> {
///         if dir == Path::new("/home/me/") {
///             vec![
///                 Entry::new("notes.txt", PathKind::File),
///                 Entry::new("photos", PathKind::Dir),
///                 Entry::new("projects", PathKind::Dir),
///             ]
///         } else {
///             Vec::new()
///         }
///     }
///     fn kind(&self, path: &Path) -> Option<PathKind> {
///         match path.to_str()? {
///             "/home/me" | "/home/me/photos" | "/home/me/projects" => Some(PathKind::Dir),
///             "/home/me/notes.txt" => Some(PathKind::File),
///             _ => None,
///         }
///     }
///     fn home(&self) -> Option<PathBuf> {
///         Some(PathBuf::from("/home/me"))
///     }
/// }
///
/// let mut field = PathField::new("~/proj", PathKind::Dir, Arc::new(Home));
/// assert_eq!(field.validity(), Validity::Advisory);
/// assert_eq!(field.completion().map(|c| c.remainder()), Some("ects"));
///
/// let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
/// assert!(field.handle(tab).changed);
/// assert_eq!(field.value(), "~/projects");
/// assert_eq!(field.validity(), Validity::Exists);
/// assert_eq!(field.expanded_value(), "/home/me/projects");
///
/// // A file does not pass where a directory is asked for.
/// field.set_value("~/notes.txt");
/// assert_eq!(field.validity(), Validity::Advisory);
/// ```
#[derive(Clone)]
pub struct PathField {
    edit: LineEdit,
    kind: PathKind,
    source: Arc<dyn PathSource + Send + Sync>,
    /// The host's candidates, trimmed, highest priority first, each path
    /// once.
    known: Vec<Candidate>,
    home: Option<PathBuf>,
    /// What the source said is at the typed path.
    found: Option<PathKind>,
    listing: Option<Listing>,
    completions: Vec<Completion>,
}

impl PathField {
    /// A field holding `value`, with the cursor at its end, that asks for
    /// a `kind` and looks paths up in `source`. It looks at `value` now.
    pub fn new(
        value: impl Into<String>,
        kind: PathKind,
        source: Arc<dyn PathSource + Send + Sync>,
    ) -> Self {
        let mut field = Self {
            edit: LineEdit::new(value),
            kind,
            source,
            known: Vec::new(),
            home: None,
            found: None,
            listing: None,
            completions: Vec::new(),
        };
        field.look(true);
        field
    }

    /// The same field offering `candidates` first.
    #[must_use]
    pub fn with_candidates(mut self, candidates: Vec<Candidate>) -> Self {
        self.set_candidates(candidates);
        self
    }

    /// Offer `candidates` before what the source lists, in place of the
    /// ones offered before. Ranks again without asking the source.
    pub fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        let mut known: Vec<Candidate> = candidates
            .into_iter()
            .filter_map(|candidate| {
                let path = candidate.path.trim();
                (!path.is_empty()).then(|| Candidate::new(path, candidate.priority))
            })
            .collect();
        known.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.path.cmp(&right.path))
        });
        let mut seen = BTreeSet::new();
        known.retain(|candidate| seen.insert(candidate.path.clone()));
        self.known = known;
        self.rank();
    }

    /// The text, as typed.
    pub fn value(&self) -> &str {
        self.edit.value()
    }

    /// The line being edited, for drawing it (a
    /// [`TextField`](crate::text_edit::TextField)) or reading its cursor.
    pub fn edit(&self) -> &LineEdit {
        &self.edit
    }

    /// The kind of path the field asks for.
    pub fn kind(&self) -> PathKind {
        self.kind
    }

    /// Replace the text, with the cursor at its end, and say whether it
    /// changed. A change looks at the new path.
    pub fn set_value(&mut self, value: impl Into<String>) -> Edit {
        let edit = self.edit.set_value(value);
        if edit.changed {
            self.look(false);
        }
        edit
    }

    /// Handle one key: `Tab` ([`PATH_FIELD_KEYS`]) completes, and every
    /// other key edits the line ([`LineEdit::handle`]).
    ///
    /// A completion is a change. `Tab` with nothing to complete is not
    /// consumed ([`Edit::IGNORED`]), so the host can take it as moving on
    /// to the next field.
    pub fn handle(&mut self, key: KeyEvent) -> Edit {
        if key.kind != KeyEventKind::Release
            && resolve(PATH_FIELD_KEYS, key) == Some(PathKey::Complete)
        {
            return if self.complete() {
                Edit {
                    changed: true,
                    consumed: true,
                }
            } else {
                Edit::IGNORED
            };
        }
        let edit = self.edit.handle(key);
        if edit.changed {
            self.look(false);
        }
        edit
    }

    /// Take the best completion: the text becomes its value, with the
    /// cursor at the end. `false` when there is nothing to complete.
    pub fn complete(&mut self) -> bool {
        let Some(best) = self.completions.first() else {
            return false;
        };
        let value = best.value.clone();
        self.set_value(value);
        true
    }

    /// Look at the path again: ask the source for the home directory,
    /// what is at the path, and the listing, dropping what the field
    /// kept. For a host that knows the file system changed under the
    /// field (it created the directory, or it refreshes on a timer).
    pub fn refresh(&mut self) {
        self.look(true);
    }

    /// Whether the typed path is there and of the field's kind, as the
    /// last look found it.
    pub fn validity(&self) -> Validity {
        if self.edit.value().trim().is_empty() {
            Validity::Unknown
        } else if self.found == Some(self.kind) {
            Validity::Exists
        } else {
            Validity::Advisory
        }
    }

    /// What the last look found at the typed path, whatever the field's
    /// kind: `None` when nothing was there or nothing is typed. A host
    /// that wants to say why a path is advisory ("is a file") reads it.
    pub fn found(&self) -> Option<PathKind> {
        if self.edit.value().trim().is_empty() {
            None
        } else {
            self.found
        }
    }

    /// Every completion of the typed text, best first: the ranked list a
    /// host may show under the field. `Tab` takes the first.
    pub fn candidates(&self) -> &[Completion] {
        &self.completions
    }

    /// The best completion, the one `Tab` takes and the hint shows.
    pub fn completion(&self) -> Option<&Completion> {
        self.completions.first()
    }

    /// The path to commit: the text trimmed, with a leading `~` expanded
    /// to the home directory the last look found ([`expand_home`]).
    pub fn expanded_value(&self) -> String {
        expand_home(self.edit.value().trim(), self.home.as_deref())
            .to_string_lossy()
            .into_owned()
    }

    /// A drawing of the field: the text, the validity mark, and the
    /// completion hint ([`PathFieldView`]).
    pub fn view(&self) -> PathFieldView<'_> {
        PathFieldView::new(self)
    }

    /// Ask the source about the typed path, then rank. With `fresh`, the
    /// home and the listing are asked for again too; otherwise the
    /// listing is kept while the directory being typed in is the same.
    fn look(&mut self, fresh: bool) {
        if fresh {
            self.home = self.source.home();
            self.listing = None;
        }
        let typed = self.edit.value().trim();
        if typed.is_empty() {
            self.found = None;
        } else {
            let expanded = expand_home(typed, self.home.as_deref());
            self.found = self.source.kind(&expanded);
            let expanded = expanded.to_string_lossy();
            if let Some(dir) = directory_of(&expanded)
                && self
                    .listing
                    .as_ref()
                    .is_none_or(|listing| listing.dir != dir)
            {
                let kind = self.kind;
                let mut entries: Vec<Entry> = self
                    .source
                    .entries(Path::new(dir))
                    .into_iter()
                    .filter(|entry| {
                        !entry.name.is_empty()
                            && !entry.name.contains('/')
                            && (kind == PathKind::File || entry.kind == PathKind::Dir)
                    })
                    .collect();
                entries.sort_by(|left, right| left.name.cmp(&right.name));
                self.listing = Some(Listing {
                    dir: dir.to_string(),
                    entries,
                });
            }
        }
        self.rank();
    }

    /// Rank every completion of the typed text from what the field kept.
    fn rank(&mut self) {
        let typed = self.edit.value().trim();
        let home = self.home.as_deref();
        let tilde = typed == "~" || typed.starts_with("~/");
        let shown = |full: &str| {
            if tilde {
                collapse_home(full, home)
            } else {
                full.to_string()
            }
        };
        let mut seen = BTreeSet::new();
        let mut completions = Vec::new();
        let mut offer = |value: String| {
            if value.len() > typed.len() && value.starts_with(typed) && seen.insert(value.clone()) {
                completions.push(Completion {
                    value,
                    typed: typed.len(),
                });
            }
        };
        for candidate in &self.known {
            offer(shown(&expand_home(&candidate.path, home).to_string_lossy()));
        }
        if !typed.is_empty() {
            let expanded = expand_home(typed, home).to_string_lossy().into_owned();
            if let Some(dir) = directory_of(&expanded)
                && let Some(listing) = self.listing.as_ref().filter(|l| l.dir == dir)
            {
                let prefix = &expanded[dir.len()..];
                for entry in &listing.entries {
                    if entry.name.starts_with(prefix) {
                        offer(shown(&format!("{dir}{}", entry.name)));
                    }
                }
            }
        }
        self.completions = completions;
    }
}

impl fmt::Debug for PathField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathField")
            .field("value", &self.value())
            .field("kind", &self.kind)
            .field("validity", &self.validity())
            .field("candidates", &self.completions.len())
            .finish_non_exhaustive()
    }
}

/// The directory part of a path written out: everything up to and
/// including its last `/`, or `None` for a bare name.
fn directory_of(path: &str) -> Option<&str> {
    path.rfind('/').map(|index| &path[..=index])
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
