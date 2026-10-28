//! The real TUI in a pane of a private tmux server, driven with
//! `send-keys` and read with `capture-pane`.
//!
//! Two tmux servers keep everything off the real one. The "inner" server
//! holds the muxes under test (the TUI's `TMUX_TMPDIR` points at it). The
//! "outer" server only hosts a pane that stands in for the operator's
//! terminal: either the TUI itself (the outside-tmux flow, where attach is
//! `tmux attach-session` in the TUI's own terminal) or a tmux client
//! attached to the inner server's `ui` session, whose window runs the TUI
//! (the inside-tmux flow, where attach is `switch-client`).
//!
//! Each test asserts the screen converges on the new state with no key
//! press after returning to the TUI, which is what an operator sees.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// How long a screen may take to converge before the test fails. Generous
/// because the suite runs many processes at once; a stale screen that
/// needs a manual refresh never converges, so the bound doesn't hide the
/// bug.
const CONVERGE: Duration = Duration::from_secs(10);

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
}

struct World {
    _temp: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    inner: PathBuf,
    outer: PathBuf,
    bin_dir: PathBuf,
    daemon: Option<std::process::Child>,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        // tmux socket paths must stay short.
        let root = temp.path().to_path_buf();
        let repo = root.join("home/src/app");
        let inner = root.join("in");
        let outer = root.join("out");
        let bin_dir = root.join("bin");
        let run = root.join("run");
        for dir in [&repo, &inner, &outer, &bin_dir, &run] {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
        // Stub harnesses: alive in a pane, never real agents.
        for harness in ["aider", "claude", "codex"] {
            let path = bin_dir.join(harness);
            fs::write(&path, "#!/bin/sh\nexec sleep 600\n").unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
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
                "i",
            ][..],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        Self {
            _temp: temp,
            root,
            repo,
            inner,
            outer,
            bin_dir,
            daemon: None,
        }
    }

    fn env_pairs(&self) -> Vec<(String, String)> {
        let home = self.root.join("home");
        let path = format!(
            "{}:{}",
            self.bin_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        vec![
            ("PATH".into(), path),
            ("HOME".into(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".into(),
                self.root.join("xdg/c").display().to_string(),
            ),
            (
                "XDG_STATE_HOME".into(),
                self.root.join("xdg/s").display().to_string(),
            ),
            (
                "XDG_DATA_HOME".into(),
                self.root.join("xdg/d").display().to_string(),
            ),
            (
                "XDG_CACHE_HOME".into(),
                self.root.join("xdg/k").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR".into(),
                self.root.join("run").display().to_string(),
            ),
            ("LC_CTYPE".into(), "C.UTF-8".into()),
            ("CONSPECTUS_DISABLE_FORGE".into(), "1".into()),
            ("CONSPECTUS_DISABLE_ZELLIJ".into(), "1".into()),
            ("TMUX_TMPDIR".into(), self.inner.display().to_string()),
        ]
    }

    /// `tmux` against the inner server (the muxes under test).
    fn inner(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-f", "/dev/null"])
            .args(args)
            .env_remove("TMUX")
            .env("HOME", self.root.join("home"))
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.inner)
            .output()
            .expect("tmux")
    }

    /// `tmux` against the outer server (the operator's terminal).
    fn outer(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-f", "/dev/null"])
            .args(args)
            .env_remove("TMUX")
            .env("HOME", self.root.join("home"))
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.outer)
            .output()
            .expect("tmux")
    }

    fn conspectus(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_conspectus"));
        cmd.args(args).env_clear().current_dir(&self.repo);
        for (key, value) in self.env_pairs() {
            cmd.env(key, value);
        }
        cmd.output().expect("conspectus")
    }

