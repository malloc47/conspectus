//! The real-session recording driver (`scripts/record/`, ADR 0244) spends the
//! model quota of the account a harness is signed in to, and once took a
//! whole session down with a runaway pty tool. These tests keep it manual
//! and bounded; none of them starts a harness.
//!
//! - every script parses;
//! - no script gives a pty tool, or anything, an unbounded input;
//! - nothing automatic (`just check`, a workflow, another recipe) runs it;
//! - a dry run prints the quota and the directories and creates nothing;
//! - a run with no confirmation refuses before it creates anything.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn driver() -> PathBuf {
    root().join("scripts/record")
}

/// The files of the driver with the given extension.
fn files(extension: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(driver())
        .expect("scripts/record")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    found.sort();
    assert!(!found.is_empty(), "no .{extension} files in scripts/record");
    found
}

#[test]
fn every_script_parses() {
    for script in files("sh") {
        let status = Command::new("bash")
            .arg("-n")
            .arg(&script)
            .status()
            .expect("start bash");
        assert!(status.success(), "{} has a syntax error", script.display());
    }
    for script in files("py") {
        let status = Command::new("python3")
            .args([
                "-I",
                "-c",
                "import ast, sys; ast.parse(open(sys.argv[1]).read())",
            ])
            .arg(&script)
            .status()
            .expect("start python3");
        assert!(status.success(), "{} has a syntax error", script.display());
    }
}

/// The code of a script: its lines without the comments.
fn code(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .expect("read script")
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The first word of each command a shell line runs: the line split where a
/// pipe, a `;`, an `&`, a parenthesis, or a backquote starts another.
fn commands(line: &str) -> Vec<String> {
    line.split(|c| matches!(c, '|' | ';' | '&' | '(' | ')' | '`'))
        .filter_map(|part| part.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[test]
fn no_script_gives_a_pty_tool_an_unbounded_input() {
    // `script` reading /dev/zero grew to 7 to 9 GB in seconds and took the
    // session that started it down with it.
    let forbidden = ["/dev/zero", "/dev/urandom", "/dev/full"];
    for path in files("sh").into_iter().chain(files("py")) {
        for line in code(&path) {
            for device in forbidden {
                assert!(
                    !line.contains(device),
                    "{} reads {device}: {line}",
                    path.display()
                );
            }
            for command in commands(&line) {
                assert!(
                    command != "yes" && command != "script",
                    "{} runs `{command}`: {line}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn run_sh_bounds_every_run() {
    let run = fs::read_to_string(driver().join("run.sh")).expect("run.sh");
    for needle in [
        "systemd-run --user --scope",
        "MemoryMax=",
        "MemorySwapMax=0",
        "OOMPolicy=continue",
        "timeout \"$secs\"",
    ] {
        assert!(run.contains(needle), "run.sh lacks {needle}");
    }
}

#[test]
fn nothing_automatic_starts_the_driver() {
    let justfile = fs::read_to_string(root().join("justfile")).expect("justfile");
    let mut recipe = String::new();
    for line in justfile.lines() {
        if !line.starts_with(' ') && !line.starts_with('#') && line.contains(':') {
            recipe = line
                .split(':')
                .next()
                .and_then(|head| head.split_whitespace().next())
                .unwrap_or("")
                .to_string();
            // `check` and every recipe it names must not depend on `record`.
            if recipe == "check" || recipe == "default" {
                assert!(
                    !line.split_whitespace().any(|word| word == "record"),
                    "{line}"
                );
            }
        }
        if recipe != "record" && !line.trim_start().starts_with('#') {
            assert!(
                !line.contains("scripts/record") && !line.contains("just record"),
                "the `{recipe}` recipe starts the recording driver: {line}"
            );
        }
    }
    let workflows = root().join(".github/workflows");
    if let Ok(entries) = fs::read_dir(&workflows) {
        for entry in entries.flatten() {
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            assert!(
                !text.contains("scripts/record") && !text.contains("just record"),
                "{} runs the recording driver",
                entry.path().display()
            );
        }
    }
}

/// `run.sh` with the given arguments, an empty scratch area, and no
/// terminal on stdin.
fn run(args: &[&str]) -> (std::process::Output, PathBuf) {
    let scratch = std::env::temp_dir().join(format!(
        "record-driver-test-{}-{}",
        std::process::id(),
        args.join("-")
    ));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("scratch");
    let output = Command::new("bash")
        .arg(driver().join("run.sh"))
        .args(args)
        .env("RECORD_TMP", &scratch)
        .env("RECORD_CONSPECTUS", "/nonexistent/conspectus")
        .current_dir(&scratch)
        .stdin(Stdio::null())
        .output()
        .expect("start run.sh");
    (output, scratch)
}

/// What a run left in the scratch area: the `conspectus-record.*` directories
/// a real run creates, and the output directory.
fn created(scratch: &Path) -> Vec<String> {
    fs::read_dir(scratch)
        .expect("scratch")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn a_dry_run_prints_the_quota_and_the_directories_and_creates_nothing() {
    for harness in ["claude-code", "codex", "pi"] {
        let (output, scratch) = run(&["--dry-run", harness]);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{harness}: {text}");
        for needle in [
            "quota",
            "short prompts",
            "creates",
            "conspectus-record.",
            "writes",
            "bounded",
        ] {
            assert!(text.contains(needle), "{harness}: no {needle:?} in\n{text}");
        }
        assert!(
            created(&scratch).is_empty(),
            "{harness}: {:?}",
            created(&scratch)
        );
        let _ = fs::remove_dir_all(scratch);
    }
}

#[test]
fn without_a_yes_or_a_terminal_a_run_refuses_before_it_creates_anything() {
    let (output, scratch) = run(&["pi"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("--yes") && text.contains("quota"), "{text}");
    assert!(created(&scratch).is_empty(), "{:?}", created(&scratch));
    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn an_unknown_harness_or_option_is_refused() {
    for args in [vec!["opencode"], vec![], vec!["--bogus", "pi"]] {
        let (output, scratch) = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(created(&scratch).is_empty());
        let _ = fs::remove_dir_all(scratch);
    }
}
