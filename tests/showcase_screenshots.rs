#![cfg(all(feature = "snapshot", debug_assertions))]
//! The README screenshot pipeline (ADR 0113): `dev scenario materialize`
//! writes the showcase world, and `tui --snapshot` renders the showcase
//! fixture with its previews read from that world, `~` standing for
//! `/fixture`, and ages pinned to the showcase's anchor time. Mirrors
//! `just screenshots`, minus the PNG step.

use std::path::Path;

use assert_cmd::Command;

/// `SHOWCASE_NOW_EPOCH` in `src/dev_scenarios.rs`.
const SHOWCASE_NOW: &str = "1790769600";

fn conspectus(scratch: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", "/fixture")
        .env("XDG_CONFIG_HOME", scratch.join("config"))
        .env("XDG_STATE_HOME", scratch.join("state"))
        .env("XDG_DATA_HOME", scratch.join("data"))
        .env("XDG_CACHE_HOME", scratch.join("cache"))
        .env("XDG_RUNTIME_DIR", scratch.join("runtime"))
        .env("CONSPECTUS_DISABLE_TMUX", "1")
        .env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd
}

fn frame(scratch: &Path, keys: &str) -> String {
    frame_with(scratch, keys, &[])
}

fn frame_with(scratch: &Path, keys: &str, extra: &[&str]) -> String {
    let output = conspectus(scratch)
        .args(["tui", "--snapshot", "--snapshot-fixture"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/showcase.json"
        ))
        .arg("--snapshot-fixture-root")
        .arg(scratch.join("world"))
        .args([
            "--snapshot-now",
            SHOWCASE_NOW,
            "--snapshot-width",
            "150",
            "--snapshot-height",
            "34",
            "--snapshot-keys",
            keys,
        ])
        .args(extra)
        .output()
        .expect("run snapshot");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    strip_ansi(&String::from_utf8_lossy(&output.stdout))
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[test]
fn the_showcase_screens_render_real_previews_with_stable_paths_and_ages() {
    let scratch = tempfile::tempdir().expect("scratch");
    conspectus(scratch.path())
        .args(["dev", "scenario", "materialize", "showcase"])
        .arg(scratch.path().join("world"))
        .assert()
        .success();

    // `just screenshots`' key scripts.
    let session = frame(scratch.path(), "jjjjjljjjj");
    assert!(session.contains("~/repos/project"), "{session}");
    assert!(session.contains("active · last 1h ago"), "{session}");
    assert!(
        session.contains("What's on feature/extra that isn't on main yet?"),
        "{session}"
    );

    let related = frame(scratch.path(), "jjjjjljjjj<Tab>jjjjj");
    assert!(related.contains("feature/extra"), "{related}");
    assert!(related.contains(" M notes.md"), "{related}");

    let repo = frame(scratch.path(), "jjjjj");
    assert!(repo.contains("Document the config format"), "{repo}");
    assert!(
        repo.contains("origin  git@github.com:conspectus/project.git"),
        "{repo}"
    );

    let prs = frame(scratch.path(), "3jl");
    assert!(prs.contains("#7 ⌥    2h  ◐"), "{prs}");
    assert!(prs.contains("Release notes for 0.3"), "{prs}");
    assert!(
        prs.contains("checks  2 passed · 1 failed · 1 pending"),
        "{prs}"
    );

    let mux = frame(scratch.path(), "2jj");
    assert!(
        mux.contains("Bash(gh pr create --draft --base main)"),
        "{mux}"
    );

    // The Pinned view: slots of several kinds, live and gone.
    let pinned = frame(scratch.path(), "4");
    for row in [
        "project-main",
        "nightly",
        "parser-session",
        "ledger-worktree",
        "ledger-pr",
    ] {
        assert!(pinned.contains(row), "{row}: {pinned}");
    }
    assert!(
        pinned.contains("stopped") && pinned.contains("removed"),
        "{pinned}"
    );

    // The History view: gone records no pin points at.
    let history = frame(scratch.path(), "5");
    for row in [
        "release-prep",
        "bugfix-sprint",
        "docs-sweep",
        "hotfix/login",
        "spike/cache",
    ] {
        assert!(history.contains(row), "{row}: {history}");
    }
    for pinned in ["nightly-refactor", "feature/ledger"] {
        assert!(
            !history.contains(pinned),
            "{pinned} is pinned, so History hides it: {history}"
        );
    }
}

#[test]
fn the_viewer_finds_text_and_shows_the_position() {
    let scratch = tempfile::tempdir().expect("scratch");
    conspectus(scratch.path())
        .args(["dev", "scenario", "materialize", "showcase"])
        .arg(scratch.path().join("world"))
        .assert()
        .success();

    // Open the selected session's transcript, then `/feature` and Enter.
    let found = frame(scratch.path(), "jjjjjljjjjv/feature<Enter>");
    assert!(found.contains("find \"feature\" 1/"), "{found}");

    // The prompt is open until Enter: its text and the key hint show.
    let typing = frame(scratch.path(), "jjjjjljjjjv/feat");
    let last = typing
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    assert!(last.contains("/feat"), "{typing}");
    assert!(last.contains("Enter find"), "{typing}");

    // A query that is not there says so.
    let none = frame(scratch.path(), "jjjjjljjjjv/zebra<Enter>");
    assert!(none.contains("find \"zebra\" no match"), "{none}");
}

/// A content query narrows the session list to the sessions whose
/// transcripts mention it, shows how many hits each has with the best
/// snippet, and says what it is looking for in the header.
#[test]
fn a_content_query_narrows_the_showcase_sessions_and_marks_their_hits() {
    let scratch = tempfile::tempdir().expect("scratch");
    conspectus(scratch.path())
        .args(["dev", "scenario", "materialize", "showcase"])
        .arg(scratch.path().join("world"))
        .assert()
        .success();

    let plain = frame(scratch.path(), "");
    let filtered = frame_with(scratch.path(), "l", &["--contains", "release summary"]);

    assert!(
        filtered.contains("find:\"release summary\" 1 found"),
        "{filtered}"
    );
    assert!(filtered.contains("2 hits"), "{filtered}");
    assert!(filtered.contains("1/11 sessions"), "{filtered}");
    assert!(filtered.contains("Expand the notes into"), "{filtered}");
    assert!(plain.contains("11 sessions"), "{plain}");

    // A query nothing matches says so.
    let none = frame_with(scratch.path(), "", &["--contains", "zzz-no-such-text"]);
    assert!(
        none.contains("No sessions match \"zzz-no-such-text\""),
        "{none}"
    );
}

/// The `/` overlay searches contents after Tab, streams the matches,
/// and keeps the query as the filter or opens the Search view.
#[test]
fn the_search_overlay_searches_contents_and_promotes_the_query() {
    let scratch = tempfile::tempdir().expect("scratch");
    conspectus(scratch.path())
        .args(["dev", "scenario", "materialize", "showcase"])
        .arg(scratch.path().join("world"))
        .assert()
        .success();

    // Rows scope is what `/` always was: Tab is the only way across.
    let rows = frame(scratch.path(), "/");
    assert!(rows.contains("/ search ("), "{rows}");
    assert!(!rows.contains("search contents"), "{rows}");

    let contents = frame(scratch.path(), "/<Tab>release summary");
    assert!(
        contents.contains("search contents (1 results)"),
        "{contents}"
    );
    assert!(contents.contains("2 hits"), "{contents}");
    assert!(contents.contains("Ctrl-F filter lists"), "{contents}");
    assert!(contents.contains("sessions searched"), "{contents}");

    // Esc leaves everything as it was: no filter, no chip.
    let closed = frame(scratch.path(), "/<Tab>release summary<Esc>");
    assert!(!closed.contains("find:"), "{closed}");
    assert!(closed.contains("11 sessions"), "{closed}");

    // Ctrl-F keeps the query as the filter on the lists.
    let filtered = frame(scratch.path(), "/<Tab>release summary<C-f>");
    assert!(
        filtered.contains("find:\"release summary\" 1 found"),
        "{filtered}"
    );
    assert!(filtered.contains("1/11 sessions"), "{filtered}");
    assert!(!filtered.contains("search contents"), "closes the overlay");

    // Ctrl-O keeps it and opens the Search view.
    let view = frame(scratch.path(), "/<Tab>release summary<C-o>");
    assert!(view.contains("find:\"release summary\""), "{view}");
    assert!(view.contains("2 hits"), "{view}");
    assert!(view.contains("search"), "{view}");
}
