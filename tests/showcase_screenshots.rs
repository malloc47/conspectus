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

    let mux = frame(scratch.path(), "2jj");
    assert!(
        mux.contains("Bash(gh pr create --draft --base main)"),
        "{mux}"
    );
}
