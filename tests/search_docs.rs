//! The operator guide and the design keep describing content search as it
//! ships: the command, the filter flag, the exit codes, and the decision
//! records.

use std::fs;
use std::path::Path;

fn doc(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

#[test]
fn the_operations_guide_documents_the_search_command_and_its_exit_codes() {
    let guide = doc("docs/operations.md");
    for needle in [
        "conspectus search",
        "--contains",
        "--group",
        "--format",
        "no match",
    ] {
        assert!(
            guide.contains(needle),
            "docs/operations.md should mention `{needle}`"
        );
    }
    // Exit status 0 matched, 1 nothing matched, 2 an error.
    let exit_section = guide
        .lines()
        .skip_while(|line| !line.contains("conspectus search"))
        .collect::<Vec<_>>()
        .join("\n");
    for code in ["`0`", "`1`", "`2`"] {
        assert!(
            exit_section.contains(code),
            "the guide should explain exit status {code}"
        );
    }
}

#[test]
fn the_overlay_keys_the_guide_names_are_the_ones_in_the_key_table() {
    let guide = doc("docs/operations.md");
    let table = doc("src/tui/keybindings/overlay.rs");
    for key in ["Ctrl-F", "Ctrl-O", "Ctrl-R", "Ctrl-T", "Ctrl-Y"] {
        assert!(guide.contains(key), "the guide should name {key}");
    }
    for key in ['f', 'o', 'r', 't', 'y'] {
        assert!(
            table.contains(&format!("Char('{key}')")) || table.contains(&format!("'{key}'")),
            "the overlay key table binds Ctrl-{key}"
        );
    }
}

#[test]
fn the_design_names_the_search_decision_records() {
    let design = doc("docs/design.md");
    for adr in ["0147", "0148"] {
        assert!(design.contains(adr), "docs/design.md should cite ADR {adr}");
    }
    let index = doc("docs/adr/README.md");
    for adr in ["0147", "0148"] {
        assert!(index.contains(adr), "the ADR index should list {adr}");
    }
}
