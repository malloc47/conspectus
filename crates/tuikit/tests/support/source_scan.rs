//! Scan plumbing the framework's source guards share.
//!
//! Each guard test includes this file with `#[path]`, so the guards walk the
//! crate, strip comments, literals, and test code, and report findings the
//! same way. A guard is a pure function over [`Source`] values built from
//! the real tree by [`load_sources`], so its self-tests can feed it a
//! leaking snippet from memory instead of a fixture on disk. The rules each
//! guard enforces, and the process for a reviewed exception, are in the
//! crate's contribution guide, `docs/contributing.md`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

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
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name != "target" && !name.starts_with('.') {
                    walk(&entry.path(), out);
                }
            } else if kind.is_file() {
                out.push(entry.path());
            }
        }
    }

    let mut paths = Vec::new();
    walk(root, &mut paths);
    paths.sort();
    paths
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

fn blank(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

pub fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Index just past the string literal whose body starts at `from`.
fn string_end(chars: &[char], from: usize) -> usize {
    let mut index = from;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '"' => return index + 1,
            _ => index += 1,
        }
    }
    chars.len()
}

/// Index just past the (possibly nested) block comment starting at `start`.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        if chars[index..].starts_with(&['/', '*']) {
            depth += 1;
            index += 2;
        } else if chars[index..].starts_with(&['*', '/']) {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    chars.len()
}

/// Index just past a raw string literal starting at the `r` at `start`, or
/// `None` when the `r` begins something else (an identifier, `r#ident`).
fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    let after_prefix = start == 0
        || !is_identifier_char(chars[start - 1])
        || (chars[start - 1] == 'b' && (start == 1 || !is_identifier_char(chars[start - 2])));
    if !after_prefix {
        return None;
    }
    let hashes = chars[start + 1..].iter().take_while(|c| **c == '#').count();
    if chars.get(start + 1 + hashes) != Some(&'"') {
        return None;
    }
    let closing: Vec<char> = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    let end = (start + 2 + hashes..chars.len())
        .find(|&index| chars[index..].starts_with(&closing))
        .map_or(chars.len(), |index| index + closing.len());
    Some(end)
}

/// Index just past the character literal starting at `start`, or `None` for
/// a lifetime.
fn char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start + 1) == Some(&'\\') {
        let tail = chars.get(start + 3..)?;
        let close = tail.iter().position(|c| *c == '\'')?;
        Some(start + 3 + close + 1)
    } else if chars.get(start + 2) == Some(&'\'') {
        Some(start + 3)
    } else {
        None
    }
}

/// Index just past the comment or literal that starts at `index`, if one does.
fn skipped_end(chars: &[char], index: usize) -> Option<usize> {
    let second = chars.get(index + 1).copied();
    match chars[index] {
        '/' if second == Some('/') => Some(
            chars[index..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(chars.len(), |offset| index + offset),
        ),
        '/' if second == Some('*') => Some(block_comment_end(chars, index)),
        '"' => Some(string_end(chars, index + 1)),
        'r' => raw_string_end(chars, index),
        '\'' => char_literal_end(chars, index),
        _ => None,
    }
}

/// `text` with comments and string and character literals blanked out, line
/// breaks kept so line numbers still match.
pub fn sanitize_rust(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = skipped_end(&chars, index) {
            out.extend(chars[index..end].iter().map(|c| blank(*c)));
            index = end;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

/// The attribute that gates an item to test builds.
const TEST_GATE: &str = "#[cfg(test)]";

/// Index just past the item that begins at `from`: through its closing brace,
/// or through the `;` of a body-less item such as `mod tests;`.
fn item_end(chars: &[char], from: usize) -> usize {
    let mut nesting = 0i32;
    let mut index = from;
    while index < chars.len() {
        match chars[index] {
            '(' | '[' => nesting += 1,
            ')' | ']' => nesting -= 1,
            ';' if nesting == 0 => return index + 1,
            '{' if nesting == 0 => {
                let mut depth = 0i32;
                for (offset, c) in chars[index..].iter().enumerate() {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    if depth == 0 {
                        return index + offset + 1;
                    }
                }
                return chars.len();
            }
            _ => {}
        }
        index += 1;
    }
    chars.len()
}

/// Blanks every `#[cfg(test)]` item out of sanitized source. Expects text
/// that already went through `sanitize_rust`, so braces in literals are gone.
pub fn strip_test_items(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    let gate: Vec<char> = TEST_GATE.chars().collect();
    let mut out = chars.clone();
    let mut index = 0;
    while index < chars.len() {
        if chars[index..].starts_with(&gate) {
            let end = item_end(&chars, index + gate.len());
            for c in &mut out[index..end] {
                *c = blank(*c);
            }
            index = end;
        } else {
            index += 1;
        }
    }
    out.into_iter().collect()
}

/// Whether `needle` occurs in `line` starting at a word start.
pub fn contains_at_word_start(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(index, _)| {
        line[..index]
            .chars()
            .next_back()
            .is_none_or(|c| !is_identifier_char(c))
    })
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
