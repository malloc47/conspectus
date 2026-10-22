//! History end to end (ADR 0140): a real git repo and a private tmux
//! server, observed by the CLI in an isolated home.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use serde_json::Value;

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
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&tmux_tmpdir).unwrap();
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

    fn history_dir(&self) -> PathBuf {
        self.home.join(".local/state/conspectus/history")
    }

    fn conspectus(&self) -> Command {
        let mut cmd = Command::cargo_bin("conspectus").expect("binary");
        // tmux prints tabs in `-F` formats as `_` without a UTF-8
        // locale, so the private server needs one, as a login shell has.
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

    fn tmux(&self, args: &[&str]) -> bool {
        StdCommand::new("tmux")
            .args(args)
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env_remove("TMUX")
            .status()
            .is_ok_and(|status| status.success())
    }

    fn graph(&self, extra: &[&str]) -> Value {
        let output = self
            .conspectus()
            .args(["graph", "--format", "json"])
            .args(extra)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn records(&self, file: &str) -> Vec<Value> {
        let text = std::fs::read_to_string(self.history_dir().join(file)).unwrap_or_default();
        if text.is_empty() {
            return Vec::new();
        }
        let value: Value = serde_json::from_str(&text).unwrap();
        value["records"].as_array().cloned().unwrap_or_default()
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

fn names(records: &[Value]) -> Vec<String> {
    records
        .iter()
        .map(|record| record["recipe"]["name"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn no_cache_never_creates_history() {
    let world = World::new();
    world.graph(&["--no-cache"]);
    assert!(!world.history_dir().exists());
}

#[test]
fn a_run_records_the_repo_and_checkout_it_discovered() {
    let world = World::new();
    world.graph(&[]);
    let repos = world.records("repos.json");
    assert_eq!(repos.len(), 1, "{repos:?}");
    assert!(
        repos[0]["recipe"]["common_dir"]
            .as_str()
            .unwrap()
            .ends_with("src/app/.git")
    );
    let checkouts = world.records("checkouts.json");
    assert_eq!(checkouts.len(), 1);
    assert_eq!(checkouts[0]["recipe"]["branch"], "main");
    assert!(
        checkouts[0]["recipe"]["head"]
            .as_str()
            .is_some_and(|h| h.len() == 40)
    );
}

#[test]
fn a_hand_made_mux_is_followed_through_a_rename_and_goes_gone_when_killed() {
    if !tmux_available() {
        eprintln!("tmux not installed; skipping");
        return;
    }
    let world = World::new();
    let repo = world.repo.display().to_string();
    assert!(world.tmux(&[
        "new-session",
        "-d",
        "-s",
        "hist-a",
        "-c",
        &repo,
        "sleep",
        "600"
    ]));

    world.graph(&[]);
    let muxes = world.records("muxes.json");
    assert_eq!(names(&muxes), ["hist-a"]);
    let id = muxes[0]["id"].clone();
    assert_eq!(
        muxes[0]["recipe"]["argv"],
        serde_json::json!(["sleep", "600"])
    );
    assert_eq!(muxes[0]["recipe"]["argv_source"], "observed");

    // Renamed outside Conspectus: the same record follows it.
    assert!(world.tmux(&["rename-session", "-t", "hist-a", "hist-b"]));
    world.graph(&[]);
    let muxes = world.records("muxes.json");
    assert_eq!(names(&muxes), ["hist-b"]);
    assert_eq!(muxes[0]["id"], id);

    // Killed: the probe succeeds with no server, so the mux is gone,
    // and it is still in the graph as a gone node.
    assert!(world.tmux(&["kill-server"]));
    let graph = world.graph(&[]);
    let gone: Vec<_> = graph["node_presence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["presence"] == "gone")
        .map(|entry| {
            entry["node_id"]["native_id"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(gone, ["tmux:hist-b"], "{graph:#}");
    assert_eq!(world.records("muxes.json")[0]["id"], id, "history keeps it");
}

#[test]
fn a_removed_worktree_goes_gone_with_its_branch_and_head() {
    let world = World::new();
    let worktree = world.repo.parent().unwrap().join("app-feature");
    assert!(
        StdCommand::new("git")
            .args(["worktree", "add", "--quiet", "-b", "feature"])
            .arg(&worktree)
            .current_dir(&world.repo)
            .status()
            .unwrap()
            .success()
    );
    world.graph(&[]);
    let recorded = world
        .records("checkouts.json")
        .into_iter()
        .filter(|record| record["recipe"]["branch"] == "feature")
        .count();
    assert_eq!(recorded, 1);

    assert!(
        StdCommand::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&worktree)
            .current_dir(&world.repo)
            .status()
            .unwrap()
            .success()
    );
    let graph = world.graph(&[]);
    let gone_roots: Vec<_> = graph["node_presence"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["presence"] == "gone" && entry["node_id"]["type"] == "checkout")
        .map(|entry| entry["node_id"]["root"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(gone_roots, [path(&worktree)], "{graph:#}");
    let record = world
        .records("checkouts.json")
        .into_iter()
        .find(|record| record["recipe"]["branch"] == "feature")
        .unwrap();
    assert!(
        record["recipe"]["head"]
            .as_str()
            .is_some_and(|h| h.len() == 40)
    );
}

fn path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

#[test]
fn snapshot_mode_never_records_history() {
    let world = World::new();
    let output = world
        .conspectus()
        .args(["tui", "--snapshot", "--snapshot-width", "80", "--snapshot-height", "10"])
        .output()
        .unwrap();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Built without the dev-only `snapshot` feature.
        assert!(stderr.contains("--snapshot"), "{stderr}");
        return;
    }
    assert!(!world.history_dir().exists());
}
