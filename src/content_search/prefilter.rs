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
