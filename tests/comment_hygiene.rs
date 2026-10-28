//! Source comments explain behavior and rationale; backlog IDs belong
//! in commit messages and the backlog itself (ADR 0100). A comment may
//! still point at an open backlog item when it says so, e.g.
//! "open work (backlog `CSP-123`)".
//!
//! The rule covers the host and the framework (`crates/tuikit`), in
//! `src`, `tests`, and `examples` of each; `tests/support/source_tree.rs`
//! explains why and how the scans are guarded against visiting too few
//! files.

use std::fs;
use std::path::Path;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::EVERY_RUST_TREE;

/// Backlog IDs such as `CSP-123` and subtasks such as `CSP-123.01`.
/// Legacy backlog IDs (`P7-003`, `T8-043a`, `H-SERVE-PERF-001a`) count too.
fn backlog_ids(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
        .map(|token| token.trim_end_matches('.'))
        .filter(|token| is_task_id(token) || is_legacy_id(token))
        .collect()
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn is_task_id(token: &str) -> bool {
    token.strip_prefix("CSP-").is_some_and(|number| {
        let (id, subtask) = number.split_once('.').unwrap_or((number, "0"));
        digits(id) && digits(subtask)
    })
}

fn is_legacy_id(token: &str) -> bool {
    let token = token
        .strip_suffix(|c: char| c.is_ascii_lowercase())
        .unwrap_or(token);
    let Some((prefix, number)) = token.rsplit_once('-') else {
        return false;
    };
    if !digits(number) {
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

/// Every comment under `trees` that cites a backlog ID without pointing
/// at open work, as `path:line: ID` findings.
fn problems(root: &Path, trees: &[source_tree::Tree]) -> Vec<String> {
    let mut problems = Vec::new();
    for path in &source_tree::rust_files(root, trees) {
        let text = fs::read_to_string(path).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            let Some(comment) = comment(line) else {
                continue;
            };
            if comment.to_ascii_lowercase().contains("backlog") {
                continue;
            }
            for id in backlog_ids(comment) {
                problems.push(format!(
                    "{}:{}: `{id}`",
                    source_tree::relative(root, path),
                    number + 1
                ));
            }
        }
    }
    problems
}

#[test]
fn comments_do_not_cite_backlog_ids_as_history() {
    let problems = problems(source_tree::workspace_root(), EVERY_RUST_TREE);
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
        backlog_ids("see CSP-003, CSP-212.01. Also P7-003, T8-043a and H-SERVE-PERF-001a."),
        [
            "CSP-003",
            "CSP-212.01",
            "P7-003",
            "T8-043a",
            "H-SERVE-PERF-001a"
        ]
    );
    assert!(backlog_ids("ADR 0083, utf-8, x86-64, v0.1.0, P-1, H-2, CSP-, CSP-1.x").is_empty());
}

#[test]
fn a_cited_backlog_id_is_caught_in_every_tree_the_scan_covers() {
    let violation = "// fixed in CSP-123\nfn f() {}\n";
    source_tree::assert_scan_covers_every_tree(EVERY_RUST_TREE, violation, problems);
}
