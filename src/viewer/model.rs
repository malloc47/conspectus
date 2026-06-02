//! Viewer data model — pure types, no I/O.
//!
//! The viewer takes [`SessionLocator`] as its input and produces
//! [`TranscriptDocument`] for rendering. Conspectus-graph types
//! (`AgentSessionId`, `AgentSessionNode`, etc.) do NOT appear here;
//! the conversion happens in `crate::tui::viewer_bridge` so the
//! extracted crate stays graph-agnostic (ADR 0052).
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "harness", rename_all = "kebab-case")]
pub enum SessionLocator {
    /// `<state_root>/projects/*/<session_key>.jsonl`.
    ClaudeCode {
        state_root: PathBuf,
        session_key: String,
    },
    /// `<state_root>/sessions/YYYY/MM/DD/rollout-*-<session_key>.jsonl`.
    Codex {
        state_root: PathBuf,
        session_key: String,
    },
    /// `<db_path>` (SQLite) holding `session_id`. ADR 0013.
    ///
    /// Rename override because conspectus's harness key for this
    /// provider is the single-word `opencode`, not what
    /// `kebab-case` would produce.
    #[serde(rename = "opencode")]
    OpenCode {
        db_path: PathBuf,
        session_id: String,
    },
}

impl SessionLocator {
    /// Provider-neutral harness key for the locator. Matches the
    /// `AgentSessionId::harness_key` strings conspectus uses
    /// elsewhere so the bridge doesn't have to remap.
    pub fn harness_key(&self) -> &'static str {
        match self {
            Self::ClaudeCode { .. } => "claude-code",
            Self::Codex { .. } => "codex",
            Self::OpenCode { .. } => "opencode",
        }
    }

    /// The harness-native session identifier (UUID for Claude/Codex,
    /// `ses_*` for OpenCode). Used in `TranscriptMeta` headers.
    pub fn session_key(&self) -> &str {
        match self {
            Self::ClaudeCode { session_key, .. } => session_key,
            Self::Codex { session_key, .. } => session_key,
            Self::OpenCode { session_id, .. } => session_id,
        }
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
    /// `system`). Matches the inline-preview labels per the
    /// H-TRANSCRIPT-009 sketch so the two surfaces stay
    /// consistent.
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
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_locator_claude() -> SessionLocator {
        SessionLocator::ClaudeCode {
            state_root: PathBuf::from("/home/u/.claude"),
            session_key: "0b34e59c-14d0-4d04-be79-4dc1d4c120c2".to_string(),
        }
    }

    fn sample_locator_codex() -> SessionLocator {
        SessionLocator::Codex {
            state_root: PathBuf::from("/home/u/.codex"),
            session_key: "019df146-41e8-7fb0-8df0-dc326b4fdee8".to_string(),
        }
    }

    fn sample_locator_opencode() -> SessionLocator {
        SessionLocator::OpenCode {
            db_path: PathBuf::from("/home/u/.local/share/opencode/opencode.db"),
            session_id: "ses_17f328a8effeK52nvLaEV954yO".to_string(),
        }
    }

    #[test]
    fn locator_display_matches_harness_session_form() {
        assert_eq!(
            sample_locator_claude().to_string(),
            "claude-code:0b34e59c-14d0-4d04-be79-4dc1d4c120c2"
        );
        assert_eq!(
            sample_locator_codex().to_string(),
            "codex:019df146-41e8-7fb0-8df0-dc326b4fdee8"
        );
        assert_eq!(
            sample_locator_opencode().to_string(),
            "opencode:ses_17f328a8effeK52nvLaEV954yO"
        );
    }

    #[test]
    fn locator_harness_key_round_trips() {
        assert_eq!(sample_locator_claude().harness_key(), "claude-code");
        assert_eq!(sample_locator_codex().harness_key(), "codex");
        assert_eq!(sample_locator_opencode().harness_key(), "opencode");
    }

    #[test]
    fn locator_serde_round_trip_claude() {
        let original = sample_locator_claude();
        let json = serde_json::to_string(&original).expect("serialize");
        // Tag-shape contract: external bin / future config files
        // should be able to write `{"harness": "claude-code", ...}`.
        assert!(json.contains("\"harness\":\"claude-code\""), "got {json}");
        let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn locator_serde_round_trip_codex() {
        let original = sample_locator_codex();
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn locator_serde_round_trip_opencode() {
        let original = sample_locator_opencode();
        let json = serde_json::to_string(&original).expect("serialize");
        assert!(json.contains("\"harness\":\"opencode\""), "got {json}");
        let decoded: SessionLocator = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn turn_kind_shown_by_default_filters_noise() {
        assert!(TurnKind::Message.shown_by_default());
        assert!(TurnKind::CompactionSummary.shown_by_default());
        assert!(!TurnKind::ToolUse.shown_by_default());
        assert!(!TurnKind::ToolResult.shown_by_default());
        assert!(!TurnKind::Thinking.shown_by_default());
    }

    #[test]
    fn turn_role_header_labels_match_inline_preview() {
        assert_eq!(TurnRole::User.header_label(), "you");
        assert_eq!(TurnRole::Assistant.header_label(), "assistant");
        assert_eq!(TurnRole::System.header_label(), "system");
    }

    #[test]
    fn transcript_turn_serde_round_trip_with_timestamp() {
        let original = TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: "Hello **world**.".to_string(),
            timestamp: Some(
                Utc.with_ymd_and_hms(2026, 6, 1, 16, 52, 36)
                    .single()
                    .expect("valid timestamp"),
            ),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: TranscriptTurn = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn transcript_turn_serde_omits_absent_timestamp() {
        let turn = TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: "hi".to_string(),
            timestamp: None,
        };
        let json = serde_json::to_string(&turn).expect("serialize");
        assert!(!json.contains("timestamp"), "got {json}");
        let decoded: TranscriptTurn = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, turn);
    }

    #[test]
    fn transcript_document_unavailable_carries_meta_and_no_turns() {
        let doc = TranscriptDocument::unavailable(&sample_locator_claude());
        assert!(doc.is_empty());
        assert_eq!(doc.meta.harness, "claude-code");
        assert_eq!(doc.meta.session_key, "0b34e59c-14d0-4d04-be79-4dc1d4c120c2");
        assert!(doc.meta.cwd.is_none());
    }

    #[test]
    fn transcript_document_serde_round_trip() {
        let original = TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "abc-123".to_string(),
                cwd: Some("/home/u/src/proj".to_string()),
            },
            turns: vec![
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    body: "what's up?".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::Message,
                    body: "not much".to_string(),
                    timestamp: Some(
                        Utc.with_ymd_and_hms(2026, 6, 1, 17, 0, 0)
                            .single()
                            .expect("valid timestamp"),
                    ),
                },
            ],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: TranscriptDocument = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }
}
