// Extracted from cli.rs H-HYG-011 rolling wave.
use super::*;
use std::time::Duration;
// H-REF-006 wave 4: hook helpers moved to cli/hook.rs
use super::hook::{
    ensure_claude_hook, harness_binaries, has_claude_hook, remove_claude_hook,
    resolve_harness_pid_with,
};
// H-REF-006 wave 10: pin helpers moved to cli/pin.rs
use super::pin::{format_epoch_iso8601, resolve_resume_argv_with_cache};
// H-REF-006 wave 12: TUI helpers moved to cli/tui.rs
use super::tui::parse_tui_duration;

fn program(cmd: &ProcCommand) -> String {
    cmd.get_program().to_string_lossy().into_owned()
}

fn args(cmd: &ProcCommand) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn resolve_harness_pid_walks_past_wrapper_layers_to_claude() {
    // Simulated process tree the hook writer would actually see:
    //   42 = sh -c '<...> conspectus hook write claude-code'
    //   41 = claude (the long-lived harness)
    //   40 = bash (login shell, not a harness)
    // Start walk from pid 42 (parent of the conspectus child).
    let tree: std::collections::HashMap<u32, (&str, u32)> =
        [(42, ("sh", 41)), (41, ("claude", 40)), (40, ("bash", 1))]
            .into_iter()
            .collect();
    let pair = resolve_harness_pid_with(42, &["claude", "claude-code"], |pid| {
        tree.get(&pid).map(|(comm, ppid)| (comm.to_string(), *ppid))
    });
    assert_eq!(pair, Some((41, 40)));
}

#[test]
fn resolve_harness_pid_returns_none_when_no_ancestor_matches() {
    let tree: std::collections::HashMap<u32, (&str, u32)> =
        [(42, ("sh", 41)), (41, ("emacs", 40)), (40, ("bash", 1))]
            .into_iter()
            .collect();
    let pair = resolve_harness_pid_with(42, &["claude", "claude-code"], |pid| {
        tree.get(&pid).map(|(comm, ppid)| (comm.to_string(), *ppid))
    });
    assert_eq!(pair, None);
}

#[test]
fn resolve_harness_pid_stops_at_init() {
    // pid 1 should terminate the walk without consulting the
    // reader so we never falsely match a process named after a
    // harness running as init/PID 1 in a container.
    let pair = resolve_harness_pid_with(1, &["claude"], |_| {
        panic!("walker must stop at pid 1 without reading")
    });
    assert_eq!(pair, None);
}

#[test]
fn resolve_harness_pid_terminates_on_missing_proc_entry() {
    // A pid that has exited mid-walk should fail closed (return
    // None) instead of panicking or looping. Mirrors what
    // /proc-based reads do for a vanished process.
    let pair = resolve_harness_pid_with(99, &["claude"], |_| None);
    assert_eq!(pair, None);
}

#[test]
fn harness_binaries_recognizes_supported_harnesses() {
    assert_eq!(harness_binaries("claude-code"), &["claude", "claude-code"]);
    assert_eq!(harness_binaries("codex"), &["codex", "codex-rs"]);
    assert_eq!(harness_binaries("opencode"), &["opencode"]);
    assert_eq!(harness_binaries("unknown"), &[] as &[&str]);
}

#[test]
fn parse_tui_duration_accepts_each_supported_unit() {
    assert_eq!(parse_tui_duration("500ms"), Ok(Duration::from_millis(500)));
    assert_eq!(parse_tui_duration("30s"), Ok(Duration::from_secs(30)));
    assert_eq!(parse_tui_duration("2m"), Ok(Duration::from_secs(120)));
    assert_eq!(parse_tui_duration("1h"), Ok(Duration::from_secs(3600)));
}

#[test]
fn parse_tui_duration_trims_surrounding_whitespace() {
    assert_eq!(parse_tui_duration("  10s  "), Ok(Duration::from_secs(10)));
}

#[test]
fn parse_tui_duration_rejects_missing_unit() {
    let err = parse_tui_duration("30").unwrap_err();
    assert!(err.contains("missing unit"), "got: {err}");
}

#[test]
fn parse_tui_duration_rejects_unknown_unit() {
    let err = parse_tui_duration("30d").unwrap_err();
    assert!(err.contains("unknown unit"), "got: {err}");
}

#[test]
fn parse_tui_duration_rejects_empty_string() {
    assert!(parse_tui_duration("").is_err());
    assert!(parse_tui_duration("   ").is_err());
}

