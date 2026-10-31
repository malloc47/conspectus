//! The documentation set and its lint stay in place.
//!
//! The library denies undocumented public modules and items at compile
//! time (`#![deny(missing_docs)]` in `src/lib.rs`). That denial is a line a
//! tidy-up could delete without anything failing, so these tests fail when
//! it, the include that makes the README the crate-level documentation, or a
//! page the contribution guide promises goes missing. The lint and the
//! pages are described in `docs/contributing.md`; what the pages say is
//! held to the code by `tests/doc_sync.rs`.

use std::fs;
use std::path::Path;

/// Crate-level attributes `src/lib.rs` must keep, each on a line of its own.
const REQUIRED_ATTRIBUTES: &[&str] = &[
    "#![doc = include_str!(\"../README.md\")]",
    "#![deny(missing_docs)]",
    "#![deny(rustdoc::broken_intra_doc_links)]",
    "#![deny(rustdoc::private_intra_doc_links)]",
];

/// Pages of the documentation set, relative to the crate root.
const PAGES: &[&str] = &[
    "README.md",
    "docs/guide.md",
    "docs/architecture.md",
    "docs/house-rules.md",
    "docs/contributing.md",
    "docs/roadmap.md",
    "docs/controls/_template.md",
    "docs/decisions/README.md",
];

/// Headings every control page keeps, in this order, so pages share one shape.
const TEMPLATE_SECTIONS: &[&str] = &[
    "## Purpose",
    "## Usage example",
    "## Key table",
    "## Theme tokens",
    "## House rules followed",
    "## Seams left open",
];

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_library_keeps_its_documentation_lints() {
    let lib = read("src/lib.rs");
    let missing: Vec<&str> = REQUIRED_ATTRIBUTES
        .iter()
        .copied()
        .filter(|attribute| !lib.lines().any(|line| line.trim() == *attribute))
        .collect();
    assert!(
        missing.is_empty(),
        "src/lib.rs lost crate-level attributes the documentation lint relies on: {missing:?}"
    );
}

#[test]
fn every_page_of_the_documentation_set_exists_and_has_a_title() {
    for page in PAGES {
        let text = read(page);
        assert!(
            text.lines()
                .next()
                .is_some_and(|line| line.starts_with("# ")),
            "{page} should open with a level-one heading"
        );
        assert!(
            text.lines().count() > 5,
            "{page} is a stub; it should say something"
        );
    }
}

#[test]
fn the_control_template_keeps_the_sections_each_page_fills_in() {
    let template = read("docs/controls/_template.md");
    let mut after = 0;
    for heading in TEMPLATE_SECTIONS {
        let found = template
            .lines()
            .skip(after)
            .position(|line| line.trim_end() == *heading)
            .unwrap_or_else(|| {
                panic!("docs/controls/_template.md lacks `{heading}` (in this order)")
            });
        after += found + 1;
    }
}
