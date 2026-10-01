//! Viewer data model — pure types, no I/O.
//!
//! Per ADR 0052 §"Own its data model — as a normalized superset",
//! the types here are the **normalized superset of every supported
//! harness's transcript format**. Parsers translate native records
//! into this shape; the renderer and future cross-harness
//! operations (export, search, diff) consume only the normalized
//! form. Provider-specific reality lives in the per-harness
//! parsers and dies before reaching the widget.
//!
//! The viewer takes [`SessionLocator`] as its input and produces
//! [`TranscriptDocument`] for rendering. Conspectus-graph types
//! (`AgentSessionId`, `AgentSessionNode`, etc.) do NOT appear here;
//! the conversion happens in `crate::tui::viewer_bridge` so the
//! extracted crate stays graph-agnostic.
//!
//! Every public type derives `Serialize` + `Deserialize`. The
//! extracted-crate `bin` will accept a `SessionLocator` from clap;
//! exports/snapshot tests round-trip through serde.

use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identifies a session that the viewer is asked to render.
///
/// Per harness because the on-disk shape differs: JSONL files for
/// Claude Code and Codex, a SQLite database row for OpenCode. The
/// renderer never branches on harness — it consumes the parsed
/// [`TranscriptDocument`] — but the parser dispatcher needs to know
/// which file/db to open.
///
/// An open struct rather than a per-harness enum: adapters build
/// the locator via
/// [`crate::discovery::harness::HarnessAdapter::transcript_source`];
/// each adapter's `state_root` interpretation is documented on
/// that method. Adding a fifth harness with a native transcript
/// source is a matter of implementing the trait methods — no new
/// enum variant, no bridge / parser dispatch update.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionLocator {
    /// Adapter harness key (e.g. `claude-code`, `codex`,
    /// `opencode`). Matches
    /// [`crate::model::AgentSessionId::harness_key`].
    pub harness_key: String,
    /// Harness-native session id (UUID for Claude/Codex,
    /// `ses_<alphanumeric>` for OpenCode).
    pub session_key: String,
    /// Adapter-authored transcript root. Interpretation is
    /// parser-specific:
    /// - claude-code + codex → harness state root; the parser
    ///   walks the standard per-session file layout beneath it.
    /// - opencode → resolved SQLite database file (or its
    ///   parent directory; the opencode adapter's
    ///   `transcript_source` handles both shapes).
    pub state_root: PathBuf,
}

impl SessionLocator {
    /// Adapter harness key for this locator.
    pub fn harness_key(&self) -> &str {
        &self.harness_key
    }

    /// The harness-native session identifier.
    pub fn session_key(&self) -> &str {
        &self.session_key
    }
}

impl fmt::Display for SessionLocator {
    /// `<harness>:<session-key>` form for status-bar diagnostics.
    /// Matches what an operator would see in `conspectus table
    /// sessions` so the two surfaces line up.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.harness_key(), self.session_key())
    }
}

/// A single rendered transcript turn. The viewer iterates these
/// linearly for layout; [`TurnKind`] lets the renderer treat
/// compaction summaries / tool blocks / channel-marker turns
/// distinctly from ordinary prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTurn {
    pub role: TurnRole,
    pub kind: TurnKind,
    /// Raw body content. For [`TurnKind::Message`] this is
    /// Markdown; the renderer feeds it through `tui-markdown`. For
    /// tool blocks it is the tool call payload / output verbatim.
    pub body: String,
    /// Optional turn timestamp in UTC, parsed from the harness's
    /// own record format (RFC 3339 in Claude / Codex, epoch in
    /// OpenCode). `None` when the source record had no timestamp
    /// or it failed to parse — viewers degrade gracefully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    /// `true` when this turn was part of an exchange the user
    /// interrupted before the agent finished responding. Hidden by
    /// default; revealed via the viewer's `I` toggle. Detection is
    /// per-harness: Claude marks user records with no descendants in
    /// the parent/child uuid graph; Codex consumes its
    /// `event_msg.turn_aborted` signal; OpenCode reads
    /// `error.name == "MessageAbortedError"` on assistant rows. Both
    /// the user-sent text *and* any partial assistant/tool output
    /// that landed before the abort are tagged, so the whole aborted
    /// exchange disappears together when `show_aborted` is off — the
    /// goal is to match what the harness's own UI showed the user.
    #[serde(default, skip_serializing_if = "is_false")]
    pub aborted: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Who produced the turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
    System,
}

impl TurnRole {
    /// Short label the viewer header uses (`you`, `assistant`,
    /// `system`). Matches the inline-preview labels so the two
    /// surfaces stay consistent.
    pub fn header_label(self) -> &'static str {
        match self {
            Self::User => "you",
            Self::Assistant => "assistant",
            Self::System => "system",
        }
    }
}

/// Why this turn exists in the transcript. Renderer uses this to
/// pick chrome (compaction-summary banner, tool-block fold, etc.).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    /// Plain prose Markdown — the common case.
    Message,
    /// Claude Code compaction summary (`isCompactSummary`). The
    /// boundary itself appears in the transcript distinctly.
    CompactionSummary,
    /// Tool use record. Body is the tool invocation payload.
    ToolUse,
    /// Tool result record. Body is the tool output verbatim.
    ToolResult,
    /// Model thinking / reasoning block (Claude Code thinking
    /// blocks, Codex `reasoning` records).
    Thinking,
}

impl TurnKind {
    /// Whether the renderer shows this kind by default. Tool blocks
    /// and thinking blocks are noisy and opt-in via a toggle in
    /// the viewer (matching `claude-history`'s `--show-tools` /
    /// `--show-thinking`). Messages and compaction summaries are
    /// always shown.
    pub fn shown_by_default(self) -> bool {
        matches!(self, Self::Message | Self::CompactionSummary)
    }
}

/// A full transcript ready for the widget to render.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranscriptDocument {
    pub meta: TranscriptMeta,
    pub turns: Vec<TranscriptTurn>,
}

impl TranscriptDocument {
    /// Convenience: build an empty "transcript unavailable" document
    /// so the widget always has something coherent to render when a
    /// parser fails. The renderer detects the empty-turns case and
    /// shows a one-line "transcript unavailable" banner.
    pub fn unavailable(locator: &SessionLocator) -> Self {
        Self {
            meta: TranscriptMeta {
                harness: locator.harness_key().to_string(),
                session_key: locator.session_key().to_string(),
                cwd: None,
            },
            turns: Vec::new(),
        }
    }

    /// True when no turns were parsed. Renderer treats this as
    /// "transcript unavailable" instead of "empty session".
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }
}

/// Per-document metadata the header pane displays.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranscriptMeta {
    /// Harness key (`claude-code` / `codex` / `opencode`).
    pub harness: String,
    /// Harness-native session id.
    pub session_key: String,
    /// Session cwd when known. `None` for harnesses that don't
    /// record it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
