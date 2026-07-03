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
    /// Stable identifier for this harness. Every production impl
    /// returns a `&'static str` literal (typically the module's
    /// `HARNESS_KEY` const), which matches how registry-derived
    /// consumers expect to consume the key (TUI filter menu items,
    /// resume dispatch keys, provider stamps).
    fn harness_key(&self) -> &'static str;

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;

    /// Short display label rendered in the TUI's row label column
    /// and filter chips (H-EXT-002). Defaults to the harness key;
    /// adapters override when the key is longer than the display
    /// budget — `claude-code` collapses to `claude` per H-TBL-014.
    fn display_label(&self) -> &'static str {
        self.harness_key()
    }

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

    /// Launch-time options surfaced in the pin-create form
    /// (H-EXT-002). Each entry is a stable-id + label + argv
    /// fragment; the pin editor lets the operator toggle each on
    /// or off, and the resulting argv appends every toggled
    /// option's fragment to [`Self::launch_argv`]. Defaults to
    /// no options — most adapters ship without any and rely on
    /// per-pin `launch.argv` overrides for exotic invocations.
    fn launch_options(&self) -> &'static [HarnessLaunchOption] {
        &[]
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

/// Canonical set of registered harness adapters (H-EXT-002).
/// The six parallel-table functions below iterate this list
/// instead of a hand-rolled match. Registering a new harness is
/// a two-step operation: add the adapter to
/// [`HarnessDiscovery::with_default_adapters`] and to
/// [`registered_adapters`]; the launch / resume / label / filter
/// / options surfaces pick it up automatically.
///
/// The list is a static `LazyLock` because adapter values are
/// `Send + Sync` zero-state and outlive the process.
static REGISTERED_ADAPTERS: std::sync::LazyLock<Vec<Box<dyn HarnessAdapter>>> =
    std::sync::LazyLock::new(|| {
        // Order matches the pre-H-EXT-002 hardcoded
        // `HARNESS_OPTIONS` slice so the TUI filter menu order
        // stays byte-identical. Discovery iteration order
        // (via `HarnessDiscovery::with_default_adapters`)
        // uses a separate registration and can differ if a new
        // adapter is added mid-list; keep both in sync when
        // adding.
        vec![
            Box::new(ClaudeCodeAdapter::new()),
            Box::new(CodexAdapter::new()),
            Box::new(OpenCodeAdapter::new()),
            Box::new(AiderAdapter::new()),
        ]
    });

/// Iterate every registered harness adapter (H-EXT-002). Used by
/// TUI filter menus, launch / resume dispatch, row-label lookup,
/// and the launch-options aggregator. Order is stable and
/// matches [`HarnessDiscovery::with_default_adapters`] so
/// registry-derived UI surfaces don't reshuffle when a new
/// adapter is added mid-list.
pub fn registered_adapters() -> impl Iterator<Item = &'static dyn HarnessAdapter> {
    REGISTERED_ADAPTERS.iter().map(|b| &**b)
}

/// Find a registered adapter by key (H-EXT-002). Returns `None`
/// when the key doesn't match any registered adapter.
fn adapter_for(harness_key: &str) -> Option<&'static dyn HarnessAdapter> {
    registered_adapters().find(|a| a.harness_key() == harness_key)
}

/// Look up the per-harness default launch argv. Convenience for the
/// CLI launch path so it can resolve `pin.harness` → argv without
/// re-instantiating an adapter or walking the discovery registry.
pub fn launch_argv_for(harness_key: &str) -> Vec<std::ffi::OsString> {
    adapter_for(harness_key)
        .map(|a| a.launch_argv())
        .unwrap_or_default()
}

pub fn launch_options_for(harness_key: &str) -> &'static [HarnessLaunchOption] {
    adapter_for(harness_key)
        .map(|a| a.launch_options())
        .unwrap_or(&[])
}

pub fn launch_option_for(harness_key: &str, option_id: &str) -> Option<HarnessLaunchOption> {
    launch_options_for(harness_key)
        .iter()
        .copied()
        .find(|option| option.id == option_id)
}

/// Registered harness keys in declaration order (H-EXT-002).
/// Used by the TUI controls overlay's harness filter menu — the
/// pre-H-EXT-002 caller was a hardcoded
/// `HARNESS_OPTIONS: &[&str]` in `tui/widgets/controls.rs`.
///
/// Wrapped in a `LazyLock<Vec<&'static str>>` so the &-of-slice
/// return type is `'static` (the multi-select widget needs
/// `&[&'static str]`).
pub fn harness_keys() -> &'static [&'static str] {
    static KEYS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
        REGISTERED_ADAPTERS
            .iter()
            .map(|a| a.harness_key())
            .collect()
    });
    KEYS.as_slice()
}

