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

/// Build the per-row search inputs from a slice of [`Row`]s. The
/// haystack concatenates every visible field for the row kind so a
/// query like `"puffin"` matches an alias **or** a title **or** a
/// preview. The label is the most operator-recognizable text —
/// alias / title / display path — which is what the result list
/// renders.
pub fn items_from_rows<'a>(rows: &'a [Row]) -> Vec<SearchItem<'a>> {
    rows.iter()
        .map(|row| match &row.kind {
            RowKind::AgentSession(session) => {
                SearchItem {
                    id: row.id.clone(),
                    label: Cow::Owned(session.display_label().map(str::to_string).unwrap_or_else(
                        || format!("{}:{}", session.harness_label, session.short_id),
                    )),
                    haystack: Cow::Owned(agent_haystack(session)),
                }
            }
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
    if let Some(preview) = session.preview.as_deref() {
        parts.push(preview);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentSessionId, NodeId};
    use crate::tui::rows::{AgentSessionRow, MuxIndicator, RowKind};

    fn agent_row(harness: &str, key: &str, alias: Option<&str>, preview: &str) -> Row {
        let session_id = AgentSessionId::new(harness, "/state", key);
        let primary = NodeId::AgentSession(session_id.clone());
        Row {
            id: RowId::AgentSession(primary.clone()),
            depth: 1,
            expandable: false,
            kind: RowKind::AgentSession(AgentSessionRow {
                session: session_id,
                short_id: key.to_string(),
                harness_label: harness.to_string(),
                cwd_display: Some("~/proj".to_string()),
                recency: None,
                activity_epoch: None,
                mux_state: MuxIndicator::Unmuxed,
                preview: Some(preview.to_string()),
                title: None,
                alias: alias.map(str::to_string),
                primary_node: primary,
            }),
        }
    }

    #[test]
    fn substring_backend_returns_empty_for_blank_query() {
        let rows = vec![agent_row("claude-code", "abc", Some("puffin"), "hello")];
        let items = items_from_rows(&rows);
        let backend = SubstringBackend;
        assert!(backend.rank("", &items).is_empty());
        assert!(backend.rank("   ", &items).is_empty());
    }

    #[test]
    fn substring_backend_matches_case_insensitively() {
        let rows = vec![
            agent_row("claude-code", "abc", Some("Puffin"), "hello"),
            agent_row("codex", "xyz", None, "nope"),
        ];
        let items = items_from_rows(&rows);
        let backend = SubstringBackend;
        let matches = backend.rank("PUFFIN", &items);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, rows[0].id);
    }

    #[test]
    fn substring_backend_ranks_earlier_matches_higher() {
        // Two rows; one has the needle at the start, one near the
        // end. The earlier match outranks the later one.
        let rows = vec![
            agent_row("claude-code", "early", Some("puffin"), "x"),
            agent_row("codex", "late", None, "nothing here except puffin"),
        ];
        let items = items_from_rows(&rows);
        let backend = SubstringBackend;
        let matches = backend.rank("puffin", &items);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].id, rows[0].id);
        assert_eq!(matches[1].id, rows[1].id);
    }

    #[test]
    fn substring_backend_marks_matched_range_inside_haystack() {
        let rows = vec![agent_row("claude-code", "a", Some("puffin"), "x")];
        let items = items_from_rows(&rows);
        let backend = SubstringBackend;
        let matches = backend.rank("ffi", &items);
        assert_eq!(matches.len(), 1);
        let range = matches[0].matched_range.clone().unwrap();
        assert_eq!(&items[0].haystack[range], "ffi");
    }

    #[test]
    fn substring_backend_finds_match_in_preview_or_cwd() {
        let rows = vec![agent_row("claude-code", "a", None, "look at puffin here")];
        let items = items_from_rows(&rows);
        let backend = SubstringBackend;
        let matches = backend.rank("look at", &items);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn items_from_rows_uses_alias_then_title_then_short_id_for_label() {
        let rows = vec![agent_row("claude-code", "abc123", Some("nice-alias"), "x")];
        let items = items_from_rows(&rows);
        assert_eq!(items[0].label.as_ref(), "nice-alias");

        // Without an alias the label falls back to harness:short_id.
        let no_alias = vec![agent_row("claude-code", "abc123", None, "x")];
        let items = items_from_rows(&no_alias);
        assert_eq!(items[0].label.as_ref(), "claude-code:abc123");
    }
}
