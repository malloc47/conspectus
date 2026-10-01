use super::*;

#[test]
fn missing_binary_reports_unavailable_binary_not_found() {
    let runner = SystemTmux::with_binary("/definitely/not/here/tmux");

    let outcome = runner.list_sessions("#{session_name}").expect("non-fatal");

    assert_eq!(
        outcome,
        TmuxOutcome::Unavailable(UnavailableReason::BinaryNotFound)
    );
}

#[test]
fn fake_tmux_returns_pre_canned_sessions() {
    let runner = FakeTmux::with_sessions("alpha:/work\nbeta:/work\n");

    let outcome = runner.list_sessions("#{session_name}").expect("ok");

    assert_eq!(
        outcome,
        TmuxOutcome::Sessions("alpha:/work\nbeta:/work\n".to_string())
    );
}

#[test]
fn fake_tmux_can_report_no_server() {
    let runner = FakeTmux::unavailable(UnavailableReason::NoServer);

    let outcome = runner.list_sessions("#{session_name}").expect("ok");

    assert_eq!(
        outcome,
        TmuxOutcome::Unavailable(UnavailableReason::NoServer)
    );
}

#[test]
fn fake_tmux_can_surface_failed_runs() {
    let runner = FakeTmux::failed(Some(2), "permission denied");

    let outcome = runner.list_sessions("#{session_name}").expect("ok");

    assert_eq!(
        outcome,
        TmuxOutcome::Failed {
            code: Some(2),
            message: "permission denied".to_string(),
        }
    );
}

#[test]
fn no_server_stderr_maps_to_unavailable() {
    assert!(looks_like_no_server(
        "no server running on /tmp/tmux-1000/default"
    ));
    assert!(looks_like_no_server(
        "error connecting to /tmp/tmux-1000/default (No sessions)"
    ));
    assert!(!looks_like_no_server("permission denied"));
}

#[test]
fn unavailable_reason_has_stable_diagnostic_strings() {
    assert_eq!(
        UnavailableReason::BinaryNotFound.as_str(),
        "tmux binary not found"
    );
    assert_eq!(
        UnavailableReason::NoServer.as_str(),
        "tmux server not running"
    );
}

#[test]
fn parser_yields_empty_rows_for_empty_output() {
    assert!(parse_list_sessions("").is_empty());
    assert!(parse_list_sessions("\n\n   \n").is_empty());
}

#[test]
fn parser_extracts_name_path_activity_and_created_epoch() {
    let rows = parse_list_sessions("alpha\t/work/alpha\t1700000500\t1700000000\n");

    assert_eq!(
        rows,
        vec![TmuxSessionRow {
            name: "alpha".to_string(),
            path: Some("/work/alpha".to_string()),
            activity_epoch: Some(1700000500),
            created_epoch: Some(1700000000),
            last_attached_epoch: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
        }]
    );
}

#[test]
fn parser_extracts_last_attached_epoch() {
    // `session_last_attached` is the 11th (trailing)
    // tab field. All ten preceding fields present, then the epoch.
    let rows =
        parse_list_sessions("alpha\t/work\t1700000500\t1700000000\t\t\t\t\t1\talpha\t1700000400\n");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].last_attached_epoch, Some(1700000400));
}

#[test]
fn parser_defaults_last_attached_epoch_when_field_absent() {
    // Older/short output without the trailing field parses cleanly
    // with `last_attached_epoch: None`.
    let rows = parse_list_sessions("alpha\t/work\t1700000500\t1700000000\n");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].last_attached_epoch, None);
}

#[test]
fn parser_extracts_active_pane_process_fields() {
    let rows = parse_list_sessions(
        "alpha\t/work\t1700000500\t1700000000\tclaude\t123\t/work\tclaude --resume abc\n",
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].active_pane_command.as_deref(), Some("claude"));
    assert_eq!(rows[0].active_pane_pid, Some(123));
    assert_eq!(rows[0].active_pane_current_path.as_deref(), Some("/work"));
    assert_eq!(
        rows[0].active_pane_start_command.as_deref(),
        Some("claude --resume abc")
    );
}

#[test]
fn parser_extracts_session_attached_flag() {
    let rows = parse_list_sessions("alpha\t/work\t1700000500\t1700000000\t\t\t\t\t1\n");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].client_attached, Some(true));

    let rows = parse_list_sessions("beta\t/work\t1700000500\t1700000000\t\t\t\t\t0\n");
    assert_eq!(rows[0].client_attached, Some(false));
}

