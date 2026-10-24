//! Content search over parsed transcripts (ADR 0147).
//!
//! One pure matcher shared by the `/` overlay, the scanner, the
//! viewer's find, and the `search` command, so every surface agrees on
//! what counts as a hit. Nothing here does I/O or starts a thread.
//!
//! Matching is one case-insensitive substring of the trimmed query.
//! Case folding is ASCII only: non-ASCII text matches exactly, because
//! Unicode folding can change byte lengths and would break highlight
//! ranges. A turn yields at most one [`Hit`], at its first match.
//!
//! Hits are payload on display (ADR 0086, Tier 2): a snippet is a
//! window around the match, length-capped, with whitespace collapsed and
//! control sequences removed.
//!
//! The viewer boundary applies (see [`crate::viewer`]): this module
//! imports nothing from the TUI, the graph model, or discovery.

// The scanner and the viewer's find use the rest of this API as they
// land (backlog `CSP-616.03`, `CSP-616.05`); until then only `/` and the
// config use part of it.
#![allow(dead_code)]

use std::cmp::Ordering;
use std::ops::Range;

use chrono::{DateTime, Utc};

use super::model::{TranscriptDocument, TranscriptTurn, TurnKind, TurnRole};

/// Longest snippet, in characters, before the `…` truncation markers.
pub const SNIPPET_MAX_CHARS: usize = 160;

/// How much raw text on each side of a match is normalized when
/// building a snippet, so a match in a multi-megabyte tool result
/// doesn't normalize the whole body.
const SNIPPET_WINDOW_CHARS: usize = SNIPPET_MAX_CHARS * 4;

/// How hits are presented: one result per session, or one per matching
/// turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Grouping {
    #[default]
    Session,
    Message,
}

impl Grouping {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "session" => Some(Self::Session),
            "message" => Some(Self::Message),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Message => "message",
        }
    }
}

/// Which turns are searched.
///
/// By default only what the viewer shows by default: user and assistant
/// messages and compaction summaries. Tool calls and output, and
/// thinking, are opt-in, and so are aborted and hidden turns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentKinds {
    /// User and assistant messages and compaction summaries.
    pub messages: bool,
    /// Tool calls and tool output.
    pub tools: bool,
    /// Model thinking.
    pub thinking: bool,
    /// Turns from an exchange the operator interrupted.
    pub aborted: bool,
    /// Turns the harness's own interface never showed.
    pub hidden: bool,
}

impl Default for ContentKinds {
    fn default() -> Self {
        Self {
            messages: true,
            tools: false,
            thinking: false,
            aborted: false,
            hidden: false,
        }
    }
}

impl ContentKinds {
    /// Whether `turn` is in scope.
    pub fn includes(&self, turn: &TranscriptTurn) -> bool {
        if (turn.aborted && !self.aborted) || (turn.hidden && !self.hidden) {
            return false;
        }
        match turn.kind {
            TurnKind::Message => {
                self.messages && matches!(turn.role, TurnRole::User | TurnRole::Assistant)
            }
            TurnKind::CompactionSummary => self.messages,
            TurnKind::ToolUse | TurnKind::ToolResult => self.tools,
            TurnKind::Thinking => self.thinking,
        }
    }
}

/// A content query: the text and the kinds of turn to search.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentQuery {
    pub text: String,
    pub kinds: ContentKinds,
}

impl ContentQuery {
    pub fn new(text: impl Into<String>, kinds: ContentKinds) -> Self {
        Self {
            text: text.into(),
            kinds,
        }
    }

    /// The text that is matched: trimmed, `None` when nothing is left.
    pub fn needle(&self) -> Option<&str> {
        let trimmed = self.text.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }
}

/// One matching turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hit {
    /// Index into [`TranscriptDocument::turns`].
    pub turn_index: usize,
    pub role: TurnRole,
    pub kind: TurnKind,
    pub timestamp: Option<DateTime<Utc>>,
    /// A capped, normalized window around the first match, with `…` on
    /// a truncated side.
    pub snippet: String,
    /// Byte range inside `snippet` to highlight. Empty when the match
    /// was only control characters.
    pub highlight: Range<usize>,
}

/// Find the first case-insensitive (ASCII) occurrence of `needle` in
/// `haystack`, as a byte offset, without allocating. Non-ASCII bytes
/// compare exactly. The offset is always a character boundary because
/// `needle` is valid UTF-8 and starts on one.
pub fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let hay = haystack.as_bytes();
    let pat = needle.as_bytes();
    let first = *pat.first()?;
    if pat.len() > hay.len() {
        return None;
    }
    let first = first.to_ascii_lowercase();
    let last_start = hay.len() - pat.len();
    (0..=last_start).find(|&start| {
        hay[start].to_ascii_lowercase() == first
            && hay[start + 1..start + pat.len()]
                .iter()
                .zip(&pat[1..])
                .all(|(h, p)| h.eq_ignore_ascii_case(p))
    })
}