    fn start_daemon(&mut self) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_conspectus"));
        cmd.arg("serve")
            .env_clear()
            .current_dir(&self.repo)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (key, value) in self.env_pairs() {
            cmd.env(key, value);
        }
        self.daemon = Some(cmd.spawn().expect("serve"));
        let socket = self.root.join("run/conspectus/server.sock");
        wait_until("the daemon socket", Duration::from_secs(30), || {
            socket.exists()
        });
        sleep(Duration::from_secs(2));
    }

    fn shell_env_prefix(&self, keep_tmux: bool) -> String {
        let mut parts = Vec::new();
        if !keep_tmux {
            parts.push("-u TMUX".to_string());
        }
        for (key, value) in self.env_pairs() {
            parts.push(format!("{key}='{value}'"));
        }
        format!("env {}", parts.join(" "))
    }

    fn tui_command(&self, keep_tmux: bool) -> String {
        self.tui_command_with(keep_tmux, "")
    }

    /// The TUI's command line with `extra` flags after the usual ones.
    fn tui_command_with(&self, keep_tmux: bool, extra: &str) -> String {
        format!(
            "{} {} tui --view mux --refresh-interval 30s {extra} 2>{}; sleep 120",
            self.shell_env_prefix(keep_tmux),
            env!("CARGO_BIN_EXE_conspectus"),
            self.root.join("tui.err").display()
        )
    }

    /// Outside-tmux flow: the TUI runs directly in the operator's terminal.
    fn start_tui_outside(&self) {
        self.start_tui_outside_with("");
    }

    /// Like [`World::start_tui_outside`] with extra `tui` flags.
    fn start_tui_outside_with(&self, extra: &str) {
        let command = self.tui_command_with(false, extra);
        // A content filter can leave the Mux view with no rows, so wait
        // for the tab strip rather than a mux.
        let up = |screen: &str| {
            if extra.is_empty() {
                screen.contains('▣')
            } else {
                screen.contains("sessions · mux")
            }
        };
        assert!(
            self.outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
        );
        wait_until("the TUI to list muxes", CONVERGE, || up(&self.screen()));
    }

    /// Inside-tmux flow: the TUI runs in session `ui` of the inner server
    /// and the operator's terminal is a client attached to it.
    fn start_tui_inside(&self) {
        let command = self.tui_command(true);
        assert!(
            self.inner(&[
                "new-session",
                "-d",
                "-s",
                "ui",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
        );
        let attach = format!(
            "env -u TMUX TMUX_TMPDIR='{}' tmux attach -t ui; echo CLIENT-EXITED; sleep 120",
            self.inner.display()
        );
        assert!(
            self.outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &attach
            ])
            .status
            .success()
        );
        wait_until("the TUI to list muxes", CONVERGE, || {
            self.tui_pane().contains('▣')
        });
    }

    /// What the operator's terminal shows.
    fn screen(&self) -> String {
        let out = self.outer(&["capture-pane", "-p", "-t", "host"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// The TUI's own pane in the inside-tmux flow, whether or not a
    /// client is looking at it.
    fn tui_pane(&self) -> String {
        let out = self.inner(&["capture-pane", "-p", "-t", "ui"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn keys(&self, keys: &[&str]) {
        let mut args = vec!["send-keys", "-t", "host"];
        args.extend_from_slice(keys);
        assert!(self.outer(&args).status.success());
        sleep(Duration::from_millis(250));
    }

    fn new_mux(&self, name: &str, shell: bool) {
        let mut args = vec!["new-session", "-d", "-s", name, "-c"];
        let cwd = self.repo.display().to_string();
        args.push(&cwd);
        if !shell {
            args.extend(["sleep", "600"]);
        }
        assert!(self.inner(&args).status.success());
    }

    fn pin(&self, name: &str) {
        let out = self.conspectus(&["pin", "add", name, "--name", &format!("pin-{name}")]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn mux_alive(&self, name: &str) -> bool {
        self.inner(&["has-session", "-t", &format!("={name}")])
            .status
            .success()
    }

    /// Move the TUI's cursor onto the mux row for `name` (Mux view, first
    /// row selected).
    fn select_mux(&self, text: impl Fn() -> String, name: &str) {
        let screen = text();
        let rows: Vec<&str> = screen
            .lines()
            .map(left_pane)
            .filter(|line| line.contains('▣'))
            .collect();
        let index = rows
            .iter()
            .position(|line| line.contains(name))
            .unwrap_or_else(|| panic!("no row for {name}:\n{screen}"));
        for _ in 0..index {
            self.keys(&["j"]);
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if let Some(mut daemon) = self.daemon.take() {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
        let _ = self.inner(&["kill-server"]);
        let _ = self.outer(&["kill-server"]);
    }
}

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}");
}

/// Wait for the operator's terminal to satisfy `done`, showing the
/// screen if it never does.
fn wait_screen(world: &World, what: &str, done: impl Fn(&str) -> bool) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = world.screen();
        if done(&last) {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}:\n{last}");
}

/// The row text for `name` in a captured screen, if listed.
fn row<'a>(screen: &'a str, name: &str) -> Option<&'a str> {
    screen
        .lines()
        .map(left_pane)
        .find(|line| line.contains('▣') && line.contains(name))
}

/// The left pane's part of a screen line.
fn left_pane(line: &str) -> &str {
    line.split("││").next().unwrap_or(line)
}

/// A ghost row: the pin marker is hollow and the state is the stopped
/// glyph, with nothing saying it is running.
fn is_stopped_row(line: &str) -> bool {
    use conspectus::tui::icons::state::{PIN_STOPPED, STOPPED};
    line.contains(PIN_STOPPED) && line.contains(STOPPED)
}

fn assert_converges_to_stopped(world: &World, read: impl Fn(&World) -> String, name: &str) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = read(world);
        if row(&last, name).is_some_and(is_stopped_row) {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("`{name}` never showed as stopped without a refresh:\n{last}");
}

fn assert_converges_to_absent(world: &World, read: impl Fn(&World) -> String, name: &str) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = read(world);
        if row(&last, name).is_none() && last.contains("stopped") {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("`{name}` never left the Mux view without a refresh:\n{last}");
}

/// Attach to `victim` from the TUI (Enter on its row).
fn attach(world: &World, read: impl Fn() -> String) {
    world.select_mux(read, "victim");
    world.keys(&["Enter"]);
    sleep(Duration::from_millis(1500));
}

macro_rules! needs_tmux {
    () => {
        if !tmux_available() {
            eprintln!("tmux not installed; skipping");
            return;
        }
    };
}

// ---- Outside tmux: attach-session in the TUI's own terminal -----------

fn outside_exit(pinned: bool, daemon: bool) {
    let mut world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    if pinned {
        world.pin("victim");
    }
    if daemon {
        world.start_daemon();
    }
    world.start_tui_outside();
    attach(&world, || world.screen());
    world.keys(&["exit", "Enter"]);
    if pinned {
        assert_converges_to_stopped(&world, World::screen, "victim");
    } else {
        assert_converges_to_absent(&world, World::screen, "victim");
    }
    assert!(!world.mux_alive("victim"));
}

#[test]
fn outside_tmux_exiting_a_pinned_mux_shows_its_ghost_without_a_refresh() {
    needs_tmux!();
    outside_exit(true, false);
}

#[test]
fn outside_tmux_exiting_an_unpinned_mux_removes_it_from_the_mux_view() {
    needs_tmux!();
    outside_exit(false, false);
}

#[test]
fn outside_tmux_exiting_a_pinned_mux_with_a_daemon_shows_its_ghost() {
    needs_tmux!();
    outside_exit(true, true);
}

#[test]
fn outside_tmux_detaching_leaves_the_mux_running() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    world.start_tui_outside();
    attach(&world, || world.screen());
    world.keys(&["C-b", "d"]);
    wait_screen(&world, "the TUI to return", |screen| screen.contains('▣'));
    sleep(Duration::from_secs(2));
    let screen = world.screen();
    let line = row(&screen, "victim").unwrap_or_else(|| panic!("victim vanished:\n{screen}"));
    assert!(!is_stopped_row(line), "{screen}");
    assert!(world.mux_alive("victim"));
}

#[test]
fn outside_tmux_exiting_the_only_mux_converges_when_the_server_dies() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("victim", true);
    world.pin("victim");
    world.start_tui_outside();
    world.keys(&["Enter"]);
    sleep(Duration::from_millis(1500));
    world.keys(&["exit", "Enter"]);
    assert_converges_to_stopped(&world, World::screen, "victim");
}

// ---- Inside tmux: attach is switch-client ------------------------------

fn inside_exit(pinned: bool, detach_on_destroy: &str) {
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    if pinned {
        world.pin("victim");
    }
    world.start_tui_inside();
    world.inner(&["set", "-g", "detach-on-destroy", detach_on_destroy]);
    attach(&world, || world.tui_pane());
    // The client is now looking at the victim's shell.
    wait_until("the client to show the victim", CONVERGE, || {
        !world.screen().contains("pinned · history")
    });
    world.keys(&["exit", "Enter"]);
    if detach_on_destroy == "on" {
        // The client detached when its session ended: the operator
        // reattaches later and finds the TUI.
        wait_until("the client to exit", CONVERGE, || {
            world.screen().contains("CLIENT-EXITED")
        });
        let attach = format!(
            "env -u TMUX TMUX_TMPDIR='{}' tmux attach -t ui; sleep 120",
            world.inner.display()
        );
        world.outer(&["respawn-window", "-k", "-t", "host", &attach]);
    }
    if pinned {
        assert_converges_to_stopped(&world, World::tui_pane, "victim");
    } else {
        assert_converges_to_absent(&world, World::tui_pane, "victim");
    }
}

#[test]
fn inside_tmux_returning_after_exiting_a_pinned_mux_shows_its_ghost() {
    needs_tmux!();
    inside_exit(true, "off");
}

#[test]
fn inside_tmux_returning_after_exiting_an_unpinned_mux_removes_it() {
    needs_tmux!();
    inside_exit(false, "off");
}

#[test]
fn inside_tmux_reattaching_after_the_client_was_detached_shows_the_ghost() {
    needs_tmux!();
    inside_exit(true, "on");
}

#[test]
fn inside_tmux_a_kill_from_another_client_is_seen_on_return() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", false);
    world.pin("victim");
    world.start_tui_inside();
    world.inner(&["set", "-g", "detach-on-destroy", "off"]);
    attach(&world, || world.tui_pane());
    // Killed by someone else while the operator is in another session.
    world.inner(&["kill-session", "-t", "=victim"]);
    assert_converges_to_stopped(&world, World::tui_pane, "victim");
}

// ---- Other triggers -----------------------------------------------------

#[test]
fn an_idle_tui_notices_a_kill_on_its_timer() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", false);
    world.pin("victim");
    // A short interval stands in for the operator waiting.
    let command = world
        .tui_command(false)
        .replace("--refresh-interval 30s", "--refresh-interval 1s");
    assert!(
        world
            .outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
    );
    wait_until("the TUI to list muxes", CONVERGE, || {
        world.screen().contains("victim")
    });
    world.inner(&["kill-session", "-t", "=victim"]);
    assert_converges_to_stopped(&world, World::screen, "victim");
}

