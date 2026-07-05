//! Search backend boundary for the `/` in-view overlay (T8-017).
//!
//! Per ADR 0024 the TUI ships hand-rolled match logic first; this
//! module formalizes the boundary so a future swap to `nucleo` /
//! `fuzzy-matcher` / a remote ranker requires only an
//! implementation, not a renderer change. The overlay (in
//! `src/tui/widgets/search.rs`) never imports the backend
//! directly — it goes through this trait.
//!
//! Per ADR 0031 the `/` overlay ranks within the **active filter
//! set**, not the full snapshot. The runtime extracts
//! [`SearchItem`]s from the current visible row tree (which already
//! has filters applied) and hands them to the backend.

use std::borrow::Cow;

use crate::tui::rows::{Row, RowId, RowKind};

/// Anything that can rank a list of items against a query string.
/// Implementations stay pure: no I/O, no internal state mutation
/// per call, no panics on empty input.
///
/// The trait deliberately takes ownership of nothing — items
/// borrow into the live row tree so the runtime doesn't allocate
/// per keystroke.
pub trait SearchBackend: Send + Sync {
    /// Rank `items` against `query`. Returns matches in descending
    /// score order. An empty query may return everything in
    /// stable order or nothing at all; the runtime treats an empty
    /// query as "show nothing" so the operator sees the matches
    /// only once they've typed something.
    fn rank(&self, query: &str, items: &[SearchItem<'_>]) -> Vec<SearchMatch>;
}

/// One searchable row's haystack plus the [`RowId`] the renderer
/// needs to highlight or commit. `label` is the operator-facing
/// label (used inside the overlay's result list); `haystack` is the
/// full text the backend matches against — typically a
/// concatenation of every relevant field for that row kind so
/// substrings cover alias, title, cwd, preview, etc.
#[derive(Debug, Clone)]
pub struct SearchItem<'a> {
    pub id: RowId,
    pub label: Cow<'a, str>,
    pub haystack: Cow<'a, str>,
}

/// One row that matched a query. `matched_range` is a byte range
/// inside `haystack` (or `None` when the backend doesn't model
/// inline highlights). The score is opaque — higher wins — and is
/// only used to rank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub id: RowId,
    pub score: i64,
    pub matched_range: Option<std::ops::Range<usize>>,
}

/// v1 backend: case-insensitive substring match. Score = the
/// negative byte position of the first match, so an earlier match
/// outranks a later one. Returns `Vec::new()` for an empty query
/// so the operator never sees the entire tree pop into the
/// overlay's result list before they've typed.
#[derive(Debug, Default, Clone, Copy)]
pub struct SubstringBackend;