/// The hits in `doc` for `query`, in turn order.
pub fn match_document(doc: &TranscriptDocument, query: &ContentQuery) -> Vec<Hit> {
    let Some(needle) = query.needle() else {
        return Vec::new();
    };
    doc.turns
        .iter()
        .enumerate()
        .filter(|(_, turn)| query.kinds.includes(turn))
        .filter_map(|(index, turn)| match_turn(index, turn, needle))
        .collect()
}

/// The hit in one turn, if its body contains `needle`.
pub fn match_turn(turn_index: usize, turn: &TranscriptTurn, needle: &str) -> Option<Hit> {
    let start = find_ci(&turn.body, needle)?;
    let (snippet, highlight) = snippet_for(&turn.body, start..start + needle.len(), needle);
    Some(Hit {
        turn_index,
        role: turn.role,
        kind: turn.kind,
        timestamp: turn.timestamp,
        snippet,
        highlight,
    })
}

/// A windowed view of `haystack` around a matched byte range, sized to
/// fit a result list while keeping the match visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnippet {
    /// The display text, with `…` markers on truncated sides.
    pub text: String,
    /// Byte range inside `text` to highlight. `None` when the source
    /// range was empty or fell outside the haystack.
    pub highlight: Option<Range<usize>>,
}

/// Build a [`MatchSnippet`] for `haystack` centered on `byte_range`,
/// constrained to at most `max_chars` characters of text plus the two
/// truncation markers. The result always contains the matched span.
/// Operates in `char`s so multi-byte UTF-8 stays valid.
///
/// `byte_range` is the byte range of a match against the same haystack.
/// An empty or out-of-range range yields a snippet without a highlight.
pub fn snippet_around(haystack: &str, byte_range: Range<usize>, max_chars: usize) -> MatchSnippet {
    let total_chars = haystack.chars().count();
    if total_chars <= max_chars {
        let highlight = valid_byte_range(haystack, byte_range);
        return MatchSnippet {
            text: haystack.to_string(),
            highlight,
        };
    }

    // Locate the matched span's char indices. When the byte range is
    // empty or outside the haystack, fall back to a head-anchored
    // snippet without a highlight.
    let Some((match_start_char, match_len_chars)) = chars_for_byte_range(haystack, &byte_range)
    else {
        let chars: Vec<char> = haystack.chars().take(max_chars.saturating_sub(1)).collect();
        let mut text: String = chars.iter().collect();
        text.push('…');
        return MatchSnippet {
            text,
            highlight: None,
        };
    };

    // Budget for context on either side, after accounting for the
    // matched span and the two ellipsis markers.
    let context = max_chars
        .saturating_sub(match_len_chars)
        .saturating_sub(2 /* room for two `…` */);
    let before_budget = context / 2;
    let after_budget = context - before_budget;

    let chars: Vec<char> = haystack.chars().collect();
    let mut window_start = match_start_char.saturating_sub(before_budget);
    let mut window_end = (match_start_char + match_len_chars + after_budget).min(total_chars);

    // If one side was unbounded, spend the leftover budget on the
    // other side so the visible width stays close to `max_chars`.
    if window_start == 0 && match_start_char < before_budget {
        let slack = before_budget - match_start_char;
        window_end = (window_end + slack).min(total_chars);
    }
    if window_end == total_chars {
        let used_after = window_end - (match_start_char + match_len_chars);
        if used_after < after_budget {
            let slack = after_budget - used_after;
            window_start = window_start.saturating_sub(slack);
        }
    }

    let needs_leading = window_start > 0;
    let needs_trailing = window_end < total_chars;

    let mut text = String::new();
    if needs_leading {
        text.push('…');
    }
    let pre: String = chars[window_start..match_start_char].iter().collect();
    text.push_str(&pre);
    let highlight_start = text.len();
    let matched: String = chars[match_start_char..match_start_char + match_len_chars]
        .iter()
        .collect();
    text.push_str(&matched);
    let highlight_end = text.len();
    let post: String = chars[match_start_char + match_len_chars..window_end]
        .iter()
        .collect();
    text.push_str(&post);
    if needs_trailing {
        text.push('…');
    }

    MatchSnippet {
        text,
        highlight: Some(highlight_start..highlight_end),
    }
}

