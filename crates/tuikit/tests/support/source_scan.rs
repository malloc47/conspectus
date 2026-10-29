//! Scan plumbing the framework's source guards share.
//!
//! Each guard test includes this file with `#[path]`, so the guards walk the
//! crate, strip comments, literals, and test code, and report findings the
//! same way. A guard is a pure function over [`Source`] values built from
//! the real tree by [`load_sources`], so its self-tests can feed it a
//! leaking snippet from memory instead of a fixture on disk. The rules each
//! guard enforces, and the process for a reviewed exception, are in the
//! crate's contribution guide, `docs/contributing.md`.
//!
//! The mechanism (the walk and the Rust lexer that blanks comments,
//! literals, and test items) is the test kit's `testkit::source`, which the
//! host's hygiene scans use too; this file keeps the guards' policy types
//! and report format.
// Each guard uses its own part of this file.
#![allow(dead_code, unused_imports)]

use std::fs;
use std::path::Path;

/// `text` with comments and string and character literals blanked, line
/// breaks kept.
pub use tuikit::testkit::source::code as sanitize_rust;
pub use tuikit::testkit::source::{contains_at_word_start, is_identifier_char, strip_test_items};

/// One reviewed exception: `term` may appear in `path` (crate-relative,
/// forward slashes). `term` is whatever the scan matches: a listed word for
/// the vocabulary scan, a needle's text for the needle scans.
pub struct Exception {
    pub path: &'static str,
    pub term: &'static str,
    pub reason: &'static str,
}

/// A piece of source text a scan looks for, and why the framework may not
/// use it.
pub struct Needle {
    pub text: &'static str,
    pub why: &'static str,
}

/// One file to scan: a crate-relative path with forward slashes, and its text.
pub struct Source {
    pub path: String,
    pub text: String,
}

impl Source {
    pub fn new(path: &str, text: &str) -> Self {
        Self {
            path: path.to_owned(),
            text: text.to_owned(),
        }
    }
}

/// What a scan saw and what it found.
pub struct Scan {
    pub visited: usize,
    pub violations: Vec<String>,
}

/// A message when `visited` is below `minimum`, which means the scan was
/// pointed at the wrong place.
pub fn tripwire(scan: &str, visited: usize, minimum: usize) -> Option<String> {
    (visited < minimum).then(|| {
        format!(
            "the {scan} scan visited {visited} file(s), fewer than the stated minimum of \
             {minimum}: it is probably scanning the wrong directory"
        )
    })
}

/// `Err` with a report when a scan found violations or tripped its minimum.
pub fn verdict(name: &str, scan: &Scan, minimum: usize) -> Result<(), String> {
    let mut problems = scan.violations.clone();
    problems.extend(tripwire(name, scan.visited, minimum));
    if problems.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the {name} guard failed:\n{}\n\nFix the design first. To request a \
         reviewed exception, follow the process in crates/tuikit/docs/contributing.md.",
        problems.join("\n")
    ))
}

/// Fails the test when a scan found violations or tripped its minimum.
pub fn assert_clean(name: &str, scan: &Scan, minimum: usize) {
    verdict(name, scan, minimum).unwrap_or_else(|report| panic!("{report}"));
}

pub fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every file under `root`, sorted, skipping `target` and hidden directories
/// and anything that is not a plain file or directory.
pub fn load_sources(root: &Path) -> Vec<Source> {
    tuikit::testkit::source::files(root)
        .into_iter()
        .map(|path| {
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let relative = path
                .strip_prefix(root)
                .expect("walked paths stay under the root")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            Source {
                path: relative,
                text: String::from_utf8_lossy(&bytes).into_owned(),
            }
        })
        .collect()
}

/// Marks each exception that matched something, and reports the ones that
/// matched nothing: a stale exception hides nothing, so it must go.
pub fn stale_exceptions(scan: &str, exceptions: &[Exception], used: &[bool]) -> Vec<String> {
    exceptions
        .iter()
        .zip(used)
        .filter(|(_, used)| !**used)
        .map(|(exception, _)| {
            format!(
                "stale {scan} exception for `{}` in {} matches nothing (reason was: {}); remove it",
                exception.term, exception.path, exception.reason
            )
        })
        .collect()
}

pub fn is_rust_file(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| ext == "rs")
}

/// Test code by name: `tests.rs`, `*_tests.rs`, or anything in a `tests`
/// directory (the crate's own `tests/` included).
pub fn is_test_path(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    let file = parts.pop().unwrap_or_default();
    parts.contains(&"tests") || file == "tests.rs" || file.ends_with("_tests.rs")
}
