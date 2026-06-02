//! Pure viewer state — no `ratatui::*` types, no I/O.
//!
//! Per ADR 0024 the reducer is pure and synchronous; `widget` and
//! `input` consume this state. Snapshot tests assert on `ViewerState`
//! directly.
//!
//! Filled by `H-VIEWER-NATIVE-006`.

use crate::viewer::model::TranscriptDocument;

/// The viewer's full state: a document, a scroll offset, a search
/// query, and the navigation cursor. Designed to be cheap to clone
/// for snapshot testing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewerState {
    pub document: TranscriptDocument,
    /// Line offset from document top. ADR 0052 sets the open
    /// position to the last turn — initialized at construction time
    /// by `H-VIEWER-NATIVE-006`.
    pub scroll_offset: usize,
    /// Active search query, if any.
    pub search_query: Option<String>,
    /// Search-match cursor index into the document's match list.
    pub current_match: usize,
}