#[test]
fn parser_treats_attached_list_as_interactive_tty_signal() {
    let rows = parse_list_sessions(
        "alpha\t/work\t1700000500\t1700000000\t\t\t\t\t2\tclient-123,/dev/pts/1\n",
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].client_attached, Some(true));

    let rows = parse_list_sessions("beta\t/work\t1700000500\t1700000000\t\t\t\t\t1\tclient-123\n");
    assert_eq!(rows[0].client_attached, Some(false));
}

#[test]
fn parser_handles_paths_with_spaces() {
    let rows = parse_list_sessions("with-space\t/work/has spaces/here\t\t\n");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path.as_deref(), Some("/work/has spaces/here"));
    assert!(rows[0].activity_epoch.is_none());
    assert!(rows[0].created_epoch.is_none());
}

#[test]
fn parser_handles_missing_optional_fields() {
    let rows = parse_list_sessions("only-name\n");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "only-name");
    assert!(rows[0].path.is_none());
}

#[test]
fn parser_skips_rows_without_a_name() {
    let rows = parse_list_sessions("\t/work\t\t\n");

    assert!(rows.is_empty());
}

#[test]
fn parser_drops_malformed_epoch_fields() {
    let rows = parse_list_sessions("alpha\t/work\tNaN\tunknown\n");

    assert_eq!(rows.len(), 1);
    assert!(rows[0].activity_epoch.is_none());
    assert!(rows[0].created_epoch.is_none());
}

#[test]
fn discovery_returns_zero_sessions_for_blank_runner_output() {
    let discovery = TmuxDiscovery::with_runner(FakeTmux::with_sessions(""));

    let fragment = discovery
        .discover(&DiscoveryContext::default())
        .expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn discovery_emits_mux_session_per_row() {
    let stdout = "alpha\t/work/alpha\t1\t0\nbeta\t/work/has space\t\t\n";
    let discovery = TmuxDiscovery::with_runner(FakeTmux::with_sessions(stdout));

    let fragment = discovery
        .discover(&DiscoveryContext::default())
        .expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(session) => Some(session.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 2);
    let alpha = sessions
        .iter()
        .find(|s| s.native_id == "alpha")
        .expect("alpha");
    assert_eq!(alpha.id.native_id, "tmux:alpha");
    assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
    let beta = sessions
        .iter()
        .find(|s| s.native_id == "beta")
        .expect("beta");
    assert_eq!(beta.cwd.as_deref(), Some("/work/has space"));
}

#[test]
fn discovery_yields_empty_fragment_when_tmux_unavailable() {
    let discovery = TmuxDiscovery::with_runner(FakeTmux::unavailable(UnavailableReason::NoServer));

    let fragment = discovery
        .discover(&DiscoveryContext::default())
        .expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn discovery_rows_preserve_status_for_unavailable_tmux() {
    let discovery =
        TmuxDiscovery::with_runner(FakeTmux::unavailable(UnavailableReason::BinaryNotFound));

    let rows = discovery.rows().expect("rows");

    assert_eq!(
        rows.status,
        TmuxStatus::Unavailable(UnavailableReason::BinaryNotFound)
    );
    assert!(rows.rows.is_empty());
}

#[test]
fn discovery_rows_surface_failed_status() {
    let discovery = TmuxDiscovery::with_runner(FakeTmux::failed(Some(2), "permission denied"));

    let rows = discovery.rows().expect("rows");

    assert_eq!(
        rows.status,
        TmuxStatus::Failed {
            code: Some(2),
            message: "permission denied".to_string(),
        }
    );
}

#[test]
fn missing_binary_capture_pane_reports_unavailable_binary_not_found() {
    let runner = SystemTmux::with_binary("/definitely/not/here/tmux");
    let outcome = runner.capture_pane(None, "editor").expect("non-fatal");
    assert_eq!(
        outcome,
        TmuxCaptureOutcome::Unavailable(UnavailableReason::BinaryNotFound)
    );
}

#[test]
fn fake_runner_default_capture_pane_returns_unsupported() {
    let runner = FakeTmux::with_sessions("");
    assert_eq!(
        runner.capture_pane(None, "anything").unwrap(),
        TmuxCaptureOutcome::Unsupported
    );
}

#[test]
fn missing_binary_rename_session_reports_unavailable_binary_not_found() {
    let runner = SystemTmux::with_binary("/definitely/not/here/tmux");
    let outcome = runner
        .rename_session(None, "alpha", "new")
        .expect("non-fatal");
    assert_eq!(
        outcome,
        TmuxRenameOutcome::Unavailable(UnavailableReason::BinaryNotFound)
    );
}

#[test]
fn fake_runner_default_rename_session_records_call_and_returns_renamed() {
    let runner = FakeTmux::with_sessions("");
    let outcome = runner.rename_session(None, "alpha", "beta").expect("ok");
    assert_eq!(outcome, TmuxRenameOutcome::Renamed);
    assert_eq!(
        runner.rename_calls(),
        vec![(None, "alpha".to_string(), "beta".to_string())]
    );
}

#[test]
fn fake_runner_returns_registered_rename_outcome_by_target() {
    let runner = FakeTmux::with_sessions("")
        .with_rename("alpha", TmuxRenameOutcome::NoTarget)
        .with_rename("beta", TmuxRenameOutcome::NameCollision)
        .with_rename(
            "broken",
            TmuxRenameOutcome::Failed {
                code: Some(1),
                message: "boom".to_string(),
            },
        );
    assert_eq!(
        runner.rename_session(None, "alpha", "alias").unwrap(),
        TmuxRenameOutcome::NoTarget
    );
    assert_eq!(
        runner.rename_session(None, "beta", "alias").unwrap(),
        TmuxRenameOutcome::NameCollision
    );
    assert_eq!(
        runner.rename_session(None, "broken", "alias").unwrap(),
        TmuxRenameOutcome::Failed {
            code: Some(1),
            message: "boom".to_string(),
        }
    );
    assert_eq!(
        runner.rename_session(None, "other", "alias").unwrap(),
        TmuxRenameOutcome::Renamed
    );
    assert_eq!(
        runner.rename_calls(),
        vec![
            (None, "alpha".to_string(), "alias".to_string()),
            (None, "beta".to_string(), "alias".to_string()),
            (None, "broken".to_string(), "alias".to_string()),
            (None, "other".to_string(), "alias".to_string()),
        ]
    );
}

#[test]
fn default_rename_session_impl_returns_unsupported() {
    struct ReadOnlyRunner;
    impl MuxBackend for ReadOnlyRunner {
        fn backend_key(&self) -> &'static str {
            "test-read-only"
        }

        fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
            Ok(TmuxOutcome::Sessions(String::new()))
        }
    }
    let runner = ReadOnlyRunner;
    assert_eq!(
        runner.rename_session(None, "alpha", "beta").unwrap(),
        TmuxRenameOutcome::Unsupported
    );
}

