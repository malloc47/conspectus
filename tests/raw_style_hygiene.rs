//! The TUI and the transcript viewer draw with the theme the operator
//! configures (`[tui.theme]`), never with a color or a dim of their own
//! (ADR 0153, house rule 13). A widget that names `Color::Yellow` ignores
//! the operator's palette, and one that adds `Modifier::DIM` sidesteps the
//! `muted` token an operator can retarget (some terminals draw nothing for
//! dim). Draw with a framework token (`theme.warning`, `theme.muted`), or
//! with a role the host registers in `src/tui/theme.rs` and reads through
//! an accessor (`theme.pr_open()`).
//!
//! The scan walks non-test Rust source under `src/tui` and `src/viewer`
//! with comments, literals, and `#[cfg(test)]` items blanked
//! (`tuikit::testkit::source::production_code`), skips unit-test files
//! and the files of a test module's directory (`consistency_tests/`), and
//! fails on each needle below where a word starts. The framework guards its
//! own source with the same needles (`crates/tuikit/tests/raw_styles.rs`),
//! which also refuse a reversed cursor row; the host's cursor rows draw
//! through the framework's `selection` module.
//!
//! The reviewed exceptions are per file and per needle, each with its
//! reason, and one that matches nothing fails as stale. The ANSI encoder of
//! `tui --snapshot` is the framework test kit's (`tuikit::testkit::ansi`),
//! covered by the framework guard's exception, so `src/tui/snapshot.rs`
//! names no color and needs none here. `tests/support/source_tree.rs`
//! explains which hygiene rules apply to which crate and how the scans are
//! guarded against visiting too few files.

use std::fs;
use std::path::{Component, Path};

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::Tree;
use tuikit::testkit::source::{contains_at_word_start, production_code};

/// The trees scanned: the TUI (172 Rust files today) and the transcript
/// viewer it opens (29 files today).
const TREES: &[Tree] = &[Tree::new("src/tui", 130), Tree::new("src/viewer", 21)];

/// Fewest production files the scan may read, after unit-test files and
/// test modules are skipped: 101 today. A test-code filter that skipped
/// everything would otherwise pass with the trees' tripwires intact.
const MIN_PRODUCTION_FILES: usize = 75;

/// A piece of source text the scan looks for, and what to use instead.
struct Needle {
    text: &'static str,
    why: &'static str,
}

/// What the scan looks for, matched where a word starts.
#[rustfmt::skip]
const NEEDLES: &[Needle] = &[
    Needle { text: "Color::", why: "is a raw color; read a theme token or a registered role" },
    Needle { text: "Modifier::DIM", why: "is a raw dim; use the theme's `muted` token" },
    Needle { text: "Stylize", why: "brings color and dim shorthands that bypass the theme" },
];

/// One reviewed exception: `needle` may appear in `path` (relative to the
/// workspace root, forward slashes), for `reason`.
struct Exception {
    path: &'static str,
    needle: &'static str,
    reason: &'static str,
}

/// Files whose raw colors are the defaults a theme starts from. Add an
/// entry only for a file that defines defaults the operator can override;
/// a widget that seems to need a raw color needs a role in `ROLES` (with
/// its row in the `[tui.theme]` example of `docs/operations.md`) instead.
const EXCEPTIONS: &[Exception] = &[Exception {
    path: "src/tui/theme.rs",
    needle: "Color::",
    reason: "the default colors of the roles the host registers in the framework theme (PR \
             states, node kinds, link rows, chips) and of each harness's badge: domain data \
             that `[tui.theme]` overrides by key",
}];

/// What a scan read and what it found.
struct Scan {
    /// Production files read.
    visited: usize,
    /// `path:line: needle why` findings, the path relative to the root.
    findings: Vec<String>,
    /// Exceptions that matched nothing, described.
    stale: Vec<String>,
}

