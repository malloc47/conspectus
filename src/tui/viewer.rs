//! External transcript-viewer launch (H-TRANSCRIPT-012, ADR 0019).
//!
//! Resolve the operator's selected agent session into a child-process
//! viewer hand-off. The runtime mirrors P8-010's `tmux attach`
//! pattern: leave the alt screen, exec the viewer, wait for it to
//! exit, then re-enter the alt screen.
//!
//! Resolution takes a [`BinaryProbe`] seam so tests can simulate
//! PATH state and per-binary capability without touching the host.
//! `plan()` may consult the filesystem (e.g. globbing for a Claude
//! Code JSONL file) and returns `Err` with a one-line hint when no
//! viable invocation can be assembled. v1 ships two backends per
//! the May 2026 ADR 0019 survey:
//!   - [`ClaudeHistoryViewer`] — resolves the session's on-disk
//!     JSONL by globbing
//!     `<state_scope>/projects/*/<session_key>.jsonl` and passes
//!     the file path as a positional argument
//!     (`claude-history --show-id` *prints* the id; the interactive
//!     viewer takes a file path).
//!   - [`RecallViewer`] — `recall --session <id>` for Claude Code,
//!     Codex, OpenCode, and Factory/Droid sessions.
//!
//! `recall` 0.5.0 upstream has no `--session` flag, so launching it
//! against an arbitrary session would drop the operator into the
//! search picker. The conspectus Nix overlay patches `--session`
//! into `recall` (version `0.5.0-conspectus-session`). The resolver
//! treats any `recall` whose `--help` does not advertise `--session`
//! as missing, so an unpatched upstream install fails closed rather
//! than launching a useless picker.
//!
//! Selection prefers the harness-specific backend over the
//! multi-harness backend when both binaries are on `PATH` and both
//! advertise their required flags.

use std::path::PathBuf;

use crate::model::AgentSessionId;

/// Concrete invocation plan for a viewer launch. The runtime owns
/// the alt-screen suspend/resume; this struct describes only what
/// to exec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    /// Human-readable label for the status bar (e.g.
    /// `claude-history <uuid>`).
    pub label: String,
}

/// Outcome of resolving a viewer target for the current selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewerTarget {
    Launch(LaunchPlan),
    Disabled(ViewerDisabled),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewerDisabled {
    /// No row selected.
    NoSelection,
    /// Selection isn't an agent session (group row, mux row,
    /// candidate child, etc.).
    UnsupportedRow,
    /// The session's harness has no viewer registered in v1.
    UnsupportedHarness { harness_key: String },
    /// One or more viewers support this harness, but none are on
    /// `PATH`. Carries the binary names so the status bar can
    /// suggest what to install.
    BinaryNotInstalled {
        binaries: Vec<String>,
        harness_key: String,
    },
    /// A viable backend was identified but its session-resolution
    /// step (e.g. globbing the on-disk JSONL) found nothing. Carries
    /// the binary name + a one-line hint for diagnostics.
    TranscriptNotFound { binary: String, hint: String },
}

/// PATH-discovery + capability seam. Production uses
/// [`PathBinaryProbe`]; tests inject [`FakeBinaryProbe`].
///
/// `supports_flag` exists because `recall` 0.5.0 upstream does not
/// expose a session-deep-link flag — invoking it without `--session`
/// drops the operator into the search picker, which defeats the
/// reason conspectus is launching it at all. The conspectus Nix
/// build patches `--session <ID>` in (and bumps the version to
/// `0.5.0-conspectus-session`); this probe lets us treat
/// unpatched / pre-flag installations as "not viable" rather than
/// crashing through a useless launch.
pub trait BinaryProbe {
    fn on_path(&self, binary: &str) -> bool;
    /// Spawn `<binary> --help` and check whether `flag` appears in
    /// the help text. Returns false on any spawn error.
    fn supports_flag(&self, binary: &str, flag: &str) -> bool;
}

/// Walk `$PATH` looking for an executable, and feature-detect by
/// scraping `<binary> --help`. No new dependency — the std-only
/// lookup is short enough to live here, matching ADR 0024's
/// "prefer hand-rolled first" stance for non-core surfaces.
pub struct PathBinaryProbe;

impl BinaryProbe for PathBinaryProbe {
    fn on_path(&self, binary: &str) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        for entry in std::env::split_paths(&path) {
            let mut candidate: PathBuf = entry;
            candidate.push(binary);
            if candidate.is_file() {
                return true;
            }
        }
        false
    }

    fn supports_flag(&self, binary: &str, flag: &str) -> bool {
        let Ok(output) = std::process::Command::new(binary).arg("--help").output() else {
            return false;
        };
        let haystack = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
        haystack
            .windows(flag.len())
            .any(|window| window == flag.as_bytes())
    }
}

