//! Agent harness discovery boundaries.
//!
//! Each supported harness ships with a small adapter implementing
//! [`HarnessAdapter`]. Adapters take the active [`DiscoveryContext`] and emit a
//! provider-neutral [`GraphFragment`] containing `AgentSession` nodes and the
//! source metadata needed to preserve provider provenance. Adapters must not
//! perform rendering, fork lineage resolution, or session/mux scoring – those
//! belong to higher layers.
//!
//! Most adapters look up a single state root via
//! [`DiscoveryContext::harness_state_root`]; per-repo harnesses such as aider
//! walk the configured scan roots instead. The [`HarnessDiscovery`] coordinator
//! is a [`DiscoveryProvider`] that runs every registered adapter and merges
//! fragments deterministically through [`merge_fragments`].

#[cfg(test)]
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::GraphSnapshot;

pub mod aider;
pub mod claude_code;
pub mod codex;
#[doc(hidden)]
pub mod fixtures;
pub mod opencode;

pub use aider::AiderAdapter;
pub use claude_code::ClaudeCodeAdapter;
pub use codex::CodexAdapter;
pub use opencode::OpenCodeAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarnessLaunchOption {
    pub id: &'static str,
    pub label: &'static str,
    pub argv: &'static [&'static str],
}

const CODEX_SKIP_PERMISSIONS_ARGV: &[&str] = &["--dangerously-bypass-approvals-and-sandbox"];
const CLAUDE_SKIP_PERMISSIONS_ARGV: &[&str] = &["--dangerously-skip-permissions"];

const CODEX_LAUNCH_OPTIONS: &[HarnessLaunchOption] = &[HarnessLaunchOption {
    id: "skip-permissions",
    label: "skip permissions",
    argv: CODEX_SKIP_PERMISSIONS_ARGV,
}];

const CLAUDE_CODE_LAUNCH_OPTIONS: &[HarnessLaunchOption] = &[HarnessLaunchOption {
    id: "skip-permissions",
    label: "skip permissions",
    argv: CLAUDE_SKIP_PERMISSIONS_ARGV,
}];

pub trait HarnessAdapter: Send + Sync {
    fn harness_key(&self) -> &str;

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;

    /// Default argv for spawning a fresh session of this harness when
    /// no per-pin `launch.argv` override is supplied (ADR 0057
    /// §Launch). Pure data; no I/O. Each adapter returns the bare
    /// binary invocation — model selection, prompt injection, and
    /// other harness-specific flags are intentionally out of v1 scope
    /// per ADR 0057 §Deferred. Operators who need richer launch
    /// commands use `pin.launch.argv` (a per-pin override).
    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        Vec::new()
    }

    /// Argv for resuming a specific session of this harness (ADR 0058).
    /// Returns `None` when the harness has no single-command resume
    /// CLI; callers fall back to `launch_argv` plus a status hint.
    ///
    /// `cwd` is provided for adapters that need context-aware resume
    /// (e.g., aider-style "load history from this directory"); the
    /// default-shape adapters that take `--resume <id>` ignore it.
    /// The default implementation returns `None` so adapters opt in
    /// explicitly.
    fn resume_argv(&self, session_id: &str, cwd: &Path) -> Option<Vec<std::ffi::OsString>> {
        let _ = (session_id, cwd);
        None
    }
}

/// Look up the per-harness default launch argv. Convenience for the
/// CLI launch path so it can resolve `pin.harness` → argv without
/// re-instantiating an adapter or walking the discovery registry.
pub fn launch_argv_for(harness_key: &str) -> Vec<std::ffi::OsString> {
    match harness_key {
        codex::HARNESS_KEY => CodexAdapter::new().launch_argv(),
        claude_code::HARNESS_KEY => ClaudeCodeAdapter::new().launch_argv(),
        opencode::HARNESS_KEY => OpenCodeAdapter::new().launch_argv(),
        aider::HARNESS_KEY => AiderAdapter::new().launch_argv(),
        _ => Vec::new(),
    }
}

pub fn launch_options_for(harness_key: &str) -> &'static [HarnessLaunchOption] {
    match harness_key {
        codex::HARNESS_KEY => CODEX_LAUNCH_OPTIONS,
        claude_code::HARNESS_KEY => CLAUDE_CODE_LAUNCH_OPTIONS,
        _ => &[],
    }
}

pub fn launch_option_for(harness_key: &str, option_id: &str) -> Option<HarnessLaunchOption> {
    launch_options_for(harness_key)
        .iter()
        .copied()
        .find(|option| option.id == option_id)
}

pub fn argv_contains_fragment(argv: &[String], fragment: &[&str]) -> bool {
    if fragment.is_empty() {
        return false;
    }
    argv.windows(fragment.len()).any(|window| {
        window
            .iter()
            .zip(fragment.iter())
            .all(|(actual, expected)| actual == expected)
    })
}