/// Whether `relative` is test code by its path: a unit-test file, or a
/// file below a `tests` directory or a test module's directory (a
/// `*_tests` directory, as `#[cfg(test)] mod consistency_tests;` has).
fn is_test_code(relative: &str) -> bool {
    let path = Path::new(relative);
    let in_test_directory = path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .any(|part| match part {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                name == "tests" || name.ends_with("_tests")
            }
            _ => false,
        });
    source_tree::is_test_file(path) || in_test_directory
}

/// Scans the production code under `trees` (from `root`) for the needles,
/// excusing what `exceptions` lists.
fn raw_styles(root: &Path, trees: &[Tree], exceptions: &[Exception]) -> Scan {
    let mut used = vec![false; exceptions.len()];
    let mut findings = Vec::new();
    let mut visited = 0;
    for path in source_tree::rust_files(root, trees) {
        let relative = source_tree::relative(root, &path);
        if is_test_code(&relative) {
            continue;
        }
        visited += 1;
        let text = fs::read_to_string(&path).expect("read TUI source");
        for (index, line) in production_code(&text).lines().enumerate() {
            for needle in NEEDLES {
                if !contains_at_word_start(line, needle.text) {
                    continue;
                }
                let excused = exceptions
                    .iter()
                    .position(|entry| entry.path == relative && entry.needle == needle.text);
                match excused {
                    Some(entry) => used[entry] = true,
                    None => findings.push(format!(
                        "{relative}:{}: `{}` {}",
                        index + 1,
                        needle.text,
                        needle.why
                    )),
                }
            }
        }
    }
    let stale = exceptions
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(entry, _)| {
            format!(
                "`{}` in {} matches nothing (reason was: {}); remove the exception",
                entry.needle, entry.path, entry.reason
            )
        })
        .collect();
    Scan {
        visited,
        findings,
        stale,
    }
}

/// The findings of a scan with no exceptions, for the planted tests.
fn findings(root: &Path, trees: &[Tree]) -> Vec<String> {
    raw_styles(root, trees, &[]).findings
}

#[test]
fn the_tui_draws_only_with_theme_tokens_and_roles() {
    let scan = raw_styles(source_tree::workspace_root(), TREES, EXCEPTIONS);
    assert!(
        scan.findings.is_empty(),
        "TUI code styles with a raw value, which ignores the operator's `[tui.theme]`; read a \
         framework token or a role registered in `src/tui/theme.rs` instead:\n{}",
        scan.findings.join("\n")
    );
    assert!(
        scan.stale.is_empty(),
        "a reviewed raw-style exception is stale:\n{}",
        scan.stale.join("\n")
    );
    assert!(
        scan.visited >= MIN_PRODUCTION_FILES,
        "the raw-style scan read {} production files, fewer than {MIN_PRODUCTION_FILES}: its \
         test-code filter or its trees are probably wrong",
        scan.visited
    );
}

#[test]
fn a_raw_style_is_caught_in_every_tree_the_scan_covers() {
    for needle in NEEDLES {
        let violation = format!("fn f() {{\n    let _ = {};\n}}\n", needle.text);
        source_tree::assert_scan_covers_every_tree(TREES, &violation, findings);
    }
}

#[test]
fn realistic_leaks_are_caught_and_theme_reads_prose_and_test_code_are_not() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let tui = scratch.path().join("src/tui");
    fs::create_dir_all(tui.join("consistency_tests")).expect("create trees");
    fs::create_dir_all(scratch.path().join("src/viewer")).expect("create trees");
    let leaks = "\
use ratatui::style::Stylize;
fn draw(theme: &Theme) -> Style {
    let rgb = ratatui::style::Color::Rgb(1, 2, 3);
    Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM | Modifier::BOLD)
}
";
    fs::write(tui.join("widget.rs"), leaks).expect("write leaks");
    let clean = r#"
