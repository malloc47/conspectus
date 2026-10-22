use super::*;
use crate::discovery::tmux::{FakeTmux, TmuxNewSessionOutcome};

#[test]
fn mux_new_spawns_bare_session_with_empty_argv() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("bare", TmuxNewSessionOutcome::Created);
    let cwd = std::env::temp_dir();
    let args = MuxNewArgs {
        name: "bare".to_string(),
        cwd: Some(cwd.clone()),
        socket: None,
        no_attach: true,
        pin: None,
        no_launch: false,
    };
    args.run_with_runner(&runner).expect("bare mux create");
    let calls = runner.new_session_calls();
    assert_eq!(calls.len(), 1);
    let (socket, name, argv_cwd, argv) = &calls[0];
    assert_eq!(socket.as_deref(), None);
    assert_eq!(name, "bare");
    assert_eq!(argv_cwd, &cwd);
    assert!(argv.is_empty(), "bare mux new must pass empty argv");
}

#[test]
fn mux_new_rejects_empty_name() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxNewArgs {
        name: "   ".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        no_attach: true,
        pin: None,
        no_launch: false,
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("empty name rejected");
    assert!(err.to_string().contains("non-empty"));
    assert!(runner.new_session_calls().is_empty());
}

#[test]
fn mux_new_rejects_missing_cwd() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxNewArgs {
        name: "bare".to_string(),
        cwd: Some(PathBuf::from("/nonexistent/definitely-not-here-12345")),
        socket: None,
        no_attach: true,
        pin: None,
        no_launch: false,
    };
    let err = args.run_with_runner(&runner).expect_err("bad cwd rejected");
    assert!(err.to_string().contains("not an existing directory"));
    assert!(runner.new_session_calls().is_empty());
}

#[test]
fn mux_new_threads_socket_argument() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("boxed", TmuxNewSessionOutcome::Created);
    let args = MuxNewArgs {
        name: "boxed".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: Some("iso".to_string()),
        no_attach: true,
        pin: None,
        no_launch: false,
    };
    args.run_with_runner(&runner).expect("socketed create");
    let (socket, _, _, _) = &runner.new_session_calls()[0];
    assert_eq!(socket.as_deref(), Some("iso"));
}

#[test]
fn mux_new_surfaces_name_taken_error() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("dup", TmuxNewSessionOutcome::NameTaken);
    let args = MuxNewArgs {
        name: "dup".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        no_attach: true,
        pin: None,
        no_launch: false,
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("duplicate name should error");
    let msg = err.to_string();
    assert!(msg.contains("already exists"), "unexpected msg: {msg}");
}

// -- mux launch ---------------------------------------------------

#[test]
fn mux_launch_spawns_with_harness_default_argv() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("adhoc", TmuxNewSessionOutcome::Created);
    let cwd = std::env::temp_dir();
    let args = MuxLaunchArgs {
        harness: "codex".to_string(),
        name: "adhoc".to_string(),
        cwd: Some(cwd.clone()),
        socket: None,
        argv: None,
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    args.run_with_runner(&runner).expect("mux launch");
    let calls = runner.new_session_calls();
    assert_eq!(calls.len(), 1);
    let (socket, name, argv_cwd, argv) = &calls[0];
    assert_eq!(socket.as_deref(), None);
    assert_eq!(name, "adhoc");
    assert_eq!(argv_cwd, &cwd);
    assert!(
        !argv.is_empty(),
        "mux launch must pass the harness's default argv"
    );
    // Codex adapter's default argv is `["codex"]`.
    assert_eq!(argv[0], std::ffi::OsString::from("codex"));
}

#[test]
fn mux_launch_argv_override_overrides_harness_default() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("adhoc", TmuxNewSessionOutcome::Created);
    let args = MuxLaunchArgs {
        harness: "codex".to_string(),
        name: "adhoc".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        argv: Some(vec![
            "codex".to_string(),
            "--model".to_string(),
            "gpt-5".to_string(),
        ]),
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    args.run_with_runner(&runner).expect("mux launch");
    let (_, _, _, argv) = &runner.new_session_calls()[0];
    assert_eq!(argv.len(), 3);
    assert_eq!(argv[1], std::ffi::OsString::from("--model"));
    assert_eq!(argv[2], std::ffi::OsString::from("gpt-5"));
}

#[test]
fn mux_launch_rejects_empty_name() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxLaunchArgs {
        harness: "codex".to_string(),
        name: "   ".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        argv: None,
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("empty name rejected");
    assert!(err.to_string().contains("non-empty"));
    assert!(runner.new_session_calls().is_empty());
}

#[test]
fn mux_launch_rejects_empty_harness() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxLaunchArgs {
        harness: "  ".to_string(),
        name: "adhoc".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        argv: None,
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("empty harness rejected");
    assert!(err.to_string().contains("HARNESS"));
}

#[test]
fn mux_launch_rejects_worktree_branch_without_repo() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxLaunchArgs {
        harness: "codex".to_string(),
        name: "adhoc".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        argv: None,
        worktree_branch: Some("feat/foo".to_string()),
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("branch without repo rejected");
    assert!(err.to_string().contains("--worktree-repo"));
}

#[test]
fn mux_launch_threads_socket_argument() {
    let runner =
        FakeTmux::with_sessions("").with_new_session("boxed", TmuxNewSessionOutcome::Created);
    let args = MuxLaunchArgs {
        harness: "codex".to_string(),
        name: "boxed".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: Some("iso".to_string()),
        argv: None,
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    args.run_with_runner(&runner).expect("socketed launch");
    let (socket, _, _, _) = &runner.new_session_calls()[0];
    assert_eq!(socket.as_deref(), Some("iso"));
}

#[test]
fn mux_launch_rejects_unknown_harness_with_no_argv() {
    let runner = FakeTmux::with_sessions("");
    let args = MuxLaunchArgs {
        harness: "not-a-real-harness".to_string(),
        name: "adhoc".to_string(),
        cwd: Some(std::env::temp_dir()),
        socket: None,
        argv: None,
        worktree_branch: None,
        worktree_repo: None,
        no_attach: true,
        pin: None,
        no_launch: false,
        scan_root: Vec::new(),
    };
    let err = args
        .run_with_runner(&runner)
        .expect_err("unknown harness w/o argv rejected");
    assert!(err.to_string().contains("no launch argv"));
}
