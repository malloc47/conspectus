//! Marking the rows the active content search found: each session row
//! (and a mux row with one attached session) gets the hit count and the
//! best snippet.

use super::{ContentHitCell, RowKind, RowTree};
use crate::filter::ContentFilter;
use crate::model::GraphSnapshot;
use crate::viewer::search::SessionHits;

/// The cell a session's hits become.
pub(crate) fn cell(hits: &SessionHits) -> ContentHitCell {
    ContentHitCell {
        count: u32::try_from(hits.count).unwrap_or(u32::MAX),
        snippet: hits.best.snippet.clone(),
        highlight: hits.best.highlight.clone(),
    }
}

/// Fill in `content_hit` on every row `content` found something in.
pub(crate) fn annotate(tree: &mut RowTree, snapshot: &GraphSnapshot, content: &ContentFilter) {
    for row in &mut tree.rows {
        match &mut row.kind {
            RowKind::AgentSession(session) => {
                session.content_hit = content
                    .hits
                    .get(&session.session.harness_key, &session.session.session_key)
                    .map(cell);
            }
            RowKind::MuxSession(mux) => {
                let linked = crate::tui::app::sessions_linked_to_mux(snapshot, &mux.mux);
                mux.content_hit = match linked.as_slice() {
                    [only] => content
                        .hits
                        .get(&only.harness_key, &only.session_key)
                        .map(cell),
                    _ => None,
                };
            }
            _ => {}
        }
    }
}