#[test]
fn name_collision_stderr_maps_to_name_collision() {
    assert!(looks_like_name_collision("duplicate session: alpha"));
    assert!(looks_like_name_collision("session already exists"));
    assert!(!looks_like_name_collision("can't find session alpha"));
}

#[test]
fn fake_runner_returns_registered_capture_outcomes_by_target() {
    let runner = FakeTmux::with_sessions("")
        .with_capture(
            "editor",
            TmuxCaptureOutcome::Captured("pane content".to_string()),
        )
        .with_capture("missing", TmuxCaptureOutcome::NoTarget)
        .with_capture(
            "broken",
            TmuxCaptureOutcome::Failed {
                code: Some(1),
                message: "boom".to_string(),
            },
        );
    assert_eq!(
        runner.capture_pane(None, "editor").unwrap(),
        TmuxCaptureOutcome::Captured("pane content".to_string())
    );
    assert_eq!(
        runner.capture_pane(None, "missing").unwrap(),
        TmuxCaptureOutcome::NoTarget
    );
    assert_eq!(
        runner.capture_pane(None, "broken").unwrap(),
        TmuxCaptureOutcome::Failed {
            code: Some(1),
            message: "boom".to_string(),
        }
    );
    // Unregistered targets keep the default Unsupported.
    assert_eq!(
        runner.capture_pane(None, "other").unwrap(),
        TmuxCaptureOutcome::Unsupported
    );
}

// ---- H-PIN-010 socket threading + new mutation methods ----

#[test]
fn fake_runner_default_new_session_records_call_and_returns_created() {
    let runner = FakeTmux::with_sessions("");
    let outcome = runner
        .new_session(
            None,
            "ingest",
            Path::new("/tmp/repo"),
            &[OsString::from("codex")],
        )
        .expect("ok");
    assert_eq!(outcome, TmuxNewSessionOutcome::Created);
    assert_eq!(
        runner.new_session_calls(),
        vec![(
            None,
            "ingest".to_string(),
            PathBuf::from("/tmp/repo"),
            vec![OsString::from("codex")],
        )]
    );
}