#[test]
fn launching_from_the_form_and_detaching_at_once_shows_the_new_mux_running() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.start_tui_outside();
    world.keys(&["N"]);
    sleep(Duration::from_millis(500));
    assert!(
        world.screen().contains("Launch harness"),
        "{}",
        world.screen()
    );
    world.keys(&["Enter"]);
    // Launching attaches; detach straight away.
    sleep(Duration::from_millis(1500));
    world.keys(&["C-b", "d"]);
    wait_until("the TUI to return", CONVERGE, || {
        world.screen().contains("pinned · history")
    });
    let start = Instant::now();
    loop {
        let screen = world.screen();
        let lines: Vec<&str> = screen
            .lines()
            .map(left_pane)
            .filter(|line| line.contains('▣') && !line.contains("other"))
            .collect();
        if lines.len() == 1 && !is_stopped_row(lines[0]) {
            break;
        }
        assert!(
            start.elapsed() < CONVERGE,
            "the new mux never showed as running without a refresh:\n{screen}"
        );
        sleep(Duration::from_millis(100));
    }
}

/// Pin with `p`, unpin with `p`, and bring the pin back with the undo
/// key, reading the screen and the pin store each time.
#[test]
fn undo_brings_back_a_pin_removed_with_p() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", false);
    world.start_tui_outside();
    world.select_mux(|| world.screen(), "victim");
    let pins = |world: &World| {
        let out = world.conspectus(&["pin", "list"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let marked =
        |screen: &str| row(screen, "victim").is_some_and(|line| left_pane(line).contains('◉'));

    world.keys(&["p"]);
    wait_screen(&world, "the pin marker to appear", |screen| marked(screen));
    assert!(pins(&world).contains("victim"), "{}", pins(&world));

    world.keys(&["p"]);
    wait_screen(&world, "the pin marker to go", |screen| !marked(screen));
    assert!(pins(&world).contains("no pin"), "{}", pins(&world));

    world.keys(&["u"]);
    wait_screen(&world, "the pin marker to return after undo", |screen| {
        marked(screen)
    });
    assert!(pins(&world).contains("victim"), "{}", pins(&world));

    // Nothing more to undo once the pin and its removal are both undone.
    world.keys(&["u"]);
    wait_screen(
        &world,
        "the pin to be removed by the second undo",
        |screen| !marked(screen),
    );
    world.keys(&["u"]);
    wait_screen(&world, "the empty-stack message", |screen| {
        screen.contains("nothing to undo")
    });
}

/// Two Claude Code sessions in the home the TUI reads: the first mentions
/// "quokka" twice in messages, the second only chats.
fn write_search_sessions(world: &World) {
    let dir = world.root.join("home/.claude/projects/-home-user-project");
    fs::create_dir_all(&dir).expect("transcript dir");
    let record = |key: &str, kind: &str, uuid: &str, parent: &str, time: &str, body: &str| {
        format!(
            r#"{{"cwd":"/home/user/project","sessionId":"{key}","type":"{kind}","uuid":"{uuid}","parentUuid":{parent},"timestamp":"{time}","message":{{"role":"{kind}","content":[{{"type":"text","text":"{body}"}}]}}}}"#
        )
    };
    let alpha = "aaaaaaaa-0000-0000-0000-000000000001";
    let beta = "bbbbbbbb-0000-0000-0000-000000000002";
    let first = [
        record(
            alpha,
            "user",
            "a1",
            "null",
            "2026-10-01T10:00:00.000Z",
            "We chose the quokka design",
        ),
        record(
            alpha,
            "assistant",
            "a2",
            "\"a1\"",
            "2026-10-01T10:00:05.000Z",
            "The Quokka layout is noted.",
        ),
    ];
    let second = [
        record(
            beta,
            "user",
            "b1",
            "null",
            "2026-10-02T09:00:00.000Z",
            "Unrelated chatter",
        ),
        record(
            beta,
            "assistant",
            "b2",
            "\"b1\"",
            "2026-10-02T09:00:03.000Z",
            "Nothing to see",
        ),
    ];
    fs::write(dir.join(format!("{alpha}.jsonl")), first.join("\n") + "\n").unwrap();
    fs::write(dir.join(format!("{beta}.jsonl")), second.join("\n") + "\n").unwrap();
}

/// `/`, a query, Enter: quick select picks a row of the current view and
/// the right pane follows it.
#[test]
fn quick_select_picks_a_row_of_the_current_view() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "/"]);
    wait_screen(&world, "quick select on Current", |screen| {
        screen.contains("/ quick select") && screen.contains("[ Current ] All")
    });
    world.keys(&["bbbbbbbb"]);
    wait_screen(&world, "the match", |screen| screen.contains("(1)"));
    world.keys(&["Enter"]);
    wait_screen(
        &world,
        "the overlay closed on the picked session",
        |screen| {
            !screen.contains("quick select")
                && screen.contains("bbbbbbbb-0000-0000-0000-000000000002")
        },
    );
}

