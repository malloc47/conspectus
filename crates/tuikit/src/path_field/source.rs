//! Where a path field looks things up: the read-only source its host
//! supplies, the two kinds of thing a path names, and the `~` rule.

use std::path::{Path, PathBuf};

/// What a path names: a directory, or a file, which is anything that is
/// not a directory (a regular file, a device, a socket).
///
/// A [`PathField`](super::PathField) asks for one kind: a field for a
/// working directory asks for [`Dir`](Self::Dir), so a file there does
/// not pass for one. A [`PathSource`] reports the kind of what it finds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    /// A directory.
    Dir,
    /// Anything that is not a directory.
    File,
}

/// One entry of a directory, as a [`PathSource`] lists it: its name (one
/// path component, with no separator) and what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The entry's name within its directory.
    pub name: String,
    /// What the entry is, following a link to what it names.
    pub kind: PathKind,
}

impl Entry {
    /// An entry named `name` of kind `kind`.
    pub fn new(name: impl Into<String>, kind: PathKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}

/// Read-only access to a file system: what a [`PathField`] completes and
/// checks a path against.
///
/// The framework reads no file system and no environment of its own. A
/// host supplies a source: over the real file system (listing with
/// `read_dir`, a kind from the metadata a link leads to, the home
/// directory from the environment), or an in-memory tree in tests, so a
/// test needs no real directory and no home. A source never writes.
///
/// A source answers for the path it is given as it is given: the field
/// expands a leading `~` itself, and passes a relative path on unchanged,
/// so what a relative path is relative to is the source's business.
///
/// [`PathField`]: super::PathField
pub trait PathSource {
    /// The entries of the directory `dir`, in any order: empty when `dir`
    /// is not a directory or cannot be read.
    fn entries(&self, dir: &Path) -> Vec<Entry>;

    /// What is at `path`, following links: `None` when nothing is there
    /// or it cannot be read.
    fn kind(&self, path: &Path) -> Option<PathKind>;

    /// The directory a leading `~` stands for, or `None` when there is
    /// none, in which case `~` stays as typed.
    fn home(&self) -> Option<PathBuf>;
}

/// `value` with a leading `~` or `~/` expanded to `home`; any other value,
/// and every value when there is no home, as it is.
///
/// Only the current user's home is expanded: `~name` (another user's) is
/// left alone, since a home directory for someone else cannot be found
/// portably. The rest of `~/rest` is joined to the home as a path, so
/// `~/src` is the home's `src`.
///
/// This is the one `~` rule: a path field validates, completes, and
/// commits with it, and a host that reads paths from its own files can
/// expand them the same way.
///
/// ```
/// use std::path::{Path, PathBuf};
/// use tuikit::path_field::expand_home;
///
/// let home = Some(Path::new("/home/me"));
/// assert_eq!(expand_home("~", home), PathBuf::from("/home/me"));
/// assert_eq!(expand_home("~/src", home), PathBuf::from("/home/me/src"));
/// assert_eq!(expand_home("~other/src", home), PathBuf::from("~other/src"));
/// assert_eq!(expand_home("~/src", None), PathBuf::from("~/src"));
/// ```
pub fn expand_home(value: &str, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from(value);
    };
    if value == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(value)
}

/// `path` with the home directory at its start written as `~`: the home
/// itself is `~`, and anything under it `~/rest`. Any other path, and every
/// path when there is no home, as it is. The inverse of [`expand_home`]
/// for the paths it changes.
///
/// ```
/// use std::path::Path;
/// use tuikit::path_field::collapse_home;
///
/// let home = Some(Path::new("/home/me"));
/// assert_eq!(collapse_home("/home/me/src", home), "~/src");
/// assert_eq!(collapse_home("/home/me", home), "~");
/// assert_eq!(collapse_home("/home/meadow", home), "/home/meadow");
/// ```
pub fn collapse_home(path: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return path.to_string();
    };
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".to_string();
    }
    match path
        .strip_prefix(home)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    }
}