impl SearchBackend for SubstringBackend {
    fn rank(&self, query: &str, items: &[SearchItem<'_>]) -> Vec<SearchMatch> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let needle = q.to_ascii_lowercase();
        let mut matches: Vec<SearchMatch> = items
            .iter()
            .filter_map(|item| {
                let haystack_lc = item.haystack.to_ascii_lowercase();
                let start = haystack_lc.find(&needle)?;
                Some(SearchMatch {
                    id: item.id.clone(),
                    score: -(start as i64),
                    matched_range: Some(start..start + needle.len()),
                })
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }
}

/// A windowed view of the haystack around a matched range, sized
/// so it fits in the overlay's result list while keeping the match
/// itself visible. The snippet is built by [`snippet_around`] and
/// consumed by the search renderer to show context next to each
/// match (rather than just the row label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnippet {
    /// The display text, with `…` markers on truncated sides.
    pub text: String,
    /// Byte range inside `text` to highlight (typically rendered
    /// bold). `None` when the source range was empty or fell
    /// outside the haystack.
    pub highlight: Option<std::ops::Range<usize>>,
}

/// Build a [`MatchSnippet`] for `haystack` centered on `byte_range`,
/// constrained to at most `max_chars` characters total. The result
/// always contains the matched span; truncation happens on either
/// side with a `…` marker. Operates in `char`s so multi-byte UTF-8
/// stays valid.
///
/// `byte_range` is the byte range produced by a [`SearchBackend`]
/// against the same haystack. An empty / out-of-range range yields
/// a snippet without a highlight.
pub fn snippet_around(
    haystack: &str,
    byte_range: std::ops::Range<usize>,
    max_chars: usize,
) -> MatchSnippet {
    let total_chars = haystack.chars().count();
    if total_chars <= max_chars {
        let highlight = valid_byte_range(haystack, byte_range);
        return MatchSnippet {
            text: haystack.to_string(),
            highlight,
        };
    }

    // Locate the matched span's char indices. When the byte range
    // is empty or outside the haystack, fall back to a head-anchored
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
    // matched span and the two ellipsis markers (only used when we
    // actually truncate).
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

fn chars_for_byte_range(
    haystack: &str,
    byte_range: &std::ops::Range<usize>,
) -> Option<(usize, usize)> {
    if byte_range.is_empty() || byte_range.end > haystack.len() {
        return None;
    }
    let start_char = haystack[..byte_range.start].chars().count();
    let len_chars = haystack[byte_range.clone()].chars().count();
    Some((start_char, len_chars))
}

fn valid_byte_range(
    haystack: &str,
    range: std::ops::Range<usize>,
) -> Option<std::ops::Range<usize>> {
    if range.is_empty() || range.end > haystack.len() {
        return None;
    }
    Some(range)
}

/// Build the per-row search inputs from a slice of [`Row`]s. The
/// haystack concatenates every visible field for the row kind so a
/// query like `"puffin"` matches an alias **or** a title **or** a
/// preview. The label is the most operator-recognizable text —
/// alias / title / display path — which is what the result list
/// renders.
pub fn items_from_rows<'a>(rows: &'a [Row]) -> Vec<SearchItem<'a>> {
    rows.iter()
        .map(|row| match &row.kind {
            RowKind::AgentSession(session) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(session.display_label().map_or_else(
                    || format!("{}:{}", session.harness_label, session.short_id),
                    str::to_string,
                )),
                haystack: Cow::Owned(agent_haystack(session)),
            },
            RowKind::Group(group) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(group.display_path.clone()),
                haystack: Cow::Owned(group.display_path.clone()),
            },
            RowKind::AgentSessionMuxCandidate(candidate) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(candidate.mux_label.clone()),
                haystack: Cow::Owned(candidate.mux_label.clone()),
            },
            RowKind::MuxSession(mux) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(format!("mux:{}", mux.native_id)),
                haystack: Cow::Owned(mux.native_id.clone()),
            },
            RowKind::Pr(pr) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(pr.repo_display.clone()),
                haystack: Cow::Owned(pr.repo_display.clone()),
            },
            RowKind::Fork(fork) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(fork.fork_label.clone()),
                haystack: Cow::Owned(fork.fork_label.clone()),
            },
            RowKind::Pin(pin) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(format!("pin:{}", pin.display_name)),
                haystack: Cow::Owned(format!(
                    "pin {} {} {} {}",
                    pin.display_name, pin.harness_label, pin.cwd_display, pin.mux_label
                )),
            },
            RowKind::Repo(repo) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(format!("repo:{}", repo.display_name)),
                haystack: Cow::Owned(match &repo.canonical_path {
                    Some(path) => {
                        format!("repo {} {} {}", repo.display_name, path, repo.common_dir)
                    }
                    None => format!("repo {} {}", repo.display_name, repo.common_dir),
                }),
            },
        })
        .collect()
}

fn agent_haystack(session: &crate::tui::rows::AgentSessionRow) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(6);
    if let Some(alias) = session.alias.as_deref() {
        parts.push(alias);
    }
    if let Some(title) = session.title.as_deref() {
        parts.push(title);
    }
    parts.push(&session.harness_label);
    parts.push(&session.short_id);
    if let Some(cwd) = session.cwd_display.as_deref() {
        parts.push(cwd);
    }
    if let Some(project) = session.project_display.as_deref() {
        parts.push(project);
    }
    if let Some(preview) = session.preview.as_deref() {
        parts.push(preview);
    }
    parts.join(" · ")
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