/// Action-resolver trait per ADR 0019. Each backend knows the
/// binary it would launch, which harness keys it supports, and how
/// to assemble a [`LaunchPlan`] for a session.
pub trait SessionViewerAction {
    /// Stable backend key used in diagnostics.
    fn key(&self) -> &str;
    /// True if this backend can render the given harness.
    fn supports(&self, harness_key: &str) -> bool;
    /// Binary name probed against `$PATH`.
    fn binary(&self) -> &str;
    /// Flags the binary must advertise in `--help` for this backend
    /// to be considered viable. Default empty (any installation
    /// works). Backends like `recall` override to require a
    /// patched / future flag.
    fn required_flags(&self) -> &'static [&'static str] {
        &[]
    }
    /// Construct the exec invocation for a session. May consult the
    /// filesystem to resolve harness-specific paths (e.g.
    /// `claude-history` needs the on-disk JSONL file). Returns
    /// `Err` with a one-line hint when the session can't be
    /// resolved into something this backend can launch.
    fn plan(&self, session: &AgentSessionId) -> Result<LaunchPlan, String>;
}

/// `claude-history` (raine/claude-history): mature terminal viewer
/// for Claude Code. The interactive viewer takes the conversation's
/// JSONL file as a positional argument; there is no
/// "open by session id" flag (the `--show-id` flag *prints* the id,
/// not opens it). We resolve the file by globbing
/// `<state_scope>/projects/*/<session_key>.jsonl`, which matches the
/// Claude Code state layout the harness adapter scans.
pub struct ClaudeHistoryViewer;

impl SessionViewerAction for ClaudeHistoryViewer {
    fn key(&self) -> &str {
        "claude-history"
    }

    fn supports(&self, harness_key: &str) -> bool {
        harness_key == "claude-code"
    }

    fn binary(&self) -> &str {
        "claude-history"
    }

    fn plan(&self, session: &AgentSessionId) -> Result<LaunchPlan, String> {
        let file = find_claude_session_file(&session.state_scope, &session.session_key)
            .ok_or_else(|| {
                format!(
                    "no transcript under {}/projects/*/{}.jsonl",
                    session.state_scope, session.session_key
                )
            })?;
        let file_str = file.to_string_lossy().into_owned();
        Ok(LaunchPlan {
            program: self.binary().to_string(),
            args: vec![file_str.clone()],
            label: format!("{} {}", self.key(), session.session_key),
        })
    }
}

/// Walk `<state_scope>/projects/*/` looking for `<session_key>.jsonl`.
/// Claude Code stores one project subdir per cwd (with `/` chars
/// transformed); the session UUID is the file stem. Std-only — no
/// `glob` dependency.
fn find_claude_session_file(state_scope: &str, session_key: &str) -> Option<PathBuf> {
    let projects = PathBuf::from(state_scope).join("projects");
    let entries = std::fs::read_dir(&projects).ok()?;
    let needle = format!("{session_key}.jsonl");
    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let candidate = project_dir.join(&needle);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// `recall` (zippoxer/recall): multi-harness search and resume TUI.
/// Conspectus depends on the `--session <ID>` deep-link flag —
/// without it `recall` opens a search picker, defeating the purpose
/// of launching from a conspectus selection. The flag ships in the
/// conspectus Nix overlay's patched `recall` build. Installations
/// that don't advertise `--session` in `--help` are treated as
/// missing.
pub struct RecallViewer;

const RECALL_REQUIRED_FLAGS: &[&str] = &["--session"];

impl SessionViewerAction for RecallViewer {
    fn key(&self) -> &str {
        "recall"
    }

    fn supports(&self, harness_key: &str) -> bool {
        matches!(
            harness_key,
            "claude-code" | "codex" | "opencode" | "factory" | "droid"
        )
    }

    fn binary(&self) -> &str {
        "recall"
    }

    fn required_flags(&self) -> &'static [&'static str] {
        RECALL_REQUIRED_FLAGS
    }

    fn plan(&self, session: &AgentSessionId) -> Result<LaunchPlan, String> {
        Ok(LaunchPlan {
            program: self.binary().to_string(),
            args: vec!["--session".to_string(), session.session_key.clone()],
            label: format!("{} {}", self.key(), session.session_key),
        })
    }
}

