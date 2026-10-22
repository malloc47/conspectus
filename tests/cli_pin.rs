//! Pin slots end to end (ADR 0141): the `pin` commands against a real
//! git repo and, where a mux matters, a private tmux server, in an
//! isolated home.

use std::path::PathBuf;
use std::process::Command as StdCommand;

use assert_cmd::Command;

struct World {
    _temp: tempfile::TempDir,
    home: PathBuf,
    repo: PathBuf,
    tmux_tmpdir: PathBuf,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let repo = temp.path().join("src/app");
        let tmux_tmpdir = temp.path().join("tmux");
        for dir in [&home, &repo, &tmux_tmpdir] {
            std::fs::create_dir_all(dir).unwrap();
        }
        for args in [
            &["init", "--quiet", "--initial-branch", "main"][..],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "init",
            ][..],
        ] {
            assert!(
                StdCommand::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        Self {
            _temp: temp,
            home,
            repo,
            tmux_tmpdir,
        }
    }

    fn pins_file(&self) -> PathBuf {
        self.home.join(".local/share/conspectus/pins.toml")
    }

    fn conspectus(&self) -> Command {
        let mut cmd = Command::cargo_bin("conspectus").expect("binary");
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LC_CTYPE", "C.UTF-8")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_STATE_HOME", self.home.join(".local/state"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .env("XDG_RUNTIME_DIR", self.home.join("no-daemon"))
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env("CONSPECTUS_DISABLE_FORGE", "1")
            .env("CONSPECTUS_DISABLE_ZELLIJ", "1")
            .current_dir(&self.repo);
        cmd
    }

    /// Run `conspectus <args>`; returns (success, stdout, stderr).
    fn run(&self, args: &[&str]) -> (bool, String, String) {
        let output = self.conspectus().args(args).output().unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (success, stdout, stderr) = self.run(args);
        assert!(success, "conspectus {args:?} failed: {stderr}\n{stdout}");
        stdout
    }

    fn fails(&self, args: &[&str]) -> String {
        let (success, stdout, stderr) = self.run(args);
        assert!(!success, "conspectus {args:?} should fail: {stdout}");
        stderr
    }

    fn tmux(&self, args: &[&str]) -> bool {
        StdCommand::new("tmux")
            .args(args)
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env_remove("TMUX")
            .status()
            .is_ok_and(|status| status.success())
    }

    fn start_mux(&self, name: &str) {
        let repo = self.repo.display().to_string();
        assert!(self.tmux(&["new-session", "-d", "-s", name, "-c", &repo, "sleep", "600"]));
    }

    fn pins_text(&self) -> String {
        std::fs::read_to_string(self.pins_file()).unwrap_or_default()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = self.tmux(&["kill-server"]);
    }
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn listing_with_no_slots_points_at_pin_add() {
    let world = World::new();
    let out = world.ok(&["pin", "list"]);
    assert!(out.contains("no pin slots"), "{out}");
    assert!(!world.pins_file().exists(), "listing writes nothing");
}

#[test]
fn the_old_verbs_fail_naming_their_replacement() {
    let world = World::new();
    for (verb, replacement) in [
        ("create", "pin add"),
        ("launch", "rebuild"),
        ("attach", "rebuild"),
        ("bind", "pin retarget"),
        ("rebind", "retarget"),
        ("adopt", "pin add"),
    ] {
        let stderr = world.fails(&["pin", verb, "whatever", "--harness", "codex"]);
        assert!(
            stderr.contains(&format!("`conspectus pin {verb}` was retired")),
            "{stderr}"
        );
        assert!(stderr.contains(replacement), "{verb}: {stderr}");
    }
    assert!(!world.pins_file().exists());
}

#[test]
fn a_planned_slot_round_trips_through_edit_rename_move_and_rm() {
    let world = World::new();
    world.ok(&[
        "pin",
        "add",
        "--planned",
        "--kind",
        "mux",
        "--name",
        "alpha",
        "--set",
        "cwd=/tmp",
    ]);
    world.ok(&["pin", "add", "--planned", "--kind", "mux", "--name", "beta"]);

    let list = world.ok(&["pin", "list"]);
    assert!(
        list.find("alpha").unwrap() < list.find("beta").unwrap(),
        "{list}"
    );
    assert!(list.contains("planned"), "{list}");
    let text = world.pins_text();
    assert!(text.contains("schema_version = 2"), "{text}");
    assert!(text.contains("cwd = \"/tmp\""), "{text}");

    world.ok(&[
        "pin",
        "edit",
        "alpha",
        "--set",
        "harness=codex",
        "--unset",
        "cwd",
    ]);
    let text = world.pins_text();
    assert!(text.contains("harness = \"codex\""), "{text}");
    assert!(!text.contains("cwd"), "{text}");

    world.ok(&["pin", "rename", "alpha", "gamma"]);
    world.ok(&["pin", "move", "beta", "--top"]);
    let list = world.ok(&["pin", "list"]);
    assert!(!list.contains("alpha"), "{list}");
    assert!(
        list.find("beta").unwrap() < list.find("gamma").unwrap(),
        "{list}"
    );

    world.ok(&["pin", "rm", "beta"]);
    let list = world.ok(&["pin", "list"]);
    assert!(!list.contains("beta") && list.contains("gamma"), "{list}");
    let stderr = world.fails(&["pin", "rm", "beta"]);
    assert!(stderr.contains("no pin slot named `beta`"), "{stderr}");
}

#[test]
fn bad_input_is_refused_before_anything_is_written() {
    let world = World::new();
    let stderr = world.fails(&["pin", "add", "--planned", "--kind", "bogus", "--name", "x"]);
    assert!(stderr.contains("unknown kind"), "{stderr}");
    let stderr = world.fails(&[
        "pin",
        "add",
        "--planned",
        "--kind",
        "pr",
        "--name",
        "x",
        "--set",
        "argv=a",
    ]);
    assert!(stderr.contains("no overridable field `argv`"), "{stderr}");
    let stderr = world.fails(&["pin", "add", "no-such-thing"]);
    assert!(stderr.contains("nothing matches"), "{stderr}");
    let stderr = world.fails(&["pin", "edit", "ghost", "--set", "cwd=/x"]);
    assert!(stderr.contains("no pin slot named `ghost`"), "{stderr}");
    assert!(!world.pins_file().exists());
}

#[test]
fn pinning_a_live_mux_survives_its_death_and_rebuilds() {
    if !tmux_available() {
        eprintln!("tmux not installed; skipping");
        return;
    }
    let world = World::new();
    world.start_mux("pin-a");

    let out = world.ok(&["pin", "add", "pin-a"]);
    assert!(out.contains("pinned mux"), "{out}");
    let list = world.ok(&["pin", "list"]);
    assert!(list.contains("pin-a") && list.contains("live"), "{list}");
    assert!(
        world.pins_text().contains("target = \"history:h_"),
        "{}",
        world.pins_text()
    );

    // The mux dies; the slot stays and reports it.
    assert!(world.tmux(&["kill-server"]));
    let list = world.ok(&["pin", "list"]);
    assert!(list.contains("pin-a") && list.contains("gone"), "{list}");
    let history = world.ok(&["history", "list", "--gone"]);
    assert!(
        history.contains("pin-a") && history.contains("◉"),
        "{history}"
    );

    let show = world.ok(&["pin", "show", "pin-a"]);
    assert!(show.contains("gone"), "{show}");
    let plan = world.ok(&["rebuild", "pin-a", "--dry-run"]);
    assert!(plan.contains("pin-a"), "{plan}");

    // A pinned record can't be forgotten by accident.
    let id = history.split_whitespace().next().unwrap().to_string();
    let stderr = world.fails(&["history", "forget", &id]);
    assert!(stderr.contains("pin slot"), "{stderr}");
    world.ok(&["history", "forget", &id, "--force"]);
    let list = world.ok(&["pin", "list"]);
    assert!(
        list.contains("missing"),
        "the slot outlives its record: {list}"
    );
}

#[test]
fn history_reap_skips_pinned_and_recent_records() {
    if !tmux_available() {
        eprintln!("tmux not installed; skipping");
        return;
    }
    let world = World::new();
    world.start_mux("reap-a");
    world.start_mux("reap-b");
    world.ok(&["graph", "--format", "json"]);
    world.ok(&["pin", "add", "reap-a"]);
    assert!(world.tmux(&["kill-server"]));

    // Both are gone but under the 14 day default.
    let out = world.ok(&["history", "reap", "--dry-run"]);
    assert!(out.contains("nothing to reap"), "{out}");

    // With a zero threshold only the unpinned one qualifies.
    let out = world.ok(&[
        "history",
        "reap",
        "--kind",
        "mux",
        "--older-than",
        "0s",
        "--dry-run",
    ]);
    assert!(out.contains("reap-b") && !out.contains("reap-a"), "{out}");
    world.ok(&["history", "reap", "--kind", "mux", "--older-than", "0s"]);
    let out = world.ok(&["history", "list", "--kind", "mux"]);
    assert!(out.contains("reap-a") && !out.contains("reap-b"), "{out}");
}

#[test]
fn mux_new_with_pin_and_no_launch_only_plans_a_slot() {
    let world = World::new();
    let out = world.ok(&["mux", "new", "later", "--pin", "--no-launch"]);
    assert!(out.contains("pinned `later` (planned)"), "{out}");
    assert!(
        world.pins_text().contains("id = \"later\""),
        "{}",
        world.pins_text()
    );
    assert!(
        !world.tmux(&["has-session", "-t", "later"]),
        "nothing was started"
    );
    let stderr = world.fails(&["mux", "new", "later", "--pin", "--no-launch"]);
    assert!(stderr.contains("already exists"), "{stderr}");
    let stderr = world.fails(&["mux", "new", "x", "--no-launch"]);
    assert!(stderr.contains("--pin"), "{stderr}");
}

const LEGACY: &str = r#"[session]
poll_ms = 250

[pins]
schema_version = 1

[[pins.entries]]
id = "legacy-api"
display_name = "API work"
harness = "claude-code"
cwd = "__CWD__"
[pins.entries.mux]
backend = "tmux"
name = "legacy-api"
"#;

#[test]
fn migrate_converts_legacy_entries_once() {
    let world = World::new();
    let config = world.repo.join(".conspectus.toml");
    std::fs::write(
        &config,
        LEGACY.replace("__CWD__", &world.repo.display().to_string()),
    )
    .unwrap();

    // Until migrated, the legacy pin still shows, flagged.
    let list = world.ok(&["pin", "list"]);
    assert!(
        list.contains("legacy-api") && list.contains("pin migrate"),
        "{list}"
    );
    let stderr = world.fails(&["pin", "edit", "legacy-api", "--set", "cwd=/x"]);
    assert!(stderr.contains("pin migrate"), "{stderr}");

    let dry = world.ok(&["pin", "migrate", "--dry-run"]);
    assert!(dry.contains("pin `legacy-api` -> mux slot"), "{dry}");
    assert!(
        std::fs::read_to_string(&config)
            .unwrap()
            .contains("legacy-api")
    );
    assert!(!world.pins_file().exists(), "a dry run writes nothing");

    let out = world.ok(&["pin", "migrate"]);
    assert!(out.contains("migrated 1 pin(s)"), "{out}");
    let text = world.pins_text();
    assert!(
        text.contains("id = \"legacy-api\"") && text.contains("target = \"history:h_"),
        "{text}"
    );
    let config_text = std::fs::read_to_string(&config).unwrap();
    assert!(
        config_text.contains("poll_ms = 250") && !config_text.contains("legacy-api"),
        "{config_text}"
    );

    let list = world.ok(&["pin", "list"]);
    assert!(
        list.contains("legacy-api") && !list.contains("pin migrate"),
        "{list}"
    );
    let again = world.ok(&["pin", "migrate"]);
    assert!(again.contains("no legacy pins"), "{again}");
}

#[test]
fn history_show_prints_the_record_and_its_pins() {
    let world = World::new();
    world.ok(&["graph", "--format", "json"]);
    let list = world.ok(&["history", "list", "--kind", "repo"]);
    let id = list.split_whitespace().next().unwrap().to_string();
    assert!(id.starts_with("h_"), "{list}");

    world.ok(&["pin", "add", &id, "--name", "the-repo"]);
    let show = world.ok(&["history", "show", &id]);
    assert!(
        show.contains("\"common_dir\"") && show.contains("pinned by: the-repo"),
        "{show}"
    );
    let stderr = world.fails(&["history", "show", "h_000000000000"]);
    assert!(stderr.contains("no history record"), "{stderr}");
}

#[test]
fn renaming_a_live_mux_slot_renames_the_tmux_session_with_it() {
    if !tmux_available() {
        eprintln!("tmux not installed; skipping");
        return;
    }
    let world = World::new();
    world.start_mux("ren-a");
    world.ok(&["pin", "add", "ren-a"]);

    world.ok(&["pin", "rename", "ren-a", "ren-b"]);

    assert!(
        world.tmux(&["has-session", "-t", "ren-b"]),
        "the session was renamed"
    );
    assert!(!world.tmux(&["has-session", "-t", "=ren-a"]));
    let text = world.pins_text();
    assert!(text.contains("id = \"ren-b\""), "{text}");
    assert!(
        text.contains("name = \"ren-b\""),
        "the name override follows: {text}"
    );
    let list = world.ok(&["pin", "list"]);
    assert!(list.contains("ren-b") && list.contains("live"), "{list}");

    // `--no-mux` renames the slot alone.
    world.ok(&["pin", "rename", "ren-b", "ren-c", "--no-mux"]);
    assert!(
        world.tmux(&["has-session", "-t", "ren-b"]),
        "tmux kept its name"
    );
    let text = world.pins_text();
    assert!(
        text.contains("id = \"ren-c\"") && text.contains("name = \"ren-b\""),
        "{text}"
    );
}
