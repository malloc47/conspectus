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
            nodes: vec![GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("state-aware", "scope", "s1"),
                "state-aware".to_string(),
            ))],
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

    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));

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
    let session_alpha = GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("codex", "scope", "alpha"),
        "codex".to_string(),
    ));
    let session_beta = GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("codex", "scope", "beta"),
        "codex".to_string(),
    ));

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

// ----- Per-adapter resume_argv -----

#[test]
fn codex_resume_argv_matches_tui_resume_shape() {
    let argv = resume_argv_for("codex", "abc123", Path::new("/p")).expect("codex supports resume");
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
    let argv =
        resume_argv_for("opencode", "abc123", Path::new("/p")).expect("opencode supports resume");
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

/// When a state root is not present in the discovery
/// context, `discover_with_state_root` short-circuits with an
/// empty fragment instead of invoking `inner`.
#[test]
fn discover_with_state_root_returns_empty_when_state_root_missing() {
    // A discovery context with no harness state roots at all.
    let context = DiscoveryContext::from_root("/tmp");
    let mut invoked = false;
    let fragment = discover_with_state_root(&context, "codex", |_| {
        invoked = true;
        Ok(GraphFragment::empty())
    })
    .expect("no error");
    assert!(fragment.nodes.is_empty());
    assert!(fragment.candidate_links.is_empty());
    assert!(!invoked, "inner must not run when state root is absent");
}

/// When a state root is present, the inner walker
/// receives the resolved path and the resulting fragment is
/// stamped with the harness key.
#[test]
fn discover_with_state_root_stamps_provider_when_state_root_present() {
    use tempfile::TempDir;
    let temp = TempDir::new().expect("temp dir");
    let context =
        DiscoveryContext::from_root(temp.path()).with_harness_state_root("codex", temp.path());
    let invoked = std::cell::Cell::new(false);
    let fragment = discover_with_state_root(&context, "codex", |root| {
        assert_eq!(root, temp.path());
        invoked.set(true);
        Ok(GraphFragment::empty())
    })
    .expect("no error");
    assert!(invoked.get(), "inner must run when state root is present");
    // Fragment is empty but stamp_fragment ran (nothing observable
    // on an empty fragment; behavior verified by the callers'
    // snapshot tests).
    assert!(fragment.nodes.is_empty());
}

#[test]
fn adapters_for_program_matches_binary_basenames() {
    let keys = |program: &str| -> Vec<&str> {
        adapters_for_program(program)
            .map(HarnessAdapter::harness_key)
            .collect()
    };
    assert_eq!(keys("codex"), ["codex"]);
    assert_eq!(keys("/nix/store/x-codex/bin/codex-rs --yolo"), ["codex"]);
    assert_eq!(keys("Claude"), ["claude-code"]);
    assert!(keys("npm").is_empty());
    assert!(keys("").is_empty());
}

#[test]
fn adapter_in_command_finds_the_harness_behind_a_launch_wrapper() {
    let key = |command: &str| {
        adapter_in_command(command.split_whitespace()).map(HarnessAdapter::harness_key)
    };
    assert_eq!(
        key("atelier exec claude --dangerously-skip-permissions"),
        Some("claude-code")
    );
    assert_eq!(
        key("atelier exec -- mise exec -- nix develop -c -- codex --yolo"),
        Some("codex")
    );
    assert_eq!(key("codex"), Some("codex"));
    assert_eq!(key("atelier exec -- npm run dev"), None);
    assert_eq!(key(""), None);
}
