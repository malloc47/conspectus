//! Viewer data model — pure types, no I/O.
//!
//! The viewer takes [`SessionLocator`] as its input and produces
//! [`TranscriptDocument`] for rendering. Conspectus-graph types
//! (`AgentSessionId`, `AgentSessionNode`, etc.) do NOT appear here;
//! the conversion happens in `crate::tui::viewer_bridge` so the
//! extracted crate stays graph-agnostic.
//!
//! Stubs only at this scaffold stage. `H-VIEWER-NATIVE-002` fills
//! in fields and serde derives. `H-VIEWER-NATIVE-003 .. 005` add
//! the per-harness reader paths that produce these types.

/// Identifies a session that the viewer is asked to render. Per
/// harness because the on-disk shape differs (JSONL vs SQLite).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionLocator {
    ClaudeCode {
        state_root: std::path::PathBuf,
        session_key: String,
    },
    Codex {
        state_root: std::path::PathBuf,
        session_key: String,
    },
    OpenCode {
        db_path: std::path::PathBuf,
        session_id: String,
    },
}

/// A single rendered transcript turn. The viewer iterates these
/// linearly for layout; `TurnKind` lets the renderer treat
/// compaction summaries / tool blocks / channel-marker turns
/// distinctly from ordinary prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptTurn {
    pub role: TurnRole,
    pub kind: TurnKind,
    /// Raw body content (Markdown for assistant / user turns; the
    /// renderer feeds this through `tui-markdown`).
    pub body: String,
    /// Optional turn timestamp. `H-VIEWER-NATIVE-002` will commit
    /// to chrono semantics.
    pub timestamp: Option<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnKind {
    /// Plain prose Markdown.
    Message,
    /// Claude Code compaction summary record (`isCompactSummary`).
    CompactionSummary,
    /// Tool use record — body is the tool call payload.
    ToolUse,
    /// Tool result record — body is the tool output.
    ToolResult,
    /// Model thinking / reasoning block.
    Thinking,
}

/// A full transcript ready for the widget to render.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptDocument {
    pub turns: Vec<TranscriptTurn>,
    pub meta: TranscriptMeta,
}

/// Per-document metadata the header pane displays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptMeta {
    pub harness: String,
    pub session_key: String,
    pub cwd: Option<String>,
}
