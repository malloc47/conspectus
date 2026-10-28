//! Every character the sources draw is a vetted glyph.
//!
//! A terminal draws a glyph its monospace font lacks from a fallback font,
//! and that glyph spills over its neighbor, so the framework keeps a table
//! of measured glyphs (`tuikit::glyphs`) and the host adds its own from an
//! adapter (`icons::register_glyphs`). The scan walks every Rust source of
//! both crates and fails on a character drawn in code that the union does
//! not hold. It skips comments, `#[cfg(test)]` items, and test files: a
//! test may spell a glyph to assert what is drawn.
//! `tests/support/source_tree.rs` says which trees a scan names and guards
//! each against visiting too few files.

use std::path::Path;

use tuikit::glyphs::{VettedGlyphs, drawn_glyphs};

use super::source_tree::{self, FRAMEWORK_SRC, HOST_SRC, Tree};

/// The trees whose drawn characters must all be vetted.
const EVERY_SOURCE: &[Tree] = &[HOST_SRC, FRAMEWORK_SRC];

/// Whether a source file is production code: not a test file, and not
/// under a directory of tests or snapshots.
fn is_production(root: &Path, path: &Path) -> bool {
    let relative = source_tree::relative(root, path);
    !source_tree::is_test_file(path)
        && !relative
            .split('/')
            .any(|part| part == "tests" || part == "snapshots" || part.ends_with("_tests"))
}

/// The production sources under `trees` with their text.
fn sources(root: &Path, trees: &[Tree]) -> Vec<(String, String)> {
    source_tree::rust_files(root, trees)
        .into_iter()
        .filter(|path| is_production(root, path))
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            (source_tree::relative(root, &path), text)
        })
        .collect()
}

/// How many characters were drawn, and a finding for each one `table` does
/// not vet.
fn unvetted(root: &Path, trees: &[Tree], table: &VettedGlyphs) -> (usize, Vec<String>) {
    let (mut drawn, mut findings) = (0, Vec::new());
    for (path, text) in sources(root, trees) {
        drawn += drawn_glyphs(&text).len();
        for glyph in table.unvetted(&text) {
            findings.push(format!(
                "{path}:{}: `{}` (U+{:04X}) is drawn but not vetted: measure it with \
                 crates/tuikit/scripts/glyph-widths.py and register it in \
                 src/tui/icons/vetted.rs",
                glyph.line, glyph.glyph, glyph.glyph as u32
            ));
        }
    }
    (drawn, findings)
}

fn host_table() -> VettedGlyphs {
    crate::tui::icons::vetted_glyphs().expect("the host's glyph registrations are valid")
}

#[test]
fn every_glyph_the_sources_draw_is_vetted() {
    let (drawn, findings) = unvetted(source_tree::workspace_root(), EVERY_SOURCE, &host_table());
    assert!(
        drawn > 200,
        "the scan saw only {drawn} drawn characters, so it is probably broken"
    );
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn an_unvetted_glyph_is_caught_in_every_tree_the_scan_covers() {
    let table = host_table();
    source_tree::assert_scan_covers_every_tree(
        EVERY_SOURCE,
        "fn f() -> &'static str { \"\u{2603}\" }\n",
        |root, trees| unvetted(root, trees, &table).1,
    );
}

#[test]
fn test_files_and_test_items_may_draw_what_they_like() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let write = |relative: &str, text: &str| {
        let path = scratch.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent directory")).expect("create tree");
        std::fs::write(path, text).expect("plant the file");
    };
    let snowman = "fn f() -> &'static str { \"\u{2603}\" }\n";
    write("src/widget_tests.rs", snowman);
    write("src/tests.rs", snowman);
    write("src/widget_tests/case.rs", snowman);
    write("src/snapshots/case.rs", snowman);
    write(
        "src/widget.rs",
        "// \u{2603} in prose\nfn f() {}\n#[cfg(test)]\nmod tests {\n    const A: &str = \"\u{2603}\";\n}\n",
    );
    let (drawn, findings) = unvetted(scratch.path(), &[Tree::new("src", 1)], &host_table());
    assert_eq!((drawn, findings), (0, Vec::new()));
}