fn chars_for_byte_range(haystack: &str, byte_range: &Range<usize>) -> Option<(usize, usize)> {
    if byte_range.is_empty() || byte_range.end > haystack.len() {
        return None;
    }
    let start_char = haystack[..byte_range.start].chars().count();
    let len_chars = haystack[byte_range.clone()].chars().count();
    Some((start_char, len_chars))
}

fn valid_byte_range(haystack: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.is_empty() || range.end > haystack.len() {
        return None;
    }
    Some(range)
}

/// The capped, normalized snippet for a match at `range` in `body`, and
/// the highlight inside it.
///
/// Only a bounded window of raw text around the match is normalized, so
/// a match deep in a very large body stays cheap.
fn snippet_for(body: &str, range: Range<usize>, needle: &str) -> (String, Range<usize>) {
    let start = char_boundary_back(body, range.start, SNIPPET_WINDOW_CHARS);
    let end = char_boundary_forward(body, range.end, SNIPPET_WINDOW_CHARS);
    let window = normalize(&body[start..end]);
    let needle = normalize(needle);
    let found = find_ci(&window, &needle).map(|at| at..at + needle.len());
    let snippet = snippet_around(&window, found.unwrap_or(0..0), SNIPPET_MAX_CHARS);
    let highlight = snippet.highlight.unwrap_or(0..0);
    (snippet.text, highlight)
}

/// `index` moved back by up to `chars` characters, on a boundary.
fn char_boundary_back(text: &str, index: usize, chars: usize) -> usize {
    text[..index]
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map_or(0, |(at, _)| at)
}

/// `index` moved forward by up to `chars` characters, on a boundary.
fn char_boundary_forward(text: &str, index: usize, chars: usize) -> usize {
    text[index..]
        .char_indices()
        .nth(chars)
        .map_or(text.len(), |(at, _)| index + at)
}

/// Collapse runs of whitespace to one space, trim the ends, and drop
/// control characters and terminal escape sequences, so a snippet is one
/// line of plain text.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(SNIPPET_WINDOW_CHARS * 2));
    let mut pending_space = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            skip_escape(&mut chars);
            continue;
        }
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        if c.is_control() {
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }
    out
}

/// Consume the rest of a terminal escape sequence after `ESC`: a CSI
/// (`[` ... final byte), an OSC (`]` ... `BEL` or `ESC \`), or a
/// two-character escape.
fn skip_escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for c in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '\u{7}' {
                    break;
                }
                if c == '\u{1b}' {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
            }
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}

/// What one session's hits add up to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionHits {
    /// Number of matching turns.
    pub count: usize,
    /// The hit to show for the session: a message over a tool or
    /// thinking hit, then the earliest turn.
    pub best: Hit,
    /// The latest hit time, `None` when no hit has a timestamp.
    pub latest: Option<DateTime<Utc>>,
}

impl SessionHits {
    /// Aggregate `hits` from one document. `None` when there are none.
    pub fn from_hits(hits: &[Hit]) -> Option<Self> {
        let best = hits
            .iter()
            .min_by_key(|hit| (kind_priority(hit.kind), hit.turn_index))?
            .clone();
        Some(Self {
            count: hits.len(),
            best,
            latest: hits.iter().filter_map(|hit| hit.timestamp).max(),
        })
    }
}

fn kind_priority(kind: TurnKind) -> u8 {
    match kind {
        TurnKind::Message | TurnKind::CompactionSummary => 0,
        TurnKind::ToolUse | TurnKind::ToolResult => 1,
        TurnKind::Thinking => 2,
    }
}

/// Order sessions for per-session results: more hits first, then the
/// more recent latest hit, a session with no timestamp last.
pub fn cmp_sessions(a: &SessionHits, b: &SessionHits) -> Ordering {
    b.count
        .cmp(&a.count)
        .then_with(|| cmp_latest_first(a.latest, b.latest))
}

/// Order sessions by their latest hit, newest first.
pub fn cmp_sessions_by_recency(a: &SessionHits, b: &SessionHits) -> Ordering {
    cmp_latest_first(a.latest, b.latest).then_with(|| b.count.cmp(&a.count))
}

/// Order per-message hits: newest first, a hit with no timestamp last,
/// ties in turn order.
pub fn cmp_hits(a: &Hit, b: &Hit) -> Ordering {
    cmp_latest_first(a.timestamp, b.timestamp).then_with(|| a.turn_index.cmp(&b.turn_index))
}

fn cmp_latest_first(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