/// `/`, a query, then Down five times: the cursor follows every press
/// instead of stopping after the first.
#[test]
fn quick_select_cursor_follows_repeated_arrow_keys() {
    needs_tmux!();
    let world = World::new();
    for n in 1..=7 {
        world.new_mux(&format!("pager-{n}"), false);
    }
    world.start_tui_outside();
    wait_screen(&world, "the muxes", |screen| screen.contains("7 mux"));

    world.keys(&["2", "/"]);
    wait_screen(&world, "quick select", |screen| {
        screen.contains("/ quick select")
    });
    world.keys(&["pager"]);
    wait_screen(&world, "all seven matches", |screen| screen.contains("(7)"));
    world.keys(&["Down", "Down", "Down", "Down", "Down"]);
    // The list's cursor mark, inside the modal's frame.
    let marked = |line: &&str| line.contains("│► ") && line.contains("mux:pager-");
    wait_screen(&world, "the marker on the sixth match", |screen| {
        screen.lines().any(|line| marked(&line))
            && screen
                .lines()
                .filter(marked)
                .all(|line| !line.contains("mux:pager-1 ") && !line.contains("mux:pager-2 "))
    });
}

/// `/`, Tab, a name, Enter: the wider reach finds a mux while the
/// Sessions view is showing, and opening it switches to the Mux view.
#[test]
fn quick_select_widened_with_tab_opens_an_entity_from_another_view() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "/", "Tab"]);
    wait_screen(&world, "quick select on All", |screen| {
        screen.contains("Current [ All ]")
    });
    world.keys(&["anchor"]);
    wait_screen(&world, "the mux with its outcome word", |screen| {
        screen.contains("anchor") && screen.contains("in Mux view")
    });
    world.keys(&["Enter"]);
    wait_screen(&world, "the Mux view listing it", |screen| {
        !screen.contains("quick select") && row(screen, "anchor").is_some()
    });
}