#[test]
fn parse_tui_duration_rejects_negative_or_non_integer() {
    assert!(parse_tui_duration("-5s").is_err());
    assert!(parse_tui_duration("1.5s").is_err());
}

#[test]
fn no_subcommand_defaults_to_tui() {
    let cli = Cli::parse_from(["conspectus"]);
    assert!(cli.command.is_none());
    let command = cli.command.unwrap_or_else(default_command);
    let Command::Tui(args) = command else {
        panic!("expected default command to be tui");
    };
    assert_eq!(
        parse_tui_duration(&args.refresh_interval),
        Ok(Duration::from_secs(30))
    );
    assert_eq!(
        parse_tui_duration(&args.mux_preview_interval),
        Ok(Duration::from_secs(2))
    );
}

#[test]
fn pager_candidates_when_pager_env_is_unset_starts_with_less_plus_defaults() {
    let candidates = pager_candidates_with_env(None);
    assert!(candidates.len() >= 2);
    assert_eq!(program(&candidates[0]), "less");
    assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
    assert_eq!(program(&candidates[1]), "more");
}

#[test]
fn pager_candidates_when_pager_is_bare_less_adds_default_flags() {
    // Regression for the original bug: the user had `PAGER=less`
    // (no args) plus `LESS=-R` set, so the original implementation
    // left less without `-F` and dropped into the pager even for
    // short, single-screen tables. The fix passes `-F -R -X` on
    // the command line whenever the resolved pager is plain
    // `less`, so the flags merge with the user's `$LESS` instead
    // of being silently skipped.
    let candidates = pager_candidates_with_env(Some("less".to_string()));
    assert_eq!(program(&candidates[0]), "less");
    assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
}

#[test]
fn pager_candidates_when_pager_is_less_with_explicit_args_respects_user_choice() {
    let candidates = pager_candidates_with_env(Some("less -X".to_string()));
    assert_eq!(program(&candidates[0]), "less");
    // Explicit args are kept verbatim; we do not silently append
    // `-F` because the user opted in to their own less flag set.
    assert_eq!(args(&candidates[0]), vec!["-X"]);
}

#[test]
fn pager_candidates_passes_non_less_pager_through_unchanged() {
    let candidates = pager_candidates_with_env(Some("bat --paging=always".to_string()));
    assert_eq!(program(&candidates[0]), "bat");
    assert_eq!(args(&candidates[0]), vec!["--paging=always"]);
}

#[test]
fn resolve_color_never_wins_against_every_env_signal() {
    assert!(!resolve_color(
        ColorFlag::Never,
        Some("1".into()),
        Some("1".into()),
        Some("1".into()),
        Some("xterm".into()),
        true,
    ));
}

#[test]
fn resolve_color_always_overrides_no_color_and_dumb_term() {
    assert!(resolve_color(
        ColorFlag::Always,
        Some("1".into()),
        None,
        None,
        Some("dumb".into()),
        false,
    ));
}

#[test]
fn resolve_color_auto_honors_no_color() {
    assert!(!resolve_color(
        ColorFlag::Auto,
        Some("1".into()),
        None,
        None,
        None,
        true,
    ));
    // Empty NO_COLOR is treated as unset (per the spec — value
    // matters, not just presence).
    assert!(resolve_color(
        ColorFlag::Auto,
        Some(String::new()),
        None,
        None,
        None,
        true,
    ));
}

#[test]
fn resolve_color_auto_honors_clicolor_force_even_on_non_tty() {
    assert!(resolve_color(
        ColorFlag::Auto,
        None,
        Some("1".into()),
        None,
        None,
        false,
    ));
    // CLICOLOR_FORCE=0 is *not* "force on".
    assert!(!resolve_color(
        ColorFlag::Auto,
        None,
        Some("0".into()),
        None,
        None,
        false,
    ));
}

#[test]
fn resolve_color_auto_dumb_term_opts_out() {
    assert!(!resolve_color(
        ColorFlag::Auto,
        None,
        None,
        None,
        Some("dumb".into()),
        true,
    ));
}

#[test]
fn resolve_color_auto_clicolor_zero_opts_out() {
    assert!(!resolve_color(
        ColorFlag::Auto,
        None,
        None,
        Some("0".into()),
        None,
        true,
    ));
}

#[test]
fn resolve_color_auto_falls_back_to_isatty() {
    assert!(resolve_color(ColorFlag::Auto, None, None, None, None, true));
    assert!(!resolve_color(
        ColorFlag::Auto,
        None,
        None,
        None,
        None,
        false
    ));
}

