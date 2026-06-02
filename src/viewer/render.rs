//! Per-turn rendering: [`TranscriptTurn`] → `ratatui::text::Text`
//! using `tui-markdown` for the body.
//!
//! Kept separate from the widget layer so the renderer can be
//! reused by the inline preview (`H-TRANSCRIPT-009`) once the two
//! tracks converge on a shared rendering primitive.
//!
//! Filled by `H-VIEWER-NATIVE-006`.

use crate::viewer::model::TranscriptTurn;

/// Render one turn into terminal-styled text. Stub — the real
/// implementation calls `tui_markdown::from_str` on the body and
/// composes role headers + compaction-summary markers per ADR 0052.
pub fn render_turn(_turn: &TranscriptTurn) -> String {
    String::new()
}
