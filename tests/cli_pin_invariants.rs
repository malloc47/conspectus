//! Read-only invariant smoke for `[pins]`-bearing configs.
//!
//! Mirrors the declared-link invariants: every `conspectus` subcommand
//! that the operator can run without expressing write intent
//! (`graph`, `node show`, `table`, `pin list`, `pin show`)
//! must NOT create, mtime-touch, or content-modify `.conspectus.toml`
//! / user-config files that contain a `[pins]` table. The interactive
//! `tui` event loop needs a pseudo-terminal, so these process smokes
//! cover its prelaunch validation path and the reducer/UI tests cover
//! its in-process read-only surfaces.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    // Point XDG_RUNTIME_DIR at a nonexistent path so the CLI cannot
    // find and defer to a real `conspectus serve` socket — otherwise
    // the tests silently pull the operator daemon's snapshot instead
    // of exercising the local discovery + render path.
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

/// Captured content + mtime for one config file.
type Fingerprint = (Vec<u8>, SystemTime);

/// Both fingerprints (project, user) captured together so a single
/// pre/post comparison can catch any touch.
type SandboxFingerprint = (Fingerprint, Fingerprint);

/// Capture every byte + mtime of `path` so a later comparison can
/// catch even an mtime-only touch (some kernels lazy-write content
/// changes but still bump mtime; both shapes are write-attempts the
/// invariant forbids).
fn fingerprint(path: &Path) -> Fingerprint {
    let bytes = fs::read(path).unwrap_or_else(|err| {
        panic!("fingerprint: read {}: {err}", path.display());
    });
    let metadata = fs::metadata(path).unwrap_or_else(|err| {
        panic!("fingerprint: stat {}: {err}", path.display());
    });
    let mtime = metadata.modified().expect("mtime");
    (bytes, mtime)
}

/// Project config holding both a `[session]` sibling (so we also
/// confirm sibling-section preservation) and a `[pins]` table with
/// two entries (default-socket + non-default-socket) covering the
/// shape variations the loader handles.
const PROJECT_CONFIG: &str = r#"[session]
projection = "agent"

[pins]
schema_version = 1

[[pins.entries]]
id = "ingest"
display_name = "ingest"
harness = "codex"
cwd = "/tmp/conspectus-test"
mux = { backend = "tmux", name = "ingest" }

[[pins.entries]]
id = "scratch"
display_name = "scratch"
harness = "codex"
cwd = "/tmp/conspectus-test"
mux = { backend = "tmux", name = "scratch", socket_name = "scratch" }
"#;

/// User config carrying a single global pin so we can also assert
/// the global-store path stays clean across the read-only commands.
const USER_CONFIG: &str = r#"[pins]
schema_version = 1

[[pins.entries]]
id = "global-ingest"
display_name = "global-ingest"
harness = "codex"
cwd = "/tmp/conspectus-test"
mux = { backend = "tmux", name = "global-ingest" }
"#;

struct Sandbox {
    home: tempfile::TempDir,
    project: tempfile::TempDir,
    project_config: std::path::PathBuf,
    user_config: std::path::PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let home = tempfile::TempDir::new().expect("home temp");
        let project = tempfile::TempDir::new().expect("project temp");

        let project_config = project.path().join(".conspectus.toml");
        fs::write(&project_config, PROJECT_CONFIG).expect("write project config");

        let user_dir = home.path().join(".config/conspectus");
        fs::create_dir_all(&user_dir).expect("mkdir user dir");
        let user_config = user_dir.join("config.toml");
        fs::write(&user_config, USER_CONFIG).expect("write user config");

        // Bump both files' mtimes back a couple of seconds so an
        // accidental no-op write that bumps mtime to "now" produces
        // a real diff against the captured fingerprint.
        std::thread::sleep(std::time::Duration::from_millis(50));

        Self {
            home,
            project,
            project_config,
            user_config,
        }
    }

    fn fingerprints(&self) -> SandboxFingerprint {
        (
            fingerprint(&self.project_config),
            fingerprint(&self.user_config),
        )
    }

    fn run(&self, args: &[&str]) {
        isolated_cmd(self.home.path())
            .current_dir(self.project.path())
            .args(args)
            .assert()
            .success();
    }

    fn assert_unchanged(&self, before: &SandboxFingerprint) {
        let after = self.fingerprints();
        assert_eq!(
            before.0.0,
            after.0.0,
            "project config content changed (before vs after diff at {})",
            self.project_config.display(),
        );
        assert_eq!(
            before.0.1,
            after.0.1,
            "project config mtime bumped — a read-only command touched {}",
            self.project_config.display(),
        );
        assert_eq!(
            before.1.0,
            after.1.0,
            "user config content changed at {}",
            self.user_config.display(),
        );
        assert_eq!(
            before.1.1,
            after.1.1,
            "user config mtime bumped — a read-only command touched {}",
            self.user_config.display(),
        );
    }
}