#[test]
fn resolve_color_from_env_uses_supplied_tty_signal() {
    // The wrapper pulls env vars from the real process; we can
    // only safely pin behavior under the flag values that
    // short-circuit before any env lookup.
    assert!(!resolve_color_from_env(ColorFlag::Never, true));
    assert!(resolve_color_from_env(ColorFlag::Always, false));
}

#[test]
fn pager_candidates_empty_pager_env_falls_back_to_internal_defaults() {
    let candidates = pager_candidates_with_env(Some("   ".to_string()));
    // Whitespace-only `$PAGER` falls back to the internal `less`
    // with default flags.
    assert_eq!(program(&candidates[0]), "less");
    assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
}

#[test]
fn ensure_claude_hook_preserves_existing_hooks() {
    let mut document = serde_json::json!({
        "theme": "dark",
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "startup",
                    "hooks": [
                        { "type": "command", "command": "echo existing" }
                    ]
                }
            ]
        }
    });

    assert!(ensure_claude_hook(
        &mut document,
        "conspectus hook write claude-code"
    ));
    assert!(has_claude_hook(&document));

    let entries = document["hooks"]["SessionStart"].as_array().expect("array");
    assert_eq!(entries.len(), 2);
    assert_eq!(document["theme"], "dark");
}

#[test]
fn ensure_claude_hook_is_idempotent() {
    let mut document = serde_json::json!({});

    assert!(ensure_claude_hook(
        &mut document,
        "conspectus hook write claude-code"
    ));
    assert!(!ensure_claude_hook(
        &mut document,
        "conspectus hook write claude-code"
    ));

    let entries = document["hooks"]["SessionStart"].as_array().expect("array");
    assert_eq!(entries.len(), 1);
}

#[test]
fn remove_claude_hook_preserves_unrelated_hooks_in_same_entry() {
    let mut document = serde_json::json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "startup",
                    "hooks": [
                        { "type": "command", "command": "conspectus hook write claude-code" },
                        { "type": "command", "command": "echo existing" }
                    ]
                }
            ]
        }
    });

    assert!(remove_claude_hook(&mut document));
    assert!(!has_claude_hook(&document));

    let hooks = document["hooks"]["SessionStart"][0]["hooks"]
        .as_array()
        .expect("hooks");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["command"], "echo existing");
}

// ---- ADR 0031 / F8-009: FilterArgs ----

fn filter_args_with(
    harness: Vec<&str>,
    max_age: Option<&str>,
    mux_state: Vec<&str>,
    grouping: Option<&str>,
) -> FilterArgs {
    FilterArgs {
        harness: harness.into_iter().map(String::from).collect(),
        max_age: max_age.map(String::from),
        mux_state: mux_state.into_iter().map(String::from).collect(),
        grouping: grouping.map(String::from),
    }
}

#[test]
fn filter_args_empty_produces_empty_row_filter() {
    let args = FilterArgs::default();
    let filter = args.to_row_filter().expect("parse");
    assert!(filter.is_empty());
}

#[test]
fn filter_args_harness_repeats_into_set() {
    let args = filter_args_with(vec!["claude-code", "codex"], None, vec![], None);
    let filter = args.to_row_filter().expect("parse");
    assert_eq!(
        filter
            .harness
            .as_ref()
            .map(|h| h.values().to_vec())
            .unwrap_or_default(),
        vec!["claude-code".to_string(), "codex".to_string()]
    );
}

#[test]
fn filter_args_max_age_parses_duration_suffixes() {
    let args = filter_args_with(vec![], Some("7d"), vec![], None);
    let filter = args.to_row_filter().expect("parse");
    assert_eq!(
        filter.max_age,
        Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
    );
}

#[test]
fn filter_args_max_age_reports_actionable_error() {
    let args = filter_args_with(vec![], Some("nope"), vec![], None);
    let err = args.to_row_filter().unwrap_err().to_string();
    assert!(err.contains("invalid --max-age"));
}

#[test]
fn filter_args_mux_state_parses_each_value() {
    let args = filter_args_with(vec![], None, vec!["unmuxed", "Ambiguous"], None);
    let filter = args.to_row_filter().expect("parse");
    let states = filter
        .mux_state
        .as_ref()
        .map(|m| m.values().to_vec())
        .unwrap_or_default();
    use conspectus::filter::MuxStateKey;
    assert!(states.contains(&MuxStateKey::Unmuxed));
    assert!(states.contains(&MuxStateKey::Ambiguous));
}