pub fn argv_with_fragment(mut argv: Vec<String>, fragment: &[&str]) -> Vec<String> {
    if fragment.is_empty() || argv_contains_fragment(&argv, fragment) {
        return argv;
    }
    argv.extend(fragment.iter().map(|arg| (*arg).to_string()));
    argv
}

pub fn argv_without_fragment(argv: Vec<String>, fragment: &[&str]) -> Vec<String> {
    if fragment.is_empty() {
        return argv;
    }
    let mut out = Vec::with_capacity(argv.len());
    let mut idx = 0;
    while idx < argv.len() {
        if idx + fragment.len() <= argv.len()
            && argv[idx..idx + fragment.len()]
                .iter()
                .zip(fragment.iter())
                .all(|(actual, expected)| actual == expected)
        {
            idx += fragment.len();
        } else {
            out.push(argv[idx].clone());
            idx += 1;
        }
    }
    out
}

pub fn strip_known_launch_option_fragments(argv: Vec<String>) -> Vec<String> {
    let mut stripped = argv;
    for options in [CODEX_LAUNCH_OPTIONS, CLAUDE_CODE_LAUNCH_OPTIONS] {
        for option in options {
            stripped = argv_without_fragment(stripped, option.argv);
        }
    }
    stripped
}

/// Look up the per-harness resume argv. Sibling of [`launch_argv_for`]
/// for the H-PIN-RESUME-004 launch path. Returns `None` when the
/// harness key is unknown or the adapter does not expose a resume
/// command (currently only `aider`, which tracks chat history
/// per-cwd rather than per-session).
pub fn resume_argv_for(
    harness_key: &str,
    session_id: &str,
    cwd: &Path,
) -> Option<Vec<std::ffi::OsString>> {
    match harness_key {
        codex::HARNESS_KEY => CodexAdapter::new().resume_argv(session_id, cwd),
        claude_code::HARNESS_KEY => ClaudeCodeAdapter::new().resume_argv(session_id, cwd),
        opencode::HARNESS_KEY => OpenCodeAdapter::new().resume_argv(session_id, cwd),
        aider::HARNESS_KEY => AiderAdapter::new().resume_argv(session_id, cwd),
        _ => None,
    }
}

#[derive(Default)]
pub struct HarnessDiscovery {
    adapters: Vec<Box<dyn HarnessAdapter>>,
}

impl HarnessDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter: impl HarnessAdapter + 'static) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    pub fn with_default_adapters() -> Self {
        Self::new()
            .with_adapter(CodexAdapter::new())
            .with_adapter(ClaudeCodeAdapter::new())
            .with_adapter(OpenCodeAdapter::new())
            .with_adapter(AiderAdapter::new())
    }
}

impl DiscoveryProvider for HarnessDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut fragments = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            fragments.push(adapter.discover(context)?);
        }

        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