#[test]
fn fake_runner_threads_non_default_socket_into_recorded_calls() {
    let runner = FakeTmux::with_sessions("");
    runner
        .new_session(
            Some("scratch"),
            "ingest",
            Path::new("/tmp/repo"),
            &[OsString::from("codex")],
        )
        .expect("ok");
    runner
        .attach_session(Some("scratch"), "ingest")
        .expect("ok");
    runner
        .send_keys(Some("scratch"), "ingest", "codex --resume", true)
        .expect("ok");
    runner
        .rename_session(Some("scratch"), "ingest", "ingest-refactor")
        .expect("ok");

    assert_eq!(
        runner.new_session_calls().first().unwrap().0.as_deref(),
        Some("scratch")
    );
    assert_eq!(
        runner.attach_calls(),
        vec![(Some("scratch".to_string()), "ingest".to_string())]
    );
    assert_eq!(
        runner.send_keys_calls(),
        vec![(
            Some("scratch".to_string()),
            "ingest".to_string(),
            "codex --resume".to_string(),
            true,
        )]
    );
    assert_eq!(
        runner.rename_calls(),
        vec![(
            Some("scratch".to_string()),
            "ingest".to_string(),
            "ingest-refactor".to_string(),
        )]
    );
}

#[test]
fn fake_runner_returns_registered_outcomes_per_method() {
    let runner = FakeTmux::with_sessions("")
        .with_new_session("taken", TmuxNewSessionOutcome::NameTaken)
        .with_attach("missing", TmuxAttachOutcome::NoTarget)
        .with_send_keys("missing", TmuxSendKeysOutcome::NoTarget);

    assert_eq!(
        runner
            .new_session(None, "taken", Path::new("/tmp"), &[])
            .unwrap(),
        TmuxNewSessionOutcome::NameTaken
    );
    assert_eq!(
        runner.attach_session(None, "missing").unwrap(),
        TmuxAttachOutcome::NoTarget
    );
    assert_eq!(
        runner.send_keys(None, "missing", "echo hi", false).unwrap(),
        TmuxSendKeysOutcome::NoTarget
    );
}

#[test]
fn default_trait_impls_for_new_methods_return_unsupported() {
    struct MinimalRunner;
    impl MuxBackend for MinimalRunner {
        fn backend_key(&self) -> &'static str {
            "test-minimal"
        }

        fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
            Ok(TmuxOutcome::Sessions(String::new()))
        }
    }
    let runner = MinimalRunner;
    assert_eq!(
        runner
            .new_session(None, "x", Path::new("/tmp"), &[])
            .unwrap(),
        TmuxNewSessionOutcome::Unsupported
    );
    assert_eq!(
        runner.attach_session(None, "x").unwrap(),
        TmuxAttachOutcome::Unsupported
    );
    assert_eq!(
        runner.send_keys(None, "x", "y", false).unwrap(),
        TmuxSendKeysOutcome::Unsupported
    );
    assert_eq!(
        runner.kill_session(None, "x").unwrap(),
        TmuxKillOutcome::Unsupported
    );
}

#[test]
fn fake_runner_default_kill_session_records_call_and_returns_killed() {
    let runner = FakeTmux::with_sessions("");
    assert_eq!(
        runner.kill_session(Some("scratch"), "ingest").unwrap(),
        TmuxKillOutcome::Killed
    );
    assert_eq!(
        runner.kill_calls(),
        vec![(Some("scratch".to_string()), "ingest".to_string())]
    );
}

#[test]
fn fake_runner_returns_registered_kill_outcome_by_target() {
    let runner = FakeTmux::with_sessions("").with_kill("gone", TmuxKillOutcome::NoTarget);
    assert_eq!(
        runner.kill_session(None, "gone").unwrap(),
        TmuxKillOutcome::NoTarget
    );
    // Unregistered targets still default to Killed.
    assert_eq!(
        runner.kill_session(None, "other").unwrap(),
        TmuxKillOutcome::Killed
    );
}

#[test]
fn missing_binary_kill_session_reports_unavailable_binary_not_found() {
    let runner = SystemTmux::with_binary("/definitely/not/here/tmux");
    assert_eq!(
        runner.kill_session(None, "ingest").expect("non-fatal"),
        TmuxKillOutcome::Unavailable(UnavailableReason::BinaryNotFound)
    );
}

#[test]
fn missing_binary_new_session_reports_unavailable() {
    let runner = SystemTmux::with_binary("/definitely/no/such/binary");
    let outcome = runner
        .new_session(None, "ingest", Path::new("/tmp"), &[])
        .expect("non-fatal");
    assert!(matches!(
        outcome,
        TmuxNewSessionOutcome::Unavailable(UnavailableReason::BinaryNotFound)
    ));
}