#[test]
fn filter_args_mux_state_invalid_value_errors_with_choices() {
    let args = filter_args_with(vec![], None, vec!["frobnicated"], None);
    let err = args.to_row_filter().unwrap_err().to_string();
    assert!(err.contains("invalid --mux-state"));
    assert!(err.contains("attached, ambiguous, unmuxed"));
}

#[test]
fn filter_args_grouping_parses_per_view() {
    use conspectus::tui::{Grouping, SessionsGrouping, View};
    let args = filter_args_with(vec![], None, vec![], Some("repo"));
    assert_eq!(
        args.to_grouping(View::Sessions).expect("parse"),
        Some(Grouping::Sessions(SessionsGrouping::Repo))
    );

    let args = filter_args_with(vec![], None, vec![], Some("none"));
    assert_eq!(
        args.to_grouping(View::Sessions).expect("parse"),
        Some(Grouping::Sessions(SessionsGrouping::None))
    );
}

#[test]
fn filter_args_grouping_rejects_value_for_wrong_view() {
    use conspectus::tui::View;
    // `host` is a mux grouping, not a sessions one.
    let args = filter_args_with(vec![], None, vec![], Some("host"));
    let err = args.to_grouping(View::Sessions).unwrap_err().to_string();
    assert!(err.contains("invalid --grouping `host` for --view sessions"));
    assert!(err.contains("graph, workspace, repo, checkout, scan-root"));
}

#[test]
fn filter_args_grouping_none_when_flag_omitted() {
    use conspectus::tui::View;
    let args = FilterArgs::default();
    assert!(args.to_grouping(View::Sessions).expect("parse").is_none());
}

// ---- H-PIN-012 pin launch helpers --------------------------

#[test]
fn format_attach_command_uses_bare_tmux_for_default_socket() {
    assert_eq!(
        super::pin::format_attach_command(None, "ingest"),
        "tmux attach-session -t ingest"
    );
}

#[test]
fn format_attach_command_threads_socket_via_dash_l() {
    assert_eq!(
        super::pin::format_attach_command(Some("scratch"), "ingest"),
        "tmux -L scratch attach-session -t ingest"
    );
}

#[test]
fn format_argv_for_send_keys_quotes_whitespace_tokens() {
    let argv = vec![
        std::ffi::OsString::from("codex"),
        std::ffi::OsString::from("--prompt"),
        std::ffi::OsString::from("hello world"),
    ];
    assert_eq!(
        super::pin::format_argv_for_send_keys(&argv),
        "codex --prompt \"hello world\""
    );
}

#[test]
fn format_argv_for_send_keys_leaves_bare_tokens_unquoted() {
    let argv = vec![
        std::ffi::OsString::from("codex"),
        std::ffi::OsString::from("--model=opus"),
    ];
    assert_eq!(
        super::pin::format_argv_for_send_keys(&argv),
        "codex --model=opus"
    );
}

// ----- H-PIN-RESUME-005: ISO 8601 formatting for pin show -----

#[test]
fn format_epoch_iso8601_renders_known_unix_dates() {
    // 2024-01-01T00:00:00Z = 1704067200
    assert_eq!(format_epoch_iso8601(1_704_067_200), "2024-01-01T00:00:00Z");
    // 2026-06-05T12:34:56Z (mid-day timestamp)
    // Computed: days_from_1970 = 20609, secs = 20609*86400 + 12*3600+34*60+56
    // = 1780_624_096
    assert_eq!(format_epoch_iso8601(1_780_662_896), "2026-06-05T12:34:56Z");
    // Epoch zero.
    assert_eq!(format_epoch_iso8601(0), "1970-01-01T00:00:00Z");
}

#[test]
fn format_epoch_iso8601_clamps_negative_epochs_to_zero() {
    // Defensive — sidecars shouldn't carry negative epochs, but
    // we shouldn't panic if they do.
    assert_eq!(format_epoch_iso8601(-1), "1970-01-01T00:00:00Z");
}

// ----- H-PIN-RESUME-004: launch-time resume resolver -----

mod resume_resolver {
    use conspectus::model::{
        AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode,
        GraphSnapshot, LinkEndpoint, LinkState, NodeId, PinCandidate, PinMuxRef, Provenance,
        RelationKind, SourceMetadata,
    };
    use conspectus::pin_bindings::{
        PinBindingRecord, PinBindingsCache, read as read_sidecar, write as write_sidecar,
    };
    use std::path::Path;
    use tempfile::tempdir;

