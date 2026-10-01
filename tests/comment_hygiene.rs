//! Source comments explain behavior and rationale; backlog IDs belong
//! in commit messages and `docs/backlog.md` (ADR 0100). A comment may
//! still point at an open backlog item when it says so, e.g.
//! "open work (backlog `T8-009`)".

use std::fs;
use std::path::{Path, PathBuf};

/// Backlog IDs such as `P7-003`, `T8-043a`, `H-SERVE-PERF-001a`.
fn backlog_ids(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|token| {
            let token = token
                .strip_suffix(|c: char| c.is_ascii_lowercase())
                .unwrap_or(token);
            let Some((prefix, number)) = token.rsplit_once('-') else {
                return false;
            };
            if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            let phase = prefix.len() > 1
                && matches!(prefix.as_bytes()[0], b'P' | b'F' | b'T')
                && prefix[1..].chars().all(|c| c.is_ascii_digit());
            let workstream = prefix.strip_prefix("H-").is_some_and(|rest| {
                !rest.is_empty()
                    && rest.split('-').all(|part| {
                        !part.is_empty()
                            && part
                                .chars()
                                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                    })
            });
            phase || workstream
        })
        .collect()
}

fn comment(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return Some(trimmed);
    }
    // Trailing comment after code. Good enough for this codebase: no
    // string literal in it contains `// ` followed by a backlog id.
    line.find(" // ").map(|index| &line[index..])
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn comments_do_not_cite_backlog_ids_as_history() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "tests", "examples"] {
        rust_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut problems = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            let Some(comment) = comment(line) else {
                continue;
            };
            if comment.to_ascii_lowercase().contains("backlog") {
                continue;
            }
            for id in backlog_ids(comment) {
                let relative = path.strip_prefix(root).unwrap_or(path);
                problems.push(format!("{}:{}: `{id}`", relative.display(), number + 1));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "comments cite backlog IDs; describe the behavior instead, or say \
         \"backlog `ID`\" when pointing at open work:\n{}",
        problems.join("\n")
    );
}

#[test]
fn backlog_id_matcher_recognizes_the_id_shapes() {
    assert_eq!(
        backlog_ids("see P7-003, F8-013, T8-043a, H-WT-008 and H-SERVE-PERF-001a"),
        [
            "P7-003",
            "F8-013",
            "T8-043a",
            "H-WT-008",
            "H-SERVE-PERF-001a"
        ]
    );
    assert!(backlog_ids("ADR 0083, utf-8, x86-64, P-1, H-2").is_empty());
}