/// `s`, a query: the overlay streams the match; Ctrl-F keeps it as
/// the filter, which narrows the list and shows in the header; `F`
/// clears it.
#[test]
fn the_search_overlay_filters_the_session_list_and_f_clears_it() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    // The TUI opens on the muxes; the content filter is for the sessions.
    world.keys(&["1"]);
    world.keys(&["s"]);
    wait_screen(&world, "the contents scope", |screen| {
        screen.contains("/ search contents")
    });
    world.keys(&["quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("/ search contents")
            && screen.contains("Current [ All ]")
            && screen.contains("1 result")
            && screen.contains("2 hits")
    });

    world.keys(&["C-f"]);
    wait_screen(&world, "the list narrowed with the find chip", |screen| {
        screen.contains("1/2 sessions") && !screen.contains("/ search contents")
    });

    world.keys(&["F"]);
    wait_screen(&world, "the filter cleared", |screen| {
        screen.contains("2 sessions") && !screen.contains("1/2 sessions")
    });
}

/// Esc in the contents scope cancels the scan and leaves no filter.
#[test]
fn escaping_the_contents_scope_leaves_the_lists_whole() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the result", |screen| screen.contains("2 hits"));
    world.keys(&["Escape"]);
    wait_screen(&world, "the overlay closed", |screen| {
        !screen.contains("/ search contents")
    });
    let screen = world.screen();
    assert!(!screen.contains("find:"), "{screen}");
    assert!(screen.contains("2 sessions"), "{screen}");
}

