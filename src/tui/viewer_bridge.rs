//! Bridge between the conspectus TUI (graph-aware) and the
//! extraction-ready native viewer (`src/viewer/`, graph-agnostic).
//!
//! This file lives **outside** the `src/viewer/` subtree on purpose
//! (ADR 0052 §"Extraction-ready boundary" rule 2): it's the only
//! place in conspectus that knows both the graph-flavored
//! `AgentSessionId` and the viewer-flavored `SessionLocator`. When
//! the viewer is extracted to a standalone crate, this file is the
//! glue that stays behind — extracted-crate users wire up their own
//! `SessionLocator` from clap arguments without ever touching
//! conspectus graph types.
//!
//! Harness dispatch goes through the adapter registry.
//! `locator_for_session` iterates registered adapters and asks each
//! for a `transcript_source`; the first non-`None` answer wins.
//! `build_viewer` grabs the same adapter's `transcript_parser` and
//! delegates the read. Aider (and every future harness that declines
//! `transcript_source`) drops through both functions with `None` so the
//! escape-hatch external viewer takes over.
//!
//! The viewer itself is the framework's pager (`tuikit::pager`) over the
//! viewer's transcript content: the pager owns scrolling, find, help,
//! and the footer, so it is an overlay on the modal stack like any
//! other, and the bridge only builds it.

use tuikit::pager::Pager;

use crate::discovery::harness::registered_adapters;
use crate::model::AgentSessionId;
use crate::tui::Msg;
use crate::viewer::model::{SessionLocator, TranscriptDocument};
use crate::viewer::parser::ParseError;
use crate::viewer::state::{Transcript, ViewerOpen};

/// The full-screen transcript viewer: the framework's pager over a
/// transcript, an overlay on the TUI's modal stack.
pub type TranscriptViewer = Pager<Transcript<Msg>>;

/// Map a graph-flavored [`AgentSessionId`] to a viewer-flavored
/// `SessionLocator`, or `None` when the harness key has no
/// registered native transcript source.
pub fn locator_for_session(session: &AgentSessionId) -> Option<SessionLocator> {
    registered_adapters()
        .find(|a| a.harness_key() == session.harness_key)
        .and_then(|a| a.transcript_source(session))
}

/// Resolve a session into the viewer, opened at its newest turns.
/// Parser failures degrade to a `TranscriptDocument::unavailable`
/// document so the viewer can always render a coherent banner
/// instead of bubbling the error up.
pub fn build_viewer(session: &AgentSessionId) -> Option<TranscriptViewer> {
    build_viewer_at(session, None)
}

/// [`build_viewer`], landed on a search result when `open` is given:
/// the viewer highlights the query and shows the matched turn,
/// switching on whatever hides it.
pub fn build_viewer_at(
    session: &AgentSessionId,
    open: Option<&ViewerOpen>,
) -> Option<TranscriptViewer> {
    let adapter = registered_adapters().find(|a| a.harness_key() == session.harness_key)?;
    let locator = adapter.transcript_source(session)?;
    let document = adapter
        .transcript_parser()
        .and_then(|parser| parser.read(&locator).ok())
        .unwrap_or_else(|| TranscriptDocument::unavailable(&locator));
    Some(viewer(document, open))
}

/// The viewer over `document`, its title in the harness's color, at its
/// newest turns or landed on `open`.
pub fn viewer(document: TranscriptDocument, open: Option<&ViewerOpen>) -> TranscriptViewer {
    let role = crate::tui::theme::harness_role(&document.meta.harness);
    crate::viewer::find::viewer(Transcript::new(document).title_role(role), open)
}

/// Why a session's transcript has nothing to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptMissing {
    /// There is nothing to read: the viewer has no parser for the
    /// harness, or the transcript file is gone.
    Unavailable(String),
    /// The transcript is there, but reading or parsing it failed.
    Failed(String),
}

/// Read a session's transcript for the inline preview (ADR 0112).
/// Unlike [`build_viewer`], which degrades every failure to an
/// empty document for the modal's banner, this keeps the reason so the
/// preview can say why there is nothing to show.
pub fn load_transcript(session: &AgentSessionId) -> Result<TranscriptDocument, TranscriptMissing> {
    let unsupported = || {
        TranscriptMissing::Unavailable(format!(
            "the viewer can't read {} sessions",
            session.harness_key
        ))
    };
    let adapter = registered_adapters()
        .find(|a| a.harness_key() == session.harness_key)
        .ok_or_else(unsupported)?;
    let locator = adapter.transcript_source(session).ok_or_else(unsupported)?;
    let parser = adapter.transcript_parser().ok_or_else(unsupported)?;
    parser.read(&locator).map_err(|err| match err {
        ParseError::NotFound => TranscriptMissing::Unavailable(err.to_string()),
        ParseError::Io(_) | ParseError::Malformed(_) => TranscriptMissing::Failed(err.to_string()),
    })
}

#[cfg(test)]
#[path = "viewer_bridge_tests.rs"]
mod tests;