use ratatui::style::{Color, Modifier, Style};
/// Draws in `Color::Yellow` unless the theme says otherwise; never `Modifier::DIM`.
fn draw(theme: &Theme, color: Color) -> Style {
    // Color::Red in a comment
    let _ = "Color::Blue in a string";
    let _ = MyColor::Red;
    Style::default()
        .fg(theme.accent)
        .add_modifier(theme.muted | Modifier::BOLD | Modifier::ITALIC)
        .bg(color)
}

#[cfg(test)]
mod tests {
    fn fixture() -> Color { Color::Red }
}
"#;
    fs::write(scratch.path().join("src/viewer/render.rs"), clean).expect("write clean");
    let test_code =
        "fn fixture() -> Style { Style::new().fg(Color::Red).add_modifier(Modifier::DIM) }\n";
    fs::write(tui.join("widget_tests.rs"), test_code).expect("write a test file");
    fs::write(tui.join("consistency_tests/feedback.rs"), test_code).expect("write a test module");

    let trees = [Tree::new("src/tui", 1), Tree::new("src/viewer", 1)];
    let scan = raw_styles(scratch.path(), &trees, &[]);
    assert_eq!(
        scan.findings,
        [
            "src/tui/widget.rs:1: `Stylize` brings color and dim shorthands that bypass the theme",
            "src/tui/widget.rs:3: `Color::` is a raw color; read a theme token or a registered role",
            "src/tui/widget.rs:4: `Color::` is a raw color; read a theme token or a registered role",
            "src/tui/widget.rs:4: `Modifier::DIM` is a raw dim; use the theme's `muted` token",
        ]
    );
    assert_eq!(scan.visited, 2, "the two test files are skipped");
}

#[test]
fn an_exception_excuses_one_file_and_one_needle_and_goes_stale() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let tui = scratch.path().join("src/tui");
    fs::create_dir_all(&tui).expect("create tree");
    let exceptions = [Exception {
        path: "src/tui/palette.rs",
        needle: "Color::",
        reason: "test",
    }];
    let trees = [Tree::new("src/tui", 1)];
    fs::write(
        tui.join("palette.rs"),
        "const A: Color = Color::Red;\nconst B: Modifier = Modifier::DIM;\n",
    )
    .expect("write palette");
    fs::write(tui.join("widget.rs"), "const A: Color = Color::Red;\n").expect("write widget");
    let scan = raw_styles(scratch.path(), &trees, &exceptions);
    assert_eq!(scan.findings.len(), 2, "{:?}", scan.findings);
    assert!(scan.findings[0].starts_with("src/tui/palette.rs:2: `Modifier::DIM`"));
    assert!(scan.findings[1].starts_with("src/tui/widget.rs:1: `Color::`"));
    assert!(scan.stale.is_empty());

    fs::write(tui.join("palette.rs"), "fn f() {}\n").expect("clear palette");
    let stale = raw_styles(scratch.path(), &trees, &exceptions);
    assert_eq!(stale.stale.len(), 1);
    assert!(
        stale.stale[0].contains("src/tui/palette.rs matches nothing"),
        "{:?}",
        stale.stale
    );
}

#[test]
fn test_code_is_recognized_by_its_path() {
    for test in [
        "src/tui/app_tests.rs",
        "src/tui/tests.rs",
        "src/tui/consistency_tests/feedback.rs",
        "src/viewer/tests/fixture.rs",
    ] {
        assert!(is_test_code(test), "{test} is test code");
    }
    for production in [
        "src/tui/app.rs",
        "src/tui/consistency.rs",
        "src/tui/contests/x.rs",
        "src/viewer/render.rs",
    ] {
        assert!(!is_test_code(production), "{production} is production code");
    }
}

#[test]
fn every_exception_names_a_needle_and_states_a_reason() {
    for entry in EXCEPTIONS {
        assert!(
            NEEDLES.iter().any(|needle| needle.text == entry.needle),
            "`{}` is not a needle",
            entry.needle
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "{} has no reason",
            entry.path
        );
    }
}
