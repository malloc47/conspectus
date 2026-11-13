#![cfg(feature = "snapshot")]
//! `conspectus tui --snapshot` previews an agent session as its
//! transcript (ADR 0112): a session in tmux, one outside tmux, one whose
//! transcript is gone, and a Related row's neighbor below its evidence.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use support::replay::{ReplayWorld, TmuxReplayRow};
use tuikit::testkit::strip_ansi;

const MUXED: &str = "muxedd-0000-4000-8000-000000000001";
const SOLO: &str = "solooo-0000-4000-8000-000000000002";
const GONE: &str = "goneee-0000-4000-8000-000000000003";

/// Three Claude Code sessions, one in the tmux session `editor`, as a
/// snapshot fixture. Unlike `ReplayWorld::write_snapshot_fixture`, the
/// fixture keeps the real temp paths, so the snapshot reads the
/// transcripts on disk.
struct World {
    _world: ReplayWorld,
    home: tempfile::TempDir,
    fixture: PathBuf,
}

fn world() -> World {
    let mut world = ReplayWorld::new();
    let muxed_cwd = world.mkdir("muxed");
    let solo_cwd = world.mkdir("solo");
    let gone_cwd = world.mkdir("gone");
    let muxed = world.write_claude_code_session(MUXED, &muxed_cwd);
    let solo = world.write_claude_code_session(SOLO, &solo_cwd);
    let gone = world.write_claude_code_session(GONE, &gone_cwd);
    world.add_tmux_row(TmuxReplayRow::new("editor").with_cwd(&muxed_cwd));
    let resolved = world.run().resolved;

    let home = tempfile::tempdir().expect("temp home");
    let fixture = home.path().join("world.json");
    fs::write(
        &fixture,
        serde_json::to_string(&resolved).expect("serialize snapshot"),
    )
    .expect("write fixture");

    // Discovery has read the sessions; now give two of them a
    // conversation and take the third's transcript away.
    fs::write(&muxed, exchange(MUXED, "Where is the pane?", "In tmux.")).expect("muxed");
    fs::write(&solo, exchange(SOLO, "Any tmux here?", "None at all.")).expect("solo");
    fs::remove_file(&gone).expect("remove transcript");

    World {
        _world: world,
        home,
        fixture,
    }
}

/// One user question and its answer, in Claude Code's transcript shape.
fn exchange(session: &str, question: &str, answer: &str) -> String {
    let user = serde_json::json!({
        "type": "user",
        "sessionId": session,
        "uuid": "u1",
        "parentUuid": null,
        "message": {"role": "user", "content": [{"type": "text", "text": question}]},
    });
    let assistant = serde_json::json!({
        "type": "assistant",
        "sessionId": session,
        "uuid": "a1",
        "parentUuid": "u1",
        "message": {"role": "assistant", "content": [{"type": "text", "text": answer}]},
    });
    format!("{user}\n{assistant}\n")
}

/// The right pane with `session` selected, ANSI styling stripped. The
/// three sessions sit in one group, ordered by key.
fn right_pane(world: &World, session: &str) -> String {
    let pane = right_pane_after(world, &select(session));
    assert!(pane.contains(session), "{session} is selected:\n{pane}");
    pane
}

/// Keys that select `session` in the sessions view.
fn select(session: &str) -> String {
    let downs = [GONE, MUXED, SOLO]
        .iter()
        .position(|key| *key == session)
        .expect("a world session")
        + 1;
    "j".repeat(downs)
}

/// The right pane after `keys`, ANSI styling stripped.
fn right_pane_after(world: &World, keys: &str) -> String {
    let home = world.home.path();
    let output = isolated_cmd(home)
        .current_dir(home)
        .args(["tui", "--snapshot", "--snapshot-fixture"])
        .arg(&world.fixture)
        .args([
            "--snapshot-pane",
            "right",
            "--snapshot-width",
            "160",
            "--snapshot-height",
            "40",
            "--snapshot-keys",
            keys,
        ])
        .output()
        .expect("run conspectus");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    strip_ansi(&String::from_utf8_lossy(&output.stdout))
}

/// A `conspectus` command whose config, state, and daemon lookups stay
/// under `home`.
fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_STATE_HOME", home.join(".local").join("state"));
    cmd.env("XDG_DATA_HOME", home.join(".local").join("share"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("TMUX");
    cmd
}

#[test]
fn a_session_in_tmux_previews_its_transcript() {
    let world = world();
    let pane = right_pane(&world, MUXED);
    assert!(pane.contains("tmux:editor"), "linked to its mux:\n{pane}");
    assert!(pane.contains("Where is the pane?"), "{pane}");
    assert!(pane.contains("In tmux."), "{pane}");
}

#[test]
fn a_session_outside_tmux_previews_its_transcript() {
    let world = world();
    let pane = right_pane(&world, SOLO);
    assert!(pane.contains("Any tmux here?"), "{pane}");
    assert!(pane.contains("None at all."), "{pane}");
}

#[test]
fn a_session_whose_transcript_is_gone_says_so() {
    let world = world();
    let pane = right_pane(&world, GONE);
    assert!(
        pane.contains("transcript unavailable: transcript file not found"),
        "{pane}"
    );
}

/// Keys that walk the explorer cursor past a node's five core fields
/// onto its first Related row.
const FIRST_RELATED_ROW: &str = "<Tab>jjjjj";

#[test]
fn a_related_row_to_a_mux_session_leads_its_pane_with_the_evidence() {
    let world = world();
    let pane = right_pane_after(&world, &format!("{}{FIRST_RELATED_ROW}", select(MUXED)));
    let preview = pane.split("Preview").nth(1).expect("a Preview zone");
    assert!(preview.contains("neighbor    tmux:editor"), "{pane}");
    assert!(preview.contains("evidence      exact_cwd_match"), "{pane}");
    // Snapshot mode doesn't capture panes; the pane's place is held.
    assert!(preview.contains("loading mux preview…"), "{pane}");
    assert!(
        !preview.contains("backend"),
        "the pane replaces the neighbor's fields:\n{pane}"
    );
}

#[test]
fn a_related_row_to_a_session_leads_its_transcript_with_the_evidence() {
    let world = world();
    // `2` is the mux view; its one row is `tmux:editor`.
    let pane = right_pane_after(&world, &format!("2j{FIRST_RELATED_ROW}"));
    let preview = pane.split("Preview").nth(1).expect("a Preview zone");
    assert!(
        preview.contains(&format!("neighbor    claude-code:{MUXED}")),
        "{pane}"
    );
    assert!(preview.contains("evidence"), "{pane}");
    assert!(preview.contains("Where is the pane?"), "{pane}");
    assert!(preview.contains("In tmux."), "{pane}");
}
