//! Ranking for the `/` in-view search overlay. Per ADR 0024 the
//! matcher is hand-rolled: a case-insensitive (ASCII) substring match,
//! shared with content search through [`crate::viewer::search`].
//!
//! Per ADR 0031 the `/` overlay ranks within the **active filter
//! set**, not the full snapshot. The runtime extracts
//! [`SearchItem`]s from the current visible row tree (which already
//! has filters applied) and hands them to the backend.

use std::borrow::Cow;

use crate::tui::rows::{Row, RowId, RowKind};

// The matching and snippet helpers are shared with content search so
// `/` and the transcript search agree on what a match is.
pub use crate::viewer::search::{MatchSnippet, find_ci, snippet_around};

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

/// Rank `items` against `query` with a case-insensitive substring
/// match, best first. Score is the negative byte position of the first
/// match, so an earlier match outranks a later one. An empty query
/// returns nothing, so the overlay stays empty until the operator types.
pub fn rank(query: &str, items: &[SearchItem<'_>]) -> Vec<SearchMatch> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<SearchMatch> = items
        .iter()
        .filter_map(|item| {
            let start = find_ci(&item.haystack, q)?;
            Some(SearchMatch {
                id: item.id.clone(),
                score: -(start as i64),
                matched_range: Some(start..start + q.len()),
            })
        })
        .collect();
    matches.sort_by_key(|m| std::cmp::Reverse(m.score));
    matches
}

/// Build the per-row search inputs from [`Row`]s. The
/// haystack concatenates every visible field for the row kind so a
/// query like `"puffin"` matches an alias **or** a title **or** a
/// preview. The label is the most operator-recognizable text —
/// alias / title / display path — which is what the result list
/// renders.
pub fn items_from_rows<'r>(rows: impl IntoIterator<Item = &'r Row>) -> Vec<SearchItem<'static>> {
    rows.into_iter()
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
            RowKind::Hit(hit) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(hit.snippet.clone()),
                haystack: Cow::Owned(hit.snippet.clone()),
            },
            RowKind::Pane(pane) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(format!("{} {}", pane.mux_label, pane.location)),
                haystack: Cow::Owned(format!(
                    "{} {} {}",
                    pane.mux_label,
                    pane.command.as_deref().unwrap_or_default(),
                    pane.snippet
                )),
            },
            RowKind::PaneHit(hit) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(hit.snippet.clone()),
                haystack: Cow::Owned(hit.snippet.clone()),
            },
            RowKind::Mixed(mixed) => SearchItem {
                id: row.id.clone(),
                label: Cow::Owned(mixed.label.clone()),
                haystack: Cow::Owned(format!(
                    "{} {} {}",
                    mixed.label, mixed.status, mixed.content
                )),
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
    // The session key is what the row's id column shows, so a typed
    // or pasted id finds its row.
    parts.push(&session.session.session_key);
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
