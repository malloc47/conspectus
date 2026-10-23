#![cfg(all(feature = "snapshot", debug_assertions))]
//! The example frames in `docs/tui-columns.md` match what the TUI draws.
//!
//! Each frame is a fenced block under a marker comment:
//!
//! ```text
//! <!-- frame: keys="4" width=170 height=30 -->
//! ```
//!
//! The test renders the left pane of the showcase world with those keys
//! and compares. After a deliberate change to the TUI, regenerate the
//! blocks with `just tui-columns` (`UPDATE_TUI_COLUMNS=1`).

use std::path::{Path, PathBuf};

use assert_cmd::Command;

/// `SHOWCASE_NOW_EPOCH` in `src/dev_scenarios.rs`.
const SHOWCASE_NOW: &str = "1790769600";

const DOC: &str = "docs/tui-columns.md";

fn doc_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DOC)
}

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

/// A frame's text as the document stores it: trailing spaces trimmed,
/// empty rows at the bottom of the pane (and its bottom border) dropped.
fn normalize(frame: &str) -> String {
    let mut lines: Vec<String> = frame
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    // The bottom border.
    if lines
        .last()
        .is_some_and(|line| line.starts_with('└') && line.ends_with('┘'))
    {
        lines.pop();
    }
    let is_blank_row = |line: &String| {
        let inner = line
            .trim_start_matches('│')
            .trim_end_matches('│')
            .trim_matches(' ');
        line.starts_with('│') && inner.is_empty()
    };
    while lines.last().is_some_and(is_blank_row) {
        lines.pop();
    }
    lines.join("\n")
}

fn render(scratch: &Path, keys: &str, width: u32, height: u32) -> String {
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
            &width.to_string(),
            "--snapshot-height",
            &height.to_string(),
            "--snapshot-pane",
            "left",
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
    normalize(&strip_ansi(&String::from_utf8_lossy(&output.stdout)))
}

/// One marker and the fenced block under it.
struct Frame {
    keys: String,
    width: u32,
    height: u32,
    /// Line index of the first line inside the fence.
    body_start: usize,
    /// Line index of the closing fence.
    body_end: usize,
}

fn attribute(marker: &str, name: &str) -> Option<String> {
    let rest = marker.split(&format!("{name}=")).nth(1)?;
    if let Some(quoted) = rest.strip_prefix('"') {
        return Some(quoted.split('"').next()?.to_string());
    }
    Some(
        rest.split_whitespace()
            .next()?
            .trim_end_matches("-->")
            .to_string(),
    )
}

/// The index of the first fence line at or after `from`.
fn fence_after(lines: &[&str], from: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, line)| line.starts_with("```"))
        .map(|(index, _)| index)
}

fn frames_in(lines: &[&str]) -> Vec<Frame> {
    let mut frames = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim_start().starts_with("<!-- frame:") {
            let keys = attribute(line, "keys").unwrap_or_default();
            let width = attribute(line, "width")
                .and_then(|w| w.parse().ok())
                .unwrap_or(170);
            let height = attribute(line, "height")
                .and_then(|h| h.parse().ok())
                .unwrap_or(30);
            let open = fence_after(lines, index + 1).unwrap_or_else(|| {
                panic!("{DOC}:{}: a frame marker needs a fenced block", index + 1)
            });
            let close = fence_after(lines, open + 1)
                .unwrap_or_else(|| panic!("{DOC}:{}: unterminated fenced block", open + 1));
            frames.push(Frame {
                keys,
                width,
                height,
                body_start: open + 1,
                body_end: close,
            });
            index = close + 1;
        } else {
            index += 1;
        }
    }
    frames
}

#[test]
fn the_example_frames_match_what_the_tui_draws() {
    let text = std::fs::read_to_string(doc_path()).expect("read the reference");
    let lines: Vec<&str> = text.lines().collect();
    let frames = frames_in(&lines);
    assert!(
        frames.len() >= 10,
        "the reference embeds frames for every view; found {}",
        frames.len()
    );

    let scratch = tempfile::tempdir().expect("scratch");
    conspectus(scratch.path())
        .args(["dev", "scenario", "materialize", "showcase"])
        .arg(scratch.path().join("world"))
        .assert()
        .success();

    let update = std::env::var_os("UPDATE_TUI_COLUMNS").is_some();
    let mut rewritten: Vec<String> = Vec::new();
    let mut cursor = 0;
    let mut mismatches = Vec::new();
    for frame in &frames {
        let fresh = render(scratch.path(), &frame.keys, frame.width, frame.height);
        let current = lines[frame.body_start..frame.body_end].join("\n");
        if fresh != current {
            let diff: Vec<String> = fresh
                .lines()
                .zip(current.lines().chain(std::iter::repeat("")))
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(4)
                .map(|(row, (a, b))| format!("  row {row}:\n    doc:  {b}\n    tui:  {a}"))
                .collect();
            mismatches.push(format!(
                "frame keys={:?} (doc line {}):\n{}",
                frame.keys,
                frame.body_start,
                diff.join("\n")
            ));
        }
        rewritten.extend(
            lines[cursor..frame.body_start]
                .iter()
                .map(|l| (*l).to_string()),
        );
        rewritten.extend(fresh.lines().map(str::to_string));
        cursor = frame.body_end;
    }
    rewritten.extend(lines[cursor..].iter().map(|l| (*l).to_string()));

    if update {
        let mut out = rewritten.join("\n");
        out.push('\n');
        std::fs::write(doc_path(), out).expect("write the reference");
        return;
    }
    assert!(
        mismatches.is_empty(),
        "{DOC} shows frames the TUI no longer draws. If the change is intended, \
         update the document's text and regenerate the frames with \
         `just tui-columns`.\n\n{}",
        mismatches.join("\n\n")
    );
}