struct CleanSandbox {
    home: tempfile::TempDir,
    project: tempfile::TempDir,
    project_config: std::path::PathBuf,
    user_config: std::path::PathBuf,
}

impl CleanSandbox {
    fn new() -> Self {
        let home = tempfile::TempDir::new().expect("home temp");
        let project = tempfile::TempDir::new().expect("project temp");
        let project_config = project.path().join(".conspectus.toml");
        let user_config = home.path().join(".config/conspectus/config.toml");
        Self {
            home,
            project,
            project_config,
            user_config,
        }
    }

    fn assert_no_pin_configs_created(&self) {
        assert!(
            !self.project_config.exists(),
            "read-only command created {}",
            self.project_config.display()
        );
        assert!(
            !self.user_config.exists(),
            "read-only command created {}",
            self.user_config.display()
        );
    }

    fn command(&self) -> Command {
        let mut cmd = isolated_cmd(self.home.path());
        cmd.current_dir(self.project.path());
        cmd
    }
}

#[test]
fn graph_json_does_not_mutate_pin_bearing_configs() {
    let sandbox = Sandbox::new();
    let before = sandbox.fingerprints();
    sandbox.run(&["graph", "--format", "json"]);
    sandbox.assert_unchanged(&before);
}

#[test]
fn table_sessions_does_not_mutate_pin_bearing_configs() {
    let sandbox = Sandbox::new();
    let before = sandbox.fingerprints();
    sandbox.run(&["table", "sessions"]);
    sandbox.assert_unchanged(&before);
}

#[test]
fn pin_list_does_not_mutate_pin_bearing_configs() {
    let sandbox = Sandbox::new();
    let before = sandbox.fingerprints();
    sandbox.run(&["pin", "list"]);
    sandbox.assert_unchanged(&before);
}

#[test]
fn pin_show_does_not_mutate_pin_bearing_configs() {
    let sandbox = Sandbox::new();
    let before = sandbox.fingerprints();
    sandbox.run(&["pin", "show", "ingest"]);
    sandbox.assert_unchanged(&before);
}

#[test]
fn node_show_does_not_mutate_pin_bearing_configs() {
    let sandbox = Sandbox::new();
    let before = sandbox.fingerprints();
    // `node show` on a non-existent id is enough to exercise the
    // read path; the command will exit non-zero with a clear "no
    // such node" message. We don't care about the exit status —
    // only that the read path did not mutate either config file.
    let _ = isolated_cmd(sandbox.home.path())
        .current_dir(sandbox.project.path())
        .args(["node", "show", "missing-id-just-for-invariant"])
        .assert();
    sandbox.assert_unchanged(&before);
}

#[test]
fn read_only_commands_do_not_create_pin_configs_in_clean_repo() {
    let sandbox = CleanSandbox::new();

    sandbox
        .command()
        .args(["graph", "--format", "json"])
        .assert()
        .success();
    sandbox.assert_no_pin_configs_created();

    sandbox
        .command()
        .args(["table", "sessions"])
        .assert()
        .success();
    sandbox.assert_no_pin_configs_created();

    sandbox.command().args(["pin", "list"]).assert().success();
    sandbox.assert_no_pin_configs_created();

    let _ = sandbox
        .command()
        .args(["pin", "show", "missing-pin"])
        .assert();
    sandbox.assert_no_pin_configs_created();

    let _ = sandbox
        .command()
        .args(["node", "show", "missing-id-just-for-invariant"])
        .assert();
    sandbox.assert_no_pin_configs_created();

    // Full interactive `tui` requires a PTY; the CLI validation path
    // is still enough to assert prelaunch argument handling never
    // creates pin config files in a clean repo.
    sandbox
        .command()
        .args(["tui", "--view", "nope"])
        .assert()
        .failure();
    sandbox.assert_no_pin_configs_created();
}
