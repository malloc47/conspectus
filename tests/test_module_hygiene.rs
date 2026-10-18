//! Large unit-test modules live in a sibling file so a source file's
//! production code reads at a glance (ADR 0124). A module of
//! `INLINE_TEST_MODULE_LIMIT` lines or more moves out of line:
//!
//! ```text
//! #[cfg(test)]
//! #[path = "foo_tests.rs"]
//! mod tests;
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// Inline `mod tests { ... }` blocks this long or longer must move to a
/// sibling `_tests.rs` file. Counted from the `mod tests {` line to its
/// closing brace.
const INLINE_TEST_MODULE_LIMIT: usize = 100;

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

#[test]
fn large_test_modules_live_in_sibling_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    files.sort();

    let mut offenders = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("read source file");
        for (line, length) in inline_test_modules(&text) {
            if length >= INLINE_TEST_MODULE_LIMIT {
                offenders.push(format!(
                    "{}:{line}: inline `mod tests` is {length} lines",
                    path.strip_prefix(root).unwrap_or(&path).display()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "move these test modules to a sibling `_tests.rs` file (ADR 0124):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_counter_measures_a_module_from_header_to_closing_brace() {
    let text = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let s = \"{}\";\n    }\n}\n";
    assert_eq!(inline_test_modules(text), vec![(4, 6)]);
}
