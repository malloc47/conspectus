//! The glyph guard: framework code draws a glyph through the `glyphs`
//! module, never by typing the character.
//!
//! A look-alike codepoint typed at a second site (`►` for `▶`) or a glyph
//! no font was ever asked about is how a one-cell column ends up a cell
//! wide and a selection highlight cuts a mark in half. The guard scans the
//! non-test Rust source under `src/` with the same scanner a host uses
//! (`tuikit::glyphs::drawn_glyphs`: non-ASCII characters in code, outside
//! comments and `#[cfg(test)]` items) and fails on two things:
//!
//! 1. any such character outside the `glyphs` module: use a constant from
//!    `glyphs::marks`, or add the glyph to the module with a vetting note;
//! 2. inside the `glyphs` module, any character the framework's vetted
//!    table does not hold.
//!
//! There is no exception list: a control that wants a glyph needs a mark
//! or a table entry. The rule and the way a host extends the table are in
//! `docs/contributing.md`, section "The glyph guard".

#[path = "support/source_scan.rs"]
mod source_scan;

use source_scan::{
    Scan, Source, assert_clean, crate_root, is_rust_file, is_test_path, load_sources, tripwire,
};
use tuikit::glyphs::{VettedGlyphs, drawn_glyphs};

/// Fewest non-test Rust files the scan may visit: `lib.rs`, the four glyph
/// files, and the three theme files make 8 today.
const MIN_FILES: usize = 6;

/// Whether `path` (crate-relative, forward slashes) is the glyph module.
fn in_glyph_module(path: &str) -> bool {
    path == "src/glyphs.rs" || path.starts_with("src/glyphs/")
}

/// Scans the non-test library source in `sources`.
fn glyph_scan(sources: &[Source], vetted: &VettedGlyphs) -> Scan {
    let mut violations = Vec::new();
    let mut visited = 0;
    for source in sources.iter().filter(|source| {
        is_rust_file(&source.path) && source.path.starts_with("src/") && !is_test_path(&source.path)
    }) {
        visited += 1;
        let inside = in_glyph_module(&source.path);
        let unvetted = vetted.unvetted(&source.text);
        let found = if inside {
            unvetted
        } else {
            drawn_glyphs(&source.text)
        };
        for drawn in found {
            let reason = if inside {
                "is not in the vetted table: measure it with scripts/glyph-widths.py and register it"
            } else {
                "is a glyph literal outside the glyphs module: use a constant from `glyphs::marks`, \
                 or add the glyph to the module"
            };
            violations.push(format!(
                "{}:{}: `{}` (U+{:04X}) {reason}",
                source.path, drawn.line, drawn.glyph, drawn.glyph as u32
            ));
        }
    }
    Scan {
        visited,
        violations,
    }
}

#[test]
fn framework_source_draws_glyphs_only_through_the_glyphs_module() {
    let scan = glyph_scan(&load_sources(crate_root()), &VettedGlyphs::default());
    assert_clean("glyph", &scan, MIN_FILES);
}

fn problems(path: &str, text: &str) -> Vec<String> {
    glyph_scan(&[Source::new(path, text)], &VettedGlyphs::default()).violations
}

#[test]
fn a_glyph_typed_in_a_control_is_caught_even_when_it_is_vetted() {
    let leak = "const POINTER: &str = \"►\";\n";
    assert_eq!(
        problems("src/control.rs", leak),
        [
            "src/control.rs:1: `►` (U+25BA) is a glyph literal outside the glyphs module: use a \
          constant from `glyphs::marks`, or add the glyph to the module"
        ]
    );
}

#[test]
fn a_character_literal_and_a_nested_module_are_caught_too() {
    assert_eq!(
        problems("src/control/row.rs", "const A: char = '…';").len(),
        1
    );
    assert_eq!(
        problems("src/theme/spec.rs", "fn dash() -> &'static str { \"—\" }").len(),
        1
    );
}

#[test]
fn comments_tests_and_ascii_are_not_glyph_literals() {
    let clean = r#"
/// Draws `►` before the selected row.
fn draw(marker: &str) -> String {
    // The mark is ► in the default look.
    format!("{marker} item")
}

#[cfg(test)]
mod tests {
    fn fixture() -> &'static str { "◆" }
}
"#;
    assert_eq!(problems("src/control.rs", clean), Vec::<String>::new());
    let scan = glyph_scan(
        &[
            Source::new("src/control_tests.rs", "const A: &str = \"◆\";"),
            Source::new("tests/rig.rs", "const A: &str = \"◆\";"),
            Source::new("docs/notes.md", "◆"),
        ],
        &VettedGlyphs::default(),
    );
    assert_eq!(scan.visited, 0);
    assert!(scan.violations.is_empty());
}

#[test]
fn the_glyphs_module_may_draw_what_the_table_vets_and_nothing_else() {
    let vetted = "pub const POINTER: &str = \"►\";\n";
    assert_eq!(
        problems("src/glyphs/marks.rs", vetted),
        Vec::<String>::new()
    );
    assert_eq!(problems("src/glyphs.rs", vetted), Vec::<String>::new());
    assert_eq!(
        problems("src/glyphs/marks.rs", "pub const SNOW: &str = \"☃\";\n"),
        [
            "src/glyphs/marks.rs:1: `☃` (U+2603) is not in the vetted table: measure it with \
          scripts/glyph-widths.py and register it"
        ]
    );
}

#[test]
fn a_scan_over_nothing_trips_its_tripwire() {
    let scan = glyph_scan(&[], &VettedGlyphs::default());
    assert!(tripwire("glyph", scan.visited, MIN_FILES).is_some());
}
