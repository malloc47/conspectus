//! Shared evidence-string constants (H-EXT-004).
//!
//! Every mux-attribution candidate the discovery pipeline emits
//! stamps its `SourceMetadata` with one of these strings. The
//! resolver reads them back to rank candidates
//! ([`crate::resolve::process_identity_evidence_rank`]) and the
//! stale-launch demotion path in
//! `discovery::codex_log` matches against them by literal
//! comparison.
//!
//! Pre-H-EXT-004 the strings were duplicated as literals across
//! `discovery/cross_link.rs`, `discovery/codex_log.rs`, and
//! `resolve/mod.rs`. A rename in one place without touching the
//! others would silently misrank candidates; consolidating the
//! strings here makes the coupling explicit and the rename atomic.

/// FD-path evidence: the mux's active pane holds an open
/// harness state file (session JSONL / rollout log / opencode DB
/// path). Highest-weight identity evidence for
/// [`crate::resolve::process_identity_evidence_rank`].
pub const ACTIVE_PANE_FD_SESSION_MATCH: &str = "active_pane_fd_session_match";

/// FD-path evidence corroborated by a matching session key on
/// the pane's start command. Same identity weight as
/// [`ACTIVE_PANE_FD_SESSION_MATCH`] and paired for cases where
/// the FD alone would be ambiguous.
pub const ACTIVE_PANE_FD_COMMAND_SESSION_MATCH: &str = "active_pane_fd_command_session_match";

/// Process-tree evidence: the active pane's pid tree contains a
/// harness process. Lower identity weight than FD evidence
/// because it doesn't isolate a single session.
pub const ACTIVE_PANE_PROCESS_MATCH: &str = "active_pane_process_match";

/// Command-line evidence: the pane's start command carries a
/// harness session-key argument (e.g. `--resume <uuid>`). Weaker
/// than FD evidence and demoted by codex-log freshness per
/// ADR 0028 / ADR 0048.
pub const ACTIVE_PANE_COMMAND_SESSION_MATCH: &str = "active_pane_command_session_match";

/// Codex log-derived evidence: the codex `logs_*.sqlite` table
/// binds a process pid to a thread id (ADR 0048). Highest-weight
/// alongside [`HOOK_PROCESS_SESSION_MATCH`].
pub const CODEX_LOG_PROCESS_THREAD_MATCH: &str = "codex_log_process_thread_match";

/// Hook-sidecar evidence: the harness's SessionStart hook wrote
/// a sidecar record binding a launch pid to a session id
/// (ADR 0028). Same weight as
/// [`CODEX_LOG_PROCESS_THREAD_MATCH`].
pub const HOOK_PROCESS_SESSION_MATCH: &str = "hook_process_session_match";