/// Backend preference order: harness-specific first, multi-harness
/// fallback. Centralized so the resolver and the test layout agree.
fn default_backends() -> [&'static dyn SessionViewerAction; 2] {
    [&ClaudeHistoryViewer, &RecallViewer]
}

/// Resolve a viewer launch for a session, given a PATH probe.
/// Preference: harness-specific backend over multi-harness when both
/// are present on PATH (ADR 0019).
pub fn resolve_viewer_target(session: &AgentSessionId, probe: &dyn BinaryProbe) -> ViewerTarget {
    let backends = default_backends();
    let supported: Vec<&dyn SessionViewerAction> = backends
        .into_iter()
        .filter(|backend| backend.supports(&session.harness_key))
        .collect();
    if supported.is_empty() {
        return ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness {
            harness_key: session.harness_key.clone(),
        });
    }
    // Walk backends in preference order. A backend is "viable" if
    // its binary is on PATH and advertises every required flag.
    // We remember the *first* viable backend so we can fall through
    // to it for the disabled reason if plan() refuses every viable
    // backend (e.g. transcript file missing on disk).
    let mut last_plan_error: Option<(String, String)> = None;
    for backend in &supported {
        if !probe.on_path(backend.binary()) {
            continue;
        }
        let missing_flag = backend
            .required_flags()
            .iter()
            .find(|flag| !probe.supports_flag(backend.binary(), flag));
        if missing_flag.is_some() {
            continue;
        }
        match backend.plan(session) {
            Ok(plan) => return ViewerTarget::Launch(plan),
            Err(hint) => {
                last_plan_error = Some((backend.binary().to_string(), hint));
            }
        }
    }
    if let Some((binary, hint)) = last_plan_error {
        return ViewerTarget::Disabled(ViewerDisabled::TranscriptNotFound { binary, hint });
    }
    ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
        binaries: supported.iter().map(|b| b.binary().to_string()).collect(),
        harness_key: session.harness_key.clone(),
    })
}

