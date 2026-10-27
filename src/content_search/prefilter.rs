//! A cheap byte-level test that rules out transcripts which can't match.
//!
//! A transcript is JSON on disk, so a phrase in a message may be stored
//! escaped: quotes, backslashes, control characters, and `/` are written
//! with a backslash, and a producer may write non-ASCII as `\uXXXX`.
//! Comparing the query to the raw bytes would miss those matches. The
//! filter therefore only trusts runs of characters JSON never escapes
//! (`[A-Za-z0-9_.-]`): such a run, if the query matches a turn body,
//! appears unbroken in the file, whatever surrounds it. A file that
//! lacks any one of the query's runs can't match and is skipped without
//! being parsed. A file that has them all is only a candidate, and the
//! full parse decides.
//!
//! The filter lowercases ASCII only, matching the matcher's folding. When
//! the query has no usable run (it is all spaces and punctuation, or the
//! runs are too short to help) there is no filter and every file is parsed.

use std::io::{self, Read};
use std::path::Path;

/// Bytes read at a time when checking a file: memory per worker stays
/// bounded however large the transcript is.
const CHUNK: usize = 1 << 20;

/// Runs shorter than this don't rule out enough files to be worth a pass.
const MIN_RUN: usize = 3;

/// At most this many runs are checked: the longest ones narrow the most.
const MAX_RUNS: usize = 3;

/// The lowercase runs of unescapable characters to look for, longest
/// first. Empty when no filtering is possible.
pub fn safe_runs(needle: &str) -> Vec<String> {
    let lowered = needle.to_ascii_lowercase();
    let mut runs: Vec<String> = lowered
        .split(|c: char| !is_unescaped(c))
        .filter(|run| run.len() >= MIN_RUN)
        .map(str::to_string)
        .collect();
    runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
    runs.dedup();
    runs.truncate(MAX_RUNS);
    runs
}

/// Characters a JSON string stores as themselves in any encoder.
fn is_unescaped(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// Whether `haystack` contains every run, ignoring ASCII case.
pub fn contains_all(haystack: &[u8], runs: &[String]) -> bool {
    runs.iter()
        .all(|run| contains_ignore_ascii_case(haystack, run.as_bytes()))
}

/// Whether `haystack` contains `needle` (already lowercase), ignoring
/// ASCII case.
pub fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };
    let upper = first.to_ascii_uppercase();
    let mut start = 0;
    while let Some(offset) = haystack[start..]
        .iter()
        .position(|&byte| byte == first || byte == upper)
    {
        let at = start + offset;
        let tail = &haystack[at + 1..];
        if tail.len() >= rest.len()
            && tail
                .iter()
                .zip(rest)
                .all(|(byte, want)| byte.to_ascii_lowercase() == *want)
        {
            return true;
        }
        start = at + 1;
    }
    false
}

/// Whether the file at `path` may contain a match for `runs`, reading it
/// in bounded chunks: `false` means it cannot and need not be parsed.
///
/// `true` when every run is present (ignoring ASCII case), and also when
/// the file spells out an unescapable character as a `\u00XX` escape
/// (`\u006e` for `n`): a producer that does that hides the runs from a
/// byte comparison, so the file has to be parsed. A run that straddles two
/// chunks is found because the tail of each chunk is carried into the
/// next.
pub fn file_may_match(path: &Path, runs: &[String]) -> io::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let overlap = runs
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(0)
        .saturating_sub(1)
        .max(ESCAPE_LEN - 1);
    let mut found = vec![false; runs.len()];
    let mut carry: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        let mut window = std::mem::take(&mut carry);
        window.extend_from_slice(&chunk[..read]);
        if has_escaped_unescapable(&window) {
            return Ok(true);
        }
        for (run, seen) in runs.iter().zip(&mut found) {
            if !*seen && contains_ignore_ascii_case(&window, run.as_bytes()) {
                *seen = true;
            }
        }
        if found.iter().all(|seen| *seen) {
            return Ok(true);
        }
        let keep = overlap.min(window.len());
        carry = window[window.len() - keep..].to_vec();
    }
    Ok(found.iter().all(|seen| *seen))
}

/// Length of a `\u00XX` escape.
const ESCAPE_LEN: usize = 6;

/// Whether `bytes` contain a `\u00XX` escape of a character the filter
/// treats as unescaped (a letter, digit, `_`, `.`, or `-`).
fn has_escaped_unescapable(bytes: &[u8]) -> bool {
    let mut from = 0;
    while let Some(at) = bytes[from..]
        .windows(4)
        .position(|window| window == b"\\u00")
    {
        let start = from + at;
        if let Some(digits) = bytes.get(start + 4..start + 6)
            && let Some(value) = std::str::from_utf8(digits)
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            && is_unescaped(char::from(value))
        {
            return true;
        }
        from = start + 1;
    }
    false
}
