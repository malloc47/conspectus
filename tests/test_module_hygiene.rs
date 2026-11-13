//! Large unit-test modules live in a sibling file so a source file's
//! production code reads at a glance (ADR 0124). A module of
//! `INLINE_TEST_MODULE_LIMIT` lines or more moves out of line:
//!
//! ```text
//! #[cfg(test)]
//! #[path = "foo_tests.rs"]
//! mod tests;
//! ```
//!
//! The limit applies to the host (`src`) and the framework
//! (`crates/tuikit/src`) alike; `tests/support/source_tree.rs` explains
//! which hygiene rules apply to which crate and how the scans are guarded
//! against visiting too few files.

use std::fs;
use std::path::Path;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, HOST_SRC, Tree};

/// The trees whose inline test modules are measured.
const TREES: &[Tree] = &[HOST_SRC, FRAMEWORK_SRC];

/// Inline `mod tests { ... }` blocks this long or longer must move to a
/// sibling `_tests.rs` file. Counted from the `mod tests {` line to its
/// closing brace.
const INLINE_TEST_MODULE_LIMIT: usize = 100;

/// Line count of each inline `mod tests { ... }` block in `text`, with
/// the 1-based line it starts on. Brace counting ignores braces inside
/// string and char literals well enough for test code here, which keeps
/// literal braces balanced.
fn inline_test_modules(text: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (start, line) in lines.iter().enumerate() {
        if line.trim() != "mod tests {" {
            continue;
        }
        let mut depth = 0i64;
        for (offset, body_line) in lines[start..].iter().enumerate() {
            depth += body_line.matches('{').count() as i64;
            depth -= body_line.matches('}').count() as i64;
            if depth == 0 {
                out.push((start + 1, offset + 1));
                break;
            }
        }
    }
    out
}

/// Every inline test module of `INLINE_TEST_MODULE_LIMIT` lines or more
/// under `trees`, as `path:line: ...` findings.
fn offenders(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut offenders = Vec::new();
    for path in source_tree::rust_files(root, trees) {
        let text = fs::read_to_string(&path).expect("read source file");
        for (line, length) in inline_test_modules(&text) {
            if length >= INLINE_TEST_MODULE_LIMIT {
                offenders.push(format!(
                    "{}:{line}: inline `mod tests` is {length} lines",
                    source_tree::relative(root, &path)
                ));
            }
        }
    }
    offenders
}

#[test]
fn large_test_modules_live_in_sibling_files() {
    let offenders = offenders(source_tree::workspace_root(), TREES);
    assert!(
        offenders.is_empty(),
        "move these test modules to a sibling `_tests.rs` file (ADR 0124):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn a_large_test_module_is_caught_in_every_tree_the_scan_covers() {
    let filler = "    // filler\n".repeat(INLINE_TEST_MODULE_LIMIT);
    let violation = format!("#[cfg(test)]\nmod tests {{\n{filler}}}\n");
    source_tree::assert_scan_covers_every_tree(TREES, &violation, offenders);
}

#[test]
fn the_counter_measures_a_module_from_header_to_closing_brace() {
    let text = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let s = \"{}\";\n    }\n}\n";
    assert_eq!(inline_test_modules(text), vec![(4, 6)]);
}