/// `s`, a query, Enter, then `q`: the viewer closes at once, with the
/// find still active (only `Esc` steps back through the find first).
#[test]
fn q_leaves_a_viewer_opened_from_a_search_at_once() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["Enter"]);
    wait_screen(&world, "the viewer with its find", |screen| {
        screen.contains("find \"quokka\" 1/2")
    });
    world.keys(&["q"]);
    wait_screen(&world, "back at the lists after one q", |screen| {
        !screen.contains("find \"quokka\"") && screen.contains("2 sessions")
    });
}

/// `s`, a query, Enter: the viewer opens on the match, `n` steps to
/// the next one, and `q` returns to the lists.
#[test]
fn a_search_result_opens_the_viewer_on_its_match_and_n_steps_through() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["Enter"]);
    wait_screen(&world, "the viewer on the first match", |screen| {
        screen.contains("find \"quokka\" 1/2")
    });
    world.keys(&["n"]);
    wait_screen(&world, "the second match", |screen| {
        screen.contains("find \"quokka\" 2/2")
    });
    // The first Esc clears the find; the next leaves the viewer.
    world.keys(&["Escape"]);
    wait_screen(&world, "the find cleared", |screen| {
        !screen.contains("find \"quokka\"")
    });
    world.keys(&["q"]);
    wait_screen(&world, "back at the lists", |screen| {
        screen.contains("2 sessions") && !screen.contains("claude-code:aaaaaaaa")
    });
}

/// Promoting a per-message query into the Search view lists a row per
/// matching turn, and Enter on one opens the viewer at that turn.
#[test]
fn the_search_view_lists_hits_and_enter_opens_the_viewer_at_one() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view with a row per session", |screen| {
        screen.contains("2 hits") && !screen.contains("turn 1")
    });
    // The cursor is on the session: Enter opens its first hit.
    world.keys(&["Enter"]);
    wait_screen(&world, "the viewer on a match", |screen| {
        screen.contains("find \"quokka\"")
    });
}

/// Alt+Enter on a Search-view session reveals it in the Sessions view,
/// and Backspace on the left pane returns to the Search view.
#[test]
fn alt_enter_reveals_a_search_result_and_backspace_returns() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view", |screen| {
        screen.contains("2 hits") && !screen.contains("turn 1")
    });

    world.keys(&["M-Enter"]);
    wait_screen(&world, "the Sessions view with a way back", |screen| {
        !screen.contains("2 hits") && screen.contains("Backspace back")
    });

    world.keys(&["BSpace"]);
    wait_screen(&world, "the Search view again", |screen| {
        screen.contains("2 hits") && !screen.contains("Backspace back")
    });
}

/// The flat mode lists the matching messages with no session row above
/// them, and Enter on one opens the viewer at that message.
#[test]
fn the_flat_search_view_lists_messages_and_enter_opens_the_viewer() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["BTab", "C-s"]);
    wait_screen(&world, "the flat Search view", |screen| {
        screen.contains("turn 1") && screen.contains("turn 2")
    });
    // The cursor starts on a message, not on a session.
    world.keys(&["Enter"]);
    wait_screen(&world, "the viewer on a match", |screen| {
        screen.contains("find \"quokka\"")
    });
}

/// The Search view keeps showing its search while the modal types a
/// new one; the new search joins the history when the modal closes.
#[test]
fn the_search_view_stays_fixed_while_the_modal_types_and_takes_the_search_on_exit() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view", |screen| {
        screen.contains("search 1/1 \"quokka\"")
    });

    world.keys(&[
        "s", "BSpace", "BSpace", "BSpace", "BSpace", "BSpace", "BSpace",
    ]);
    world.keys(&["chatter"]);
    wait_screen(&world, "the new search's result in the modal", |screen| {
        screen.contains("/ search contents") && screen.contains("1 result")
    });
    let screen = world.screen();
    assert!(
        screen.contains("search 1/1 \"quokka\"") && !screen.contains("search 2/2"),
        "the view moved while the modal was open:\n{screen}"
    );

    world.keys(&["Escape"]);
    wait_screen(&world, "the new search in the view", |screen| {
        screen.contains("search 2/2 \"chatter\"") && !screen.contains("/ search contents")
    });
}

