//! Path fields: a text field for a file or directory path that completes
//! it, checks it, and expands `~`, looking things up only through a
//! source its host supplies.
//!
//! This is a layer 1 control (house rule 16). A program that asks for
//! paths in several places (a working directory, a location to save to,
//! a project root) should complete, check, and expand them the same way,
//! or a field suggests files where it wants a directory, a mark says a
//! path is fine that the command then refuses, and `~/src` is checked as
//! the home's `src` but committed as typed.
//!
//! - **[`PathField`]** holds the line ([`LineEdit`](crate::text_edit::LineEdit))
//!   and asks for a [`PathKind`], a directory or a file. It completes
//!   with the host's known [`Candidate`]s first, by priority, then with
//!   what the source lists in the typed path's directory; `Tab`
//!   ([`PATH_FIELD_KEYS`]) adds the best [`Completion`]'s remainder.
//!   [`PathField::candidates`] is the whole ranked list, for a host that
//!   shows it.
//! - **[`PathSource`]** is the read-only file system the field sees: a
//!   directory's [`Entry`]s, the kind of what is at a path, and the home
//!   directory. The host supplies one over the real file system; a test
//!   supplies an in-memory tree (the test kit's `MemoryPaths`). The field
//!   asks it only when its text changes or it is told to
//!   [`refresh`](PathField::refresh), so drawing does no I/O.
//! - **[`Validity`]** is the advisory mark: nothing typed, there and of
//!   the right kind, or not. A path that is not there is still taken.
//! - **`~`**: [`expand_home`] is the one rule, which the field checks,
//!   completes, and commits ([`PathField::expanded_value`]) with, and
//!   [`collapse_home`] its inverse.
//! - **[`PathFieldView`]** draws a field in one row: the text, the mark,
//!   and, while the field is focused and a completion exists, a
//!   `Tab: <remainder>` hint in a bounded width.
//!
//! ```
//! use std::path::{Path, PathBuf};
//! use std::sync::Arc;
//! use tuikit::path_field::{Candidate, Entry, PathField, PathKind, PathSource};
//! use tuikit::theme::Theme;
//!
//! /// A file system with nothing on it but a home directory.
//! struct Empty;
//!
//! impl PathSource for Empty {
//!     fn entries(&self, _dir: &Path) -> Vec<Entry> {
//!         Vec::new()
//!     }
//!     fn kind(&self, path: &Path) -> Option<PathKind> {
//!         (path == Path::new("/home/me")).then_some(PathKind::Dir)
//!     }
//!     fn home(&self) -> Option<PathBuf> {
//!         Some(PathBuf::from("/home/me"))
//!     }
//! }
//!
//! // Known paths rank first: the higher priority completes.
//! let field = PathField::new("/srv/", PathKind::Dir, Arc::new(Empty)).with_candidates(vec![
//!     Candidate::new("/srv/web", 10),
//!     Candidate::new("/srv/api", 20),
//! ]);
//! let best: Vec<&str> = field.candidates().iter().map(|c| c.value()).collect();
//! assert_eq!(best, ["/srv/api", "/srv/web"]);
//!
//! // Focused, with a completion: the text, the mark, and the hint, in
//! // exactly the width asked for.
//! let theme = Theme::default();
//! let row = field.view().focused(true).line(40, &theme);
//! let text: String = row.spans.iter().map(|span| span.content.as_ref()).collect();
//! assert_eq!(tuikit::text::width(&text), 40);
//! assert!(text.contains(" Tab: api"), "{text}");
//!
//! // Unfocused, the hint and its cells go to the text.
//! let row = field.view().focused(false).line(40, &theme);
//! let text: String = row.spans.iter().map(|span| span.content.as_ref()).collect();
//! assert!(!text.contains("Tab:"), "{text}");
//! ```
//!
//! # What a host keeps
//!
//! What the path is for: the field's label, which known paths to offer
//! and their priorities, what `Enter` commits, and whether a path that is
//! not there may be committed. The host also supplies the source, and
//! decides when the file system has changed enough to
//! [`refresh`](PathField::refresh).

mod field;
mod source;
mod view;

pub use field::{Candidate, Completion, PathField, Validity};
pub use source::{Entry, PathKind, PathSource, collapse_home, expand_home};
pub use view::PathFieldView;
// The completion key is a standard key table, kept with the others in
// `keys`; a path field's host reaches it here too.
pub use crate::keys::{PATH_FIELD_KEYS, PathKey};