    fn make_pin(harness: &str) -> PinCandidate {
        PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: harness.to_string(),
            cwd: "/p".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".to_string(),
            binding: None,
        }
    }

    fn make_session(harness: &str, key: &str) -> AgentSessionNode {
        AgentSessionNode::new(
            AgentSessionId::new(harness, "/state", key),
            harness.to_string(),
        )
    }

    fn parent_link(snap: &mut GraphSnapshot, harness: &str, child: &str, parent: &str) {
        snap.candidate_links.push(GraphLink {
            id: format!("{child}-parent-{parent}"),
            source: NodeId::AgentSession(AgentSessionId::new(harness, "/state", child)),
            target: LinkEndpoint::Node {
                id: NodeId::AgentSession(AgentSessionId::new(harness, "/state", parent)),
            },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }

    fn seed_sidecar(cache: &PinBindingsCache, session_key: &str, harness: &str) {
        let record = PinBindingRecord::new(
            "ingest",
            "ingest",
            None,
            session_key,
            harness,
            1_738_742_400,
        );
        write_sidecar(cache, &record).expect("seed sidecar");
    }

    #[test]
    fn no_sidecar_returns_none() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        let snap = GraphSnapshot::empty();
        let pin = make_pin("codex");
        assert!(
            super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache).is_none()
        );
    }

    #[test]
    fn sidecar_with_recorded_session_in_snapshot_splices_resume_argv() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "session-a", "codex");

        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::AgentSession(make_session("codex", "session-a")));
        let pin = make_pin("codex");

        let argv = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache)
            .expect("resume argv produced");
        // codex resume_argv per H-PIN-RESUME-002: ["codex",
        // "exec", "--resume", "session-a"].
        assert_eq!(
            argv,
            vec![
                std::ffi::OsString::from("codex"),
                std::ffi::OsString::from("exec"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("session-a"),
            ]
        );
    }

    #[test]
    fn lineage_walks_forward_to_head_before_resume() {
        // a -> b -> c. Sidecar recorded `a`; we expect resume to
        // target `c`.
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "a", "codex");

        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b", "c"] {
            snap.nodes
                .push(GraphNode::AgentSession(make_session("codex", key)));
        }
        parent_link(&mut snap, "codex", "b", "a");
        parent_link(&mut snap, "codex", "c", "b");

        let pin = make_pin("codex");
        let argv = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache)
            .expect("resume argv");
        assert!(argv.contains(&std::ffi::OsString::from("c")));
        assert!(!argv.contains(&std::ffi::OsString::from("a")));
    }

    #[test]
    fn fork_in_lineage_returns_none_and_keeps_sidecar() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "a", "codex");

        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b", "c"] {
            snap.nodes
                .push(GraphNode::AgentSession(make_session("codex", key)));
        }
        parent_link(&mut snap, "codex", "b", "a");
        parent_link(&mut snap, "codex", "c", "a");

        let pin = make_pin("codex");
        let outcome = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache);
        assert!(outcome.is_none(), "fork should fall back to default argv");
        // Sidecar is NOT deleted on fork — the data isn't stale,
        // just ambiguous.
        assert!(read_sidecar(&cache, "ingest").unwrap().is_some());
    }

    #[test]
    fn missing_session_deletes_sidecar() {
        // ADR 0058 Q7: recorded session not on disk → delete the
        // sidecar before falling back.
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "gone", "codex");
        let snap = GraphSnapshot::empty();
        let pin = make_pin("codex");

        let outcome = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache);
        assert!(outcome.is_none());
        assert!(
            read_sidecar(&cache, "ingest").unwrap().is_none(),
            "stale sidecar should be deleted per ADR 0058 Q7",
        );
    }

    #[test]
    fn harness_without_resume_returns_none() {
        // aider has no resume CLI.
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "session-a", "aider");

        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::AgentSession(make_session("aider", "session-a")));
        let pin = make_pin("aider");

        let outcome = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache);
        assert!(outcome.is_none());
        // Sidecar is not deleted — the session exists, just
        // can't be resumed via CLI.
        assert!(read_sidecar(&cache, "ingest").unwrap().is_some());
    }

    #[test]
    fn claude_code_returns_claude_resume_argv() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        seed_sidecar(&cache, "session-c", "claude-code");

        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::AgentSession(make_session(
            "claude-code",
            "session-c",
        )));
        let pin = make_pin("claude-code");

        let argv = super::resolve_resume_argv_with_cache(&snap, &pin, Path::new("/p"), &cache)
            .expect("claude resume argv");
        assert_eq!(
            argv,
            vec![
                std::ffi::OsString::from("claude"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("session-c"),
            ]
        );
    }
}