/// `Ctrl-L` in the Search view lists the shown search's messages, and
/// again its sessions, without searching again.
#[test]
fn the_search_view_switches_between_sessions_and_messages() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "a row per session", |screen| {
        screen.contains("search 1/1") && screen.contains("2 hits") && !screen.contains("turn 1")
    });
    world.keys(&["C-l"]);
    wait_screen(&world, "a row per message", |screen| {
        screen.contains("turn 1") && screen.contains("turn 2")
    });
    world.keys(&["C-l"]);
    wait_screen(&world, "a row per session again", |screen| {
        screen.contains("2 hits") && !screen.contains("turn 1")
    });
    assert!(
        world.screen().contains("search 1/1"),
        "switching is not a new search"
    );
}

/// What a search looks in lives behind `Ctrl-O`, not the key list.
#[test]
fn the_search_options_pane_opens_and_goes_back() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s"]);
    wait_screen(&world, "the modal", |screen| {
        screen.contains("/ search contents") && screen.contains("Ctrl-O options")
    });
    assert!(
        !world.screen().contains("Ctrl-T") && !world.screen().contains("Ctrl-Y"),
        "the option keys are retired"
    );
    world.keys(&["C-o"]);
    wait_screen(&world, "the options pane", |screen| {
        screen.contains("[ ] Tool calls and output") && screen.contains("[x] Pane scrollback")
    });
    world.keys(&["Space"]);
    wait_screen(&world, "tool calls on", |screen| {
        screen.contains("[x] Tool calls and output")
    });
    world.keys(&["Escape"]);
    wait_screen(&world, "back at the results", |screen| {
        screen.contains("/ search contents") && !screen.contains("Pane scrollback")
    });
}

/// Ctrl-/ reaches the TUI through a real terminal as whichever key the
/// terminal sends for it (tmux sends the byte 0x1F for both names), and
/// opens the contents modal on the current view.
#[test]
fn ctrl_slash_opens_the_contents_modal_through_a_real_terminal() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });
    for key in ["C-/", "C-_"] {
        world.keys(&["1", key]);
        wait_screen(&world, "the contents modal", |screen| {
            screen.contains("/ search contents") && screen.contains("Current [ All ]")
        });
        world.keys(&["Escape"]);
        wait_screen(&world, "the modal closed", |screen| {
            !screen.contains("/ search contents")
        });
    }
}

/// Ctrl-S is not swallowed as terminal flow control: it keeps the search
/// and opens the Search view, and `<` and `>` then page between the
/// searches without scanning again.
#[test]
fn ctrl_s_keeps_the_search_and_the_search_view_pages_between_searches() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });

    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view on the first search", |screen| {
        screen.contains("search 1/1 \"quokka\"") && screen.contains("2 hits")
    });

    // A second search starts from a clean slate, not from the first.
    world.keys(&["s", "chatter"]);
    wait_screen(&world, "the second search's result", |screen| {
        screen.contains("/chatter")
            && !screen.contains("/quokkachatter")
            && screen.contains("1 hit")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view on the second search", |screen| {
        screen.contains("search 2/2 \"chatter\"")
    });

    world.keys(&["<"]);
    wait_screen(&world, "the first search again", |screen| {
        screen.contains("search 1/2 \"quokka\"") && screen.contains("2 hits")
    });
    world.keys(&[">"]);
    wait_screen(&world, "the second search again", |screen| {
        screen.contains("search 2/2 \"chatter\"")
    });

    // Reopening starts empty; Ctrl-R brings the earlier strings back,
    // newest first, and Shift-Tab switches to messages.
    world.keys(&["s"]);
    wait_screen(&world, "the empty modal", |screen| {
        screen.contains("search contents")
            && !screen.contains("/chatter")
            && !screen.contains("/quokka")
    });
    world.keys(&["C-r"]);
    wait_screen(&world, "the newest string", |screen| {
        screen.contains("/chatter")
    });
    world.keys(&["C-r"]);
    wait_screen(&world, "the older string", |screen| {
        screen.contains("/quokka")
    });
    world.keys(&["BTab"]);
    wait_screen(&world, "the messages tab", |screen| {
        screen.contains("Sessions [ Messages ]")
    });
    world.keys(&["Escape"]);
    wait_screen(&world, "the Search view without a new entry", |screen| {
        screen.contains("search 2/2")
    });
}

