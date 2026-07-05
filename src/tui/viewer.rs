//! External transcript-viewer launch (H-TRANSCRIPT-012, ADR 0019).
//!
//! Resolve the operator's selected agent session into a child-process
//! viewer hand-off. The runtime mirrors P8-010's `tmux attach`
//! pattern: leave the alt screen, exec the viewer, wait for it to
//! exit, then re-enter the alt screen.
//!
//! Per ADR 0052 this is now the **escape-hatch** path, not the
//! default. The default `T` target is the native in-tree viewer
//! (`H-VIEWER-NATIVE-*`). The external launch survives for
//! operators who prefer `claude-history`'s ledger formatting or
//! who configure another viewer via `[viewers.<harness>]`
//! (`H-TRANSCRIPT-013`).
//!
//! Resolution takes a [`BinaryProbe`] seam so tests can simulate
//! PATH state without touching the host. `plan()` may consult the
//! filesystem (e.g. globbing for a Claude Code JSONL file) and
//! returns `Err` with a one-line hint when no viable invocation
//! can be assembled. v1 of the escape-hatch ships one backend:
//!   - [`ClaudeHistoryViewer`] — resolves the session's on-disk
//!     JSONL by globbing
//!     `<state_scope>/projects/*/<session_key>.jsonl` and passes
//!     the file path as a positional argument
//!     (`claude-history --show-id` *prints* the id; the
//!     interactive viewer takes a file path).

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

/// PATH-discovery seam. Production uses [`PathBinaryProbe`]; tests
/// inject [`FakeBinaryProbe`].
pub trait BinaryProbe {
    fn on_path(&self, binary: &str) -> bool;
}

/// Walk `$PATH` looking for an executable. No new dependency — the
/// std-only lookup is short enough to live here, matching ADR 0024's
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
            args: vec![file_str],
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

/// Backend preference order. Centralized so the resolver and the
/// test layout agree. Today this is a single-entry slice;
/// `H-TRANSCRIPT-013` will extend it with config-defined viewers.
fn default_backends() -> [&'static dyn SessionViewerAction; 1] {
    [&ClaudeHistoryViewer]
}

/// Resolve a viewer launch for a session, given a PATH probe.
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
    // Walk backends in preference order. We remember the *first*
    // viable backend's plan error so we can surface
    // `TranscriptNotFound` when no backend's plan() succeeds.
    let mut last_plan_error: Option<(String, String)> = None;
    for backend in &supported {
        if !probe.on_path(backend.binary()) {
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

    /// Test seam: records probed binary names and returns membership
    /// in a fixed set of "installed" binaries.
    struct FakeBinaryProbe {
        installed: Vec<&'static str>,
        probed: RefCell<Vec<String>>,
    }

    impl FakeBinaryProbe {
        fn new(installed: &[&'static str]) -> Self {
            Self {
                installed: installed.to_vec(),
                probed: RefCell::new(Vec::new()),
            }
        }
    }

    impl BinaryProbe for FakeBinaryProbe {
        fn on_path(&self, binary: &str) -> bool {
            self.probed.borrow_mut().push(binary.to_string());
            self.installed.contains(&binary)
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
            other @ ViewerTarget::Disabled(_) => panic!("expected Launch, got {other:?}"),
        }
    }

    #[test]
    fn claude_history_missing_transcript_reports_transcript_not_found() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::TranscriptNotFound { binary, hint }) => {
                assert_eq!(binary, "claude-history");
                assert!(hint.contains("sess-uuid-abc"), "got {hint:?}");
            }
            other => panic!("expected TranscriptNotFound, got {other:?}"),
        }
    }

    #[test]
    fn aider_is_unsupported() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        match resolve_viewer_target(&session("aider"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
                assert_eq!(harness_key, "aider");
            }
            other => panic!("expected UnsupportedHarness, got {other:?}"),
        }
    }

    #[test]
    fn unknown_harness_is_unsupported() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        match resolve_viewer_target(&session("mystery"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
                assert_eq!(harness_key, "mystery");
            }
            other => panic!("expected UnsupportedHarness, got {other:?}"),
        }
    }

    /// Non-claude harnesses have no escape-hatch backend registered.
    /// Operators wanting Codex / OpenCode external viewers will
    /// configure them via `H-TRANSCRIPT-013`.
    #[test]
    fn codex_has_no_escape_hatch_backend() {
        let probe = FakeBinaryProbe::new(&["claude-history"]);
        match resolve_viewer_target(&session("codex"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
                assert_eq!(harness_key, "codex");
            }
            other => panic!("expected UnsupportedHarness, got {other:?}"),
        }
    }

    #[test]
    fn claude_code_with_no_binary_reports_install_hint() {
        let probe = FakeBinaryProbe::new(&[]);
        match resolve_viewer_target(&session("claude-code"), &probe) {
            ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
                binaries,
                harness_key,
            }) => {
                assert_eq!(harness_key, "claude-code");
                assert_eq!(binaries, vec!["claude-history"]);
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
                binaries: vec!["claude-history".to_string()],
                harness_key: "claude-code".to_string(),
            }),
            "view: install claude-history to view claude-code sessions"
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