/// Status-bar text for a disabled viewer launch.
pub fn viewer_disabled_reason(reason: &ViewerDisabled) -> String {
    match reason {
        ViewerDisabled::NoSelection => "view: no row selected".to_string(),
        ViewerDisabled::UnsupportedRow => "view: select an agent session row".to_string(),
        ViewerDisabled::UnsupportedHarness { harness_key } => {
            format!("view: no viewer registered for {harness_key}")
        }
        ViewerDisabled::BinaryNotInstalled {
            binaries,
            harness_key,
        } => {
            let install = match binaries.as_slice() {
                [single] => single.clone(),
                many => many.join(" or "),
            };
            format!("view: install {install} to view {harness_key} sessions")
        }
        ViewerDisabled::TranscriptNotFound { binary, hint } => {
            format!("view: {binary} cannot resolve transcript — {hint}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Test seam. `installed` declares which binaries the fake PATH
    /// holds. `flags_by_binary` declares which `--help` flags each
    /// installed binary advertises; entries default to "all required
    /// flags supported" when a binary is `installed` but missing from
    /// the map, so tests that don't care about capability gating
    /// stay compact.
    struct FakeBinaryProbe {
        installed: Vec<&'static str>,
        flags_by_binary: std::collections::HashMap<&'static str, Vec<&'static str>>,
        probed: RefCell<Vec<String>>,
    }

    impl FakeBinaryProbe {
        fn new(installed: &[&'static str]) -> Self {
            // Default: every installed binary advertises every flag
            // any backend asks about. Tests that want to model an
            // unpatched binary call `without_flag`.
            Self {
                installed: installed.to_vec(),
                flags_by_binary: std::collections::HashMap::new(),
                probed: RefCell::new(Vec::new()),
            }
        }

        /// Declare that `binary` is installed but its `--help` does
        /// *not* advertise `flag`. Mirrors an unpatched upstream
        /// `recall` whose `--help` lacks `--session`.
        fn without_flag(mut self, binary: &'static str, missing: &'static str) -> Self {
            let entry = self.flags_by_binary.entry(binary).or_default();
            entry.retain(|f| *f != missing);
            // Sentinel: presence of the entry (even empty) flips the
            // default-allow behavior off for that binary; the probe
            // returns true only for flags explicitly listed below.
            // For the unpatched-recall case the listed set is empty,
            // so any flag query returns false.
            self
        }
    }

    impl BinaryProbe for FakeBinaryProbe {
        fn on_path(&self, binary: &str) -> bool {
            self.probed.borrow_mut().push(binary.to_string());
            self.installed.contains(&binary)
        }

        fn supports_flag(&self, binary: &str, flag: &str) -> bool {
            match self.flags_by_binary.get(binary) {
                Some(flags) => flags.contains(&flag),
                None => true,
            }
        }
    }

    fn session(harness: &str) -> AgentSessionId {
        AgentSessionId::new(harness, "/state", "sess-uuid-abc")
    }

    /// Lay down a fake `<state>/projects/<proj>/<session_key>.jsonl`
    /// so `ClaudeHistoryViewer::plan` can resolve a file path.
    /// Returns the temp dir + the session id it created.
    fn claude_fixture(session_key: &str) -> (tempfile::TempDir, AgentSessionId) {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().join("projects").join("-home-user-proj");
        std::fs::create_dir_all(&project).expect("project dir");
        std::fs::write(project.join(format!("{session_key}.jsonl")), "{}\n").expect("seed jsonl");
        let id = AgentSessionId::new("claude-code", dir.path().to_string_lossy(), session_key);
        (dir, id)
    }

    #[test]
    fn claude_code_with_claude_history_installed_launches_claude_history() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        let (_tmp, id) = claude_fixture("sess-uuid-abc");
        match resolve_viewer_target(&id, &probe) {
            ViewerTarget::Launch(plan) => {
                assert_eq!(plan.program, "claude-history");
                assert_eq!(plan.args.len(), 1, "single positional arg = jsonl path");
                assert!(
                    plan.args[0].ends_with("/projects/-home-user-proj/sess-uuid-abc.jsonl"),
                    "got {:?}",
                    plan.args[0]
                );
            }
            other => panic!("expected Launch, got {other:?}"),
        }
    }

    #[test]
    fn claude_code_prefers_claude_history_over_recall_when_both_installed() {
        let probe = FakeBinaryProbe::new(&["claude-history", "recall"]);
        let (_tmp, id) = claude_fixture("sess-uuid-abc");
        match resolve_viewer_target(&id, &probe) {
            ViewerTarget::Launch(plan) => {
                assert_eq!(
                    plan.program, "claude-history",
                    "harness-specific backend wins per ADR 0019"
                );
            }
            other => panic!("expected Launch, got {other:?}"),
        }
    }

    #[test]
    fn claude_code_falls_back_to_recall_when_only_recall_installed() {
        let probe = FakeBinaryProbe::new(&["recall"]);
        // No fixture: ClaudeHistoryViewer can't resolve a file, but
        // it isn't on PATH anyway so it's skipped before plan() runs.
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Launch(plan) => {
                assert_eq!(plan.program, "recall");
                assert_eq!(plan.args, vec!["--session", "sess-uuid-abc"]);
            }
            other => panic!("expected Launch, got {other:?}"),
        }
    }

    /// When claude-history is on PATH but no transcript file exists,
    /// the resolver falls through to recall if available. If recall
    /// is also unavailable, surface TranscriptNotFound rather than
    /// silently failing.
    #[test]
    fn claude_history_missing_transcript_reports_transcript_not_found() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        // Note: no fixture; state_scope=/state has no projects dir.
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::TranscriptNotFound { binary, hint }) => {
                assert_eq!(binary, "claude-history");
                assert!(hint.contains("sess-uuid-abc"), "got {hint:?}");
            }
            other => panic!("expected TranscriptNotFound, got {other:?}"),
        }
    }

    /// When claude-history can't find the transcript but recall is
    /// installed, recall takes the launch — this is the
    /// missing-on-disk-but-recall-indexed safety net.
    #[test]
    fn claude_history_missing_transcript_falls_through_to_recall() {
        let probe = FakeBinaryProbe::new(&["claude-history", "recall"]);
        // No fixture for claude-history; recall on PATH so it wins.
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Launch(plan) => {
                assert_eq!(plan.program, "recall");
                assert_eq!(plan.args, vec!["--session", "sess-uuid-abc"]);
            }
            other => panic!("expected recall fallback, got {other:?}"),
        }
    }

    #[test]
    fn codex_uses_recall_only() {
        let probe = FakeBinaryProbe::new(&["recall"]);
        match resolve_viewer_target(&session("codex"), &probe) {
            ViewerTarget::Launch(plan) => {
                assert_eq!(plan.program, "recall");
                assert_eq!(plan.args, vec!["--session", "sess-uuid-abc"]);
            }
            other => panic!("expected Launch, got {other:?}"),
        }
    }

    #[test]
    fn opencode_uses_recall_only() {
        let probe = FakeBinaryProbe::new(&["recall"]);
        match resolve_viewer_target(&session("opencode"), &probe) {
            ViewerTarget::Launch(plan) => assert_eq!(plan.program, "recall"),
            other => panic!("expected Launch, got {other:?}"),
        }
    }

    /// Unpatched upstream `recall` (0.5.0) advertises no `--session`
    /// flag in `--help`. The resolver must treat it as missing
    /// rather than launching it into the search picker.
    #[test]
    fn recall_without_session_flag_is_treated_as_missing() {
        let probe = FakeBinaryProbe::new(&["recall"]).without_flag("recall", "--session");
        match resolve_viewer_target(&session("codex"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
                binaries,
                harness_key,
            }) => {
                assert_eq!(harness_key, "codex");
                assert_eq!(binaries, vec!["recall"]);
            }
            other => panic!("unpatched recall should be reported as not-installed, got {other:?}"),
        }
    }

    /// Even when claude-history is installed alongside an unpatched
    /// recall, a Claude Code row still prefers claude-history — but
    /// the unpatched recall must not silently win for sessions that
    /// claude-history doesn't cover.
    #[test]
    fn claude_history_still_wins_when_recall_lacks_session_flag() {
        let probe =
            FakeBinaryProbe::new(&["claude-history", "recall"]).without_flag("recall", "--session");
        let (_tmp, id) = claude_fixture("sess-uuid-abc");
        match resolve_viewer_target(&id, &probe) {
            ViewerTarget::Launch(plan) => assert_eq!(plan.program, "claude-history"),
            other => panic!("expected claude-history, got {other:?}"),
        }
        // ...and for codex the unpatched recall stays disabled.
        match resolve_viewer_target(&session("codex"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled { .. }) => {}
            other => panic!("codex should report not-installed, got {other:?}"),
        }
    }

    #[test]
    fn aider_is_unsupported() {
        let probe = FakeBinaryProbe::new(&["claude-history", "recall"]);
        match resolve_viewer_target(&session("aider"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
                assert_eq!(harness_key, "aider");
            }
            other => panic!("expected UnsupportedHarness, got {other:?}"),
        }
    }

    #[test]
    fn unknown_harness_is_unsupported() {
        let probe = FakeBinaryProbe::new(&["claude-history", "recall"]);
        match resolve_viewer_target(&session("mystery"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
                assert_eq!(harness_key, "mystery");
            }
            other => panic!("expected UnsupportedHarness, got {other:?}"),
        }
    }

    #[test]
    fn claude_code_with_no_binaries_lists_both_in_reason() {
        let probe = FakeBinaryProbe::new(&[]);
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
                binaries,
                harness_key,
            }) => {
                assert_eq!(harness_key, "claude-code");
                assert_eq!(binaries, vec!["claude-history", "recall"]);
            }
            other => panic!("expected BinaryNotInstalled, got {other:?}"),
        }
    }

    #[test]
    fn codex_with_no_binaries_lists_only_recall() {
        let probe = FakeBinaryProbe::new(&[]);
        match resolve_viewer_target(&session("codex"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
                binaries,
                harness_key,
            }) => {
                assert_eq!(harness_key, "codex");
                assert_eq!(binaries, vec!["recall"]);
            }
            other => panic!("expected BinaryNotInstalled, got {other:?}"),
        }
    }

    #[test]
    fn disabled_reason_text_covers_every_variant() {
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::NoSelection),
            "view: no row selected"
        );
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::UnsupportedRow),
            "view: select an agent session row"
        );
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::UnsupportedHarness {
                harness_key: "aider".to_string()
            }),
            "view: no viewer registered for aider"
        );
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::BinaryNotInstalled {
                binaries: vec!["claude-history".to_string(), "recall".to_string()],
                harness_key: "claude-code".to_string(),
            }),
            "view: install claude-history or recall to view claude-code sessions"
        );
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::BinaryNotInstalled {
                binaries: vec!["recall".to_string()],
                harness_key: "codex".to_string(),
            }),
            "view: install recall to view codex sessions"
        );
        assert_eq!(
            viewer_disabled_reason(&ViewerDisabled::TranscriptNotFound {
                binary: "claude-history".to_string(),
                hint: "no transcript under /state/projects/*/sess.jsonl".to_string(),
            }),
            "view: claude-history cannot resolve transcript — no transcript under /state/projects/*/sess.jsonl"
        );
    }
}
