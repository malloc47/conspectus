//! A shared mark's character is typed once, in the framework.
//!
//! The marks the framework defines (the list cursor, the disclosure
//! arrows, the check and failure marks, the warning mark, a rule cell)
//! each mean one thing wherever they are drawn. Typing the character at a
//! use site is how look-alike codepoints drift apart (`\u{25BA}` for
//! `\u{25B6}`) and how a glyph escapes the width checks, so drawing code
//! names the constant in `tuikit::glyphs::marks` instead. The scan covers
//! the host sources that draw into the terminal UI and skips comments,
//! `#[cfg(test)]` items, and test files.

use std::path::Path;

use tuikit::glyphs::{drawn_glyphs, marks};

use super::glyphs::sources;
use super::source_tree::{self, Tree};
use tuikit::testkit::source::{Literal, string_literals};

/// The host trees that draw into the terminal UI.
const TUI_SRC: Tree = Tree::new("src/tui", 100);
const VIEWER_SRC: Tree = Tree::new("src/viewer", 15);
const DRAWING_SOURCE: &[Tree] = &[TUI_SRC, VIEWER_SRC];

/// The marks whose character must never be typed outside the framework:
/// each has one meaning in every place it is drawn. The ellipsis and the
/// no-value dash are prose punctuation as well, so they are guarded
/// separately by [`bare_marks`].
fn guarded_marks() -> Vec<&'static str> {
    let mut out = vec![
        marks::POINTER,
        marks::BACK,
        marks::EXPANDED,
        marks::COLLAPSED,
        marks::OK,
        marks::FAIL,
        marks::WARNING,
        marks::BULLET,
        marks::RULE,
    ];
    out.extend(marks::SPINNER);
    out
}

/// Marks that are also prose punctuation. Copy such as "Loading…" or
/// "not found — try again" may carry them, but a string that is exactly
/// one of them is the mark.
fn bare_marks() -> [&'static str; 2] {
    [marks::NO_VALUE, marks::ELLIPSIS]
}

/// A reviewed use of a guarded character that is not the mark: the path
/// (relative to the workspace root), the glyph, and why.
struct Exception {
    path: &'static str,
    glyph: &'static str,
    reason: &'static str,
}

const EXCEPTIONS: &[Exception] = &[Exception {
    path: "src/viewer/table.rs",
    glyph: marks::RULE,
    reason: "one of the border characters the table reader recognizes in rendered cells",
}];

/// Every use of a guarded character in drawing code, and every string that
/// is exactly a bare mark, as `path:line: ...` findings. A reviewed
/// exception excuses a path and a glyph; one that excuses nothing is
/// itself a finding.
fn typed_marks(root: &Path, trees: &[Tree], exceptions: &[Exception]) -> Vec<String> {
    let guarded = guarded_marks();
    let mut used = vec![false; exceptions.len()];
    let mut out = Vec::new();
    for (path, text) in sources(root, trees) {
        for drawn in drawn_glyphs(&text) {
            let glyph = drawn.glyph.to_string();
            if !guarded.contains(&glyph.as_str()) {
                continue;
            }
            match exceptions
                .iter()
                .position(|e| e.path == path && e.glyph == glyph)
            {
                Some(index) => used[index] = true,
                None => out.push(format!(
                    "{path}:{}: `{glyph}` is typed here; use the matching constant in \
                     `tuikit::glyphs::marks`",
                    drawn.line
                )),
            }
        }
        for Literal {
            line,
            text: literal,
        } in string_literals(&text)
        {
            if bare_marks().contains(&literal.as_str()) {
                out.push(format!(
                    "{path}:{line}: the string \"{literal}\" is a mark; use the matching \
                     constant in `tuikit::glyphs::marks`"
                ));
            }
        }
    }
    for (exception, used) in exceptions.iter().zip(used) {
        if !used {
            out.push(format!(
                "stale exception for `{}` in {} ({}): it matches nothing, so remove it",
                exception.glyph, exception.path, exception.reason
            ));
        }
    }
    out
}

#[test]
fn no_source_types_the_character_of_a_shared_mark() {
    let findings = typed_marks(source_tree::workspace_root(), DRAWING_SOURCE, EXCEPTIONS);
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn a_typed_mark_is_caught_in_every_tree_the_guard_covers() {
    let violation =
        "fn f() -> &'static str { \"\u{25BA} \" }\nfn g() -> &'static str { \"\u{2014}\" }\n";
    source_tree::assert_scan_covers_every_tree(DRAWING_SOURCE, violation, |root, trees| {
        typed_marks(root, trees, &[])
    });
}

#[test]
fn the_guard_reads_a_bare_mark_but_not_prose_around_one() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let path = scratch.path().join("src/widget.rs");
    std::fs::create_dir_all(path.parent().expect("a parent directory")).expect("create tree");
    std::fs::write(
        &path,
        "fn f() { let _ = (\"\u{2014}\", \"\u{2026}\", \"a \u{2014} b\", \"Loading\u{2026}\"); }\n",
    )
    .expect("plant the file");
    let findings = typed_marks(scratch.path(), &[Tree::new("src", 1)], &[]);
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(findings[0].contains("\"\u{2014}\""), "{findings:?}");
    assert!(findings[1].contains("\"\u{2026}\""), "{findings:?}");
}

#[test]
fn an_exception_excuses_one_path_and_one_glyph_and_goes_stale() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    for name in ["a", "b"] {
        let path = scratch.path().join(format!("src/{name}.rs"));
        std::fs::create_dir_all(path.parent().expect("a parent directory")).expect("create tree");
        std::fs::write(&path, "const C: &str = \"\u{00D7}\";\n").expect("plant the file");
    }
    let excuse = [Exception {
        path: "src/a.rs",
        glyph: marks::FAIL,
        reason: "test",
    }];
    let trees = [Tree::new("src", 1)];
    let findings = typed_marks(scratch.path(), &trees, &excuse);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].starts_with("src/b.rs:1:"), "{findings:?}");

    let stale = [Exception {
        path: "src/missing.rs",
        glyph: marks::FAIL,
        reason: "test",
    }];
    let findings = typed_marks(scratch.path(), &trees, &stale);
    assert!(
        findings
            .iter()
            .any(|finding| finding.starts_with("stale exception")),
        "{findings:?}"
    );
}