pub(crate) fn snapshot_fragment(snapshot: GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
        node_provenance: snapshot.node_provenance,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

    struct StaticAdapter {
        key: &'static str,
        fragment: GraphFragment,
    }

    impl HarnessAdapter for StaticAdapter {
        fn harness_key(&self) -> &str {
            self.key
        }

        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(self.fragment.clone())
        }
    }

    struct StateAwareAdapter;

    impl HarnessAdapter for StateAwareAdapter {
        fn harness_key(&self) -> &str {
            "state-aware"
        }

        fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
            let Some(root) = context.harness_state_root(self.harness_key()) else {
                return Ok(GraphFragment::empty());
            };

            if !root.exists() {
                return Ok(GraphFragment::empty());
            }

            Ok(GraphFragment {
                nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("state-aware", "scope", "s1"),
                    harness_key: "state-aware".to_string(),
                    cwd: None,
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                })],
                candidate_links: Vec::new(),
                diagnostics: Vec::new(),
                node_provenance: BTreeMap::new(),
            })
        }
    }

    #[test]
    fn harness_discovery_without_adapters_returns_empty_fragment() {
        let fragment = HarnessDiscovery::new()
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment, GraphFragment::empty());
    }

    #[test]
    fn missing_state_directory_yields_empty_fragment_without_error() {
        let context =
            DiscoveryContext::default().with_harness_state_root("state-aware", "/does/not/exist");

        let fragment = HarnessDiscovery::new()
            .with_adapter(StateAwareAdapter)
            .discover(&context)
            .expect("missing state root does not error");

        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
    }

    #[test]
    fn adapter_receives_harness_state_root_from_context() {
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));

        struct Echo {
            captured: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
        }

        impl HarnessAdapter for Echo {
            fn harness_key(&self) -> &str {
                "codex"
            }

            fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
                *self.captured.lock().expect("lock") = context
                    .harness_state_root(self.harness_key())
                    .map(Path::to_path_buf);
                Ok(GraphFragment::empty())
            }
        }

        let context = DiscoveryContext::default().with_harness_state_root("codex", "/state/codex");

        HarnessDiscovery::new()
            .with_adapter(Echo {
                captured: captured.clone(),
            })
            .discover(&context)
            .expect("discovery succeeds");

        let observed = captured
            .lock()
            .expect("lock")
            .clone()
            .expect("state root passed to adapter");
        assert_eq!(observed, PathBuf::from("/state/codex"));
    }

    #[test]
    fn harness_discovery_merges_fragments_deterministically() {
        let session_alpha = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "scope", "alpha"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        });
        let session_beta = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "scope", "beta"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        });

        let fragment = HarnessDiscovery::new()
            .with_adapter(StaticAdapter {
                key: "codex-beta",
                fragment: GraphFragment {
                    nodes: vec![session_beta.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                    node_provenance: BTreeMap::new(),
                },
            })
            .with_adapter(StaticAdapter {
                key: "codex-alpha",
                fragment: GraphFragment {
                    nodes: vec![session_alpha.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                    node_provenance: BTreeMap::new(),
                },
            })
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment.nodes, vec![session_alpha, session_beta]);
    }

    // ----- H-PIN-RESUME-002: per-adapter resume_argv -----

    #[test]
    fn codex_resume_argv_matches_tui_resume_shape() {
        let argv =
            resume_argv_for("codex", "abc123", Path::new("/p")).expect("codex supports resume");
        assert_eq!(
            argv,
            vec![
                std::ffi::OsString::from("codex"),
                std::ffi::OsString::from("exec"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("abc123"),
            ]
        );
    }

    #[test]
    fn claude_code_resume_argv_matches_tui_resume_shape() {
        let argv = resume_argv_for("claude-code", "abc123", Path::new("/p"))
            .expect("claude-code supports resume");
        assert_eq!(
            argv,
            vec![
                std::ffi::OsString::from("claude"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("abc123"),
            ]
        );
    }

    #[test]
    fn aider_resume_argv_returns_none() {
        // Aider tracks chat history per-cwd, not per-session; no
        // resume CLI to splice in.
        assert!(resume_argv_for("aider", "abc123", Path::new("/p")).is_none());
    }

    #[test]
    fn opencode_resume_argv_matches_session_flag_shape() {
        // `opencode --session <id>` per `opencode --help` (the
        // earlier "unsupported" comment in src/tui/resume.rs was
        // stale; opencode added a session flag).
        let argv = resume_argv_for("opencode", "abc123", Path::new("/p"))
            .expect("opencode supports resume");
        assert_eq!(
            argv,
            vec![
                std::ffi::OsString::from("opencode"),
                std::ffi::OsString::from("--session"),
                std::ffi::OsString::from("abc123"),
            ]
        );
    }

    #[test]
    fn unknown_harness_resume_argv_returns_none() {
        assert!(resume_argv_for("nonesuch", "abc123", Path::new("/p")).is_none());
    }

    #[test]
    fn launch_options_include_skip_permissions_for_codex_and_claude() {
        let codex = launch_options_for("codex");
        assert_eq!(codex.len(), 1);
        assert_eq!(codex[0].id, "skip-permissions");
        assert_eq!(
            codex[0].argv,
            ["--dangerously-bypass-approvals-and-sandbox"]
        );

        let claude = launch_options_for("claude-code");
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].id, "skip-permissions");
        assert_eq!(claude[0].argv, ["--dangerously-skip-permissions"]);

        assert!(launch_options_for("aider").is_empty());
    }

    #[test]
    fn launch_option_fragment_helpers_preserve_manual_tokens() {
        let argv = vec![
            "sandbox".to_string(),
            "run".to_string(),
            "codex".to_string(),
        ];
        let argv = argv_with_fragment(argv, CODEX_SKIP_PERMISSIONS_ARGV);
        assert_eq!(
            argv,
            vec![
                "sandbox".to_string(),
                "run".to_string(),
                "codex".to_string(),
                "--dangerously-bypass-approvals-and-sandbox".to_string(),
            ]
        );
        assert!(argv_contains_fragment(&argv, CODEX_SKIP_PERMISSIONS_ARGV));

        let stripped = argv_without_fragment(argv, CODEX_SKIP_PERMISSIONS_ARGV);
        assert_eq!(
            stripped,
            vec![
                "sandbox".to_string(),
                "run".to_string(),
                "codex".to_string()
            ]
        );
    }

    #[test]
    fn strip_known_launch_option_fragments_removes_cross_harness_flags() {
        let argv = vec![
            "codex".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        assert_eq!(
            strip_known_launch_option_fragments(argv),
            vec!["codex".to_string()]
        );
    }

    #[test]
    fn default_trait_impl_returns_none() {
        // Adapters that don't override get None for free.
        struct NoOpAdapter;
        impl HarnessAdapter for NoOpAdapter {
            fn harness_key(&self) -> &str {
                "noop"
            }
            fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
                Ok(GraphFragment::empty())
            }
        }
        assert!(NoOpAdapter.resume_argv("abc", Path::new("/p")).is_none());
    }
}