/// Content search reads what a pane shows. The Mux view's current
/// context holds the shell's mux, Enter on its row opens the scrollback
/// at the line, and the Search view lists the pane as a row.
#[test]
fn content_search_finds_text_in_a_panes_scrollback() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.new_mux("shell", true);
    assert!(
        world
            .inner(&[
                "send-keys",
                "-t",
                "shell",
                "echo zanzibar-in-the-pane",
                "Enter"
            ])
            .status
            .success()
    );
    world.start_tui_outside();
    wait_screen(&world, "the muxes", |screen| row(screen, "shell").is_some());

    world.keys(&["C-/", "zanzibar"]);
    wait_screen(&world, "the pane's match", |screen| {
        screen.contains("/ search contents")
            && screen
                .lines()
                .any(|line| line.contains("pane") && line.contains("shell 0.0"))
    });
    world.keys(&["Enter"]);
    wait_screen(&world, "the scrollback pager at the line", |screen| {
        screen.contains("shell 0.0 scrollback") && screen.contains("zanzibar-in-the-pane")
    });
    // `q` leaves the viewer at once, find or no find.
    world.keys(&["q"]);
    wait_screen(&world, "the pager closed", |screen| {
        !screen.contains("shell 0.0 scrollback")
    });

    world.keys(&["C-/", "zanzibar", "C-s"]);
    wait_screen(&world, "the pane as a row of the Search view", |screen| {
        screen.contains("search 1/1 \"zanzibar\"")
            && screen
                .lines()
                .any(|line| line.contains("shell") && line.contains("0.0") && line.contains("hit"))
    });
}

/// The pane viewer is full screen and `a` in it attaches to the pane's
/// mux: exiting the shell there ends the mux.
#[test]
fn the_pane_viewer_is_full_screen_and_a_attaches() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.new_mux("shell", true);
    assert!(
        world
            .inner(&[
                "send-keys",
                "-t",
                "shell",
                "echo zanzibar-in-the-pane",
                "Enter"
            ])
            .status
            .success()
    );
    world.start_tui_outside();
    wait_screen(&world, "the muxes", |screen| row(screen, "shell").is_some());

    world.keys(&["C-/", "zanzibar"]);
    wait_screen(&world, "the pane's match", |screen| {
        screen
            .lines()
            .any(|line| line.contains("pane") && line.contains("shell 0.0"))
    });
    world.keys(&["Enter"]);
    wait_screen(
        &world,
        "the full-screen viewer with an attach chip",
        |screen| {
            screen.contains("shell 0.0 scrollback")
                && screen.contains("(a) attach")
                && !screen.contains("search contents")
        },
    );

    world.keys(&["a"]);
    sleep(Duration::from_millis(1500));
    world.keys(&["exit", "Enter"]);
    wait_until("the attached mux ends", Duration::from_secs(10), || {
        !world.mux_alive("shell")
    });
}

/// `F` and quitting while a scan may still be reading must not hang.
#[test]
fn quitting_with_a_content_search_running_exits_promptly() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside_with("--contains quokka");
    // Quit at once, while the startup scan may still be running.
    world.keys(&["q"]);
    wait_until("the TUI to exit", Duration::from_secs(5), || {
        !world.screen().contains("sessions · mux")
    });
}

/// With live preview off the content filter is refused, and says why.
#[test]
fn live_preview_off_refuses_a_startup_content_filter() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside_with("--no-live-preview --contains quokka");
    wait_screen(&world, "the refusal", |screen| {
        screen.contains("live preview")
    });
    let screen = world.screen();
    assert!(
        !screen.contains("find:\""),
        "no filter is applied: {screen}"
    );
}

/// The whole overlay flow leaves no transcript text in the directories
/// Conspectus writes to (the transcripts themselves live under HOME).
#[test]
fn a_tui_content_search_writes_no_transcript_text_to_disk() {
    needs_tmux!();
    let world = World::new();
    write_search_sessions(&world);
    world.new_mux("anchor", false);
    world.start_tui_outside();
    wait_screen(&world, "both sessions", |screen| {
        screen.contains("2 sessions")
    });
    world.keys(&["1", "s", "quokka"]);
    wait_screen(&world, "the streamed result", |screen| {
        screen.contains("2 hits")
    });
    world.keys(&["C-s"]);
    wait_screen(&world, "the Search view", |screen| {
        screen.contains("search 1/1") && screen.contains("2 hits")
    });
    world.keys(&["F"]);
    // Let any pending write land.
    sleep(Duration::from_secs(1));

    // The first message is only ever in the transcript and in search
    // snippets: discovery's own preview shows the last message.
    let needle = "chose the quokka";
    for dir in ["xdg/s", "xdg/k", "xdg/d", "xdg/c"] {
        let mut stack = vec![world.root.join(dir)];
        while let Some(path) = stack.pop() {
            let Ok(entries) = fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(bytes) = fs::read(&path) {
                    assert!(
                        !bytes
                            .windows(needle.len())
                            .any(|window| window == needle.as_bytes()),
                        "search text reached {}",
                        path.display()
                    );
                }
            }
        }
    }
}