/// Short display label for a harness key. Used by the row-label
/// column (H-TBL-014, H-EXT-002). Returns the key itself as an
/// owned string for unknown harnesses (previous behavior via the
/// `harness_label` catch-all in `tui/rows/mod.rs`).
pub fn display_label_for(harness_key: &str) -> String {
    match adapter_for(harness_key) {
        Some(a) => a.display_label().to_string(),
        None => harness_key.to_string(),
    }
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
    for adapter in registered_adapters() {
        for option in adapter.launch_options() {
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
    adapter_for(harness_key).and_then(|a| a.resume_argv(session_id, cwd))
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
mod registry_tests {
    //! H-EXT-002 anchor tests. These pin the registry-derived
    //! surfaces (label lookup, launch options, keys list, resume
    //! argv) so a rename or accidental removal of an adapter's
    //! override breaks a targeted test rather than a downstream
    //! snapshot.
    use super::*;

    #[test]
    fn harness_keys_matches_registered_adapter_order() {
        // Ordering matches the pre-H-EXT-002 hardcoded
        // `HARNESS_OPTIONS` slice so the TUI filter menu stays
        // byte-identical.
        assert_eq!(
            harness_keys(),
            &["claude-code", "codex", "opencode", "aider"]
        );
    }

    #[test]
    fn display_label_for_collapses_claude_code() {
        // H-TBL-014 collapse lives on the adapter (H-EXT-002)
        // rather than a match table in `tui/rows`.
        assert_eq!(display_label_for("claude-code"), "claude");
    }

    #[test]
    fn display_label_for_defaults_to_key() {
        assert_eq!(display_label_for("codex"), "codex");
        assert_eq!(display_label_for("opencode"), "opencode");
        assert_eq!(display_label_for("aider"), "aider");
    }

    #[test]
    fn display_label_for_unknown_echoes_key() {
        // Preserves the pre-H-EXT-002 catch-all behavior in
        // `harness_label`.
        assert_eq!(display_label_for("never-registered"), "never-registered");
    }

    #[test]
    fn launch_options_for_carries_permission_overrides() {
        // codex and claude-code both surface a skip-permissions
        // option; the others don't.
        assert_eq!(launch_options_for("codex").len(), 1);
        assert_eq!(launch_options_for("codex")[0].id, "skip-permissions");
        assert_eq!(launch_options_for("claude-code").len(), 1);
        assert_eq!(launch_options_for("claude-code")[0].id, "skip-permissions");
        assert!(launch_options_for("opencode").is_empty());
        assert!(launch_options_for("aider").is_empty());
        assert!(launch_options_for("never-registered").is_empty());
    }

    #[test]
    fn resume_argv_for_dispatches_via_registry() {
        let cwd = Path::new("/tmp");
        assert_eq!(
            resume_argv_for("claude-code", "sess-123", cwd),
            Some(vec![
                std::ffi::OsString::from("claude"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("sess-123"),
            ])
        );
        assert!(resume_argv_for("aider", "sess-123", cwd).is_none());
        assert!(resume_argv_for("never-registered", "sess", cwd).is_none());
    }

    #[test]
    fn launch_argv_for_dispatches_via_registry() {
        // Registered adapters return non-empty argv; unknown
        // harnesses return an empty vec (matches the pre-H-EXT-002
        // catch-all).
        for key in harness_keys() {
            assert!(
                !launch_argv_for(key).is_empty(),
                "registered adapter `{key}` returned an empty launch argv",
            );
        }
        assert!(launch_argv_for("never-registered").is_empty());
    }

    #[test]
    fn strip_known_launch_option_fragments_walks_every_registered_adapter() {
        // Given argv that contains a fragment from every adapter
        // with launch options, `strip_known_launch_option_fragments`
        // removes all of them.
        let argv = vec![
            "claude".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "extra".to_string(),
        ];
        let stripped = strip_known_launch_option_fragments(argv);
        assert_eq!(stripped, vec!["claude".to_string(), "extra".to_string()]);
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
        fn harness_key(&self) -> &'static str {
            self.key
        }

        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(self.fragment.clone())
        }
    }

    struct StateAwareAdapter;

    impl HarnessAdapter for StateAwareAdapter {
        fn harness_key(&self) -> &'static str {
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
            fn harness_key(&self) -> &'static str {
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
            fn harness_key(&self) -> &'static str {
                "noop"
            }
            fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
                Ok(GraphFragment::empty())
            }
        }
        assert!(NoOpAdapter.resume_argv("abc", Path::new("/p")).is_none());
    }
}
