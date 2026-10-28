//! The raw-style guard: framework code draws with the theme its host
//! passes in, never with a color or a dim modifier of its own (house rule
//! 13).
//!
//! A control that names `Color::Yellow` ignores the host's palette, and one
//! that adds `Modifier::DIM` sidesteps the `muted` token a host can
//! retarget (some terminals draw nothing for dim). The guard scans
//! non-test Rust source the way the independence guards do (comments,
//! literals, and `#[cfg(test)]` items blanked first; see
//! `support/source_scan.rs`) and fails on each needle below. The theme
//! module defines the default palette and the grammar that names colors,
//! so it is the reviewed exception. The rule and the exception process
//! are in `docs/contributing.md`, section "The raw-style guard".

#[path = "support/source_scan.rs"]
mod source_scan;

use source_scan::{
    Exception, Needle, Scan, Source, assert_clean, contains_at_word_start, crate_root,
    is_rust_file, is_test_path, load_sources, sanitize_rust, stale_exceptions, strip_test_items,
    tripwire,
};

/// Fewest non-test Rust files the scan may visit: `lib.rs`, four glyph
/// files, three theme files, and six text files make 14 today.
const MIN_FILES: usize = 10;

/// What the guard looks for, matched at a word start.
#[rustfmt::skip]
const NEEDLES: &[Needle] = &[
    Needle { text: "Color::", why: "is a raw color; read a theme token or a named style" },
    Needle { text: "Modifier::DIM", why: "is a raw dim; use the theme's `muted` token" },
    Needle { text: "Stylize", why: "brings color and dim shorthands that bypass the theme" },
];

/// Reviewed files that may name raw values, with why.
const EXCEPTIONS: &[Exception] = &[
    Exception {
        path: "src/theme.rs",
        term: "Color::",
        reason: "the token table holds the default palette",
    },
    Exception {
        path: "src/theme.rs",
        term: "Modifier::DIM",
        reason: "the default `muted` and cold recency tokens are dim",
    },
    Exception {
        path: "src/theme/spec.rs",
        term: "Color::",
        reason: "the grammar maps color names to colors",
    },
    Exception {
        path: "src/theme/spec.rs",
        term: "Modifier::DIM",
        reason: "the grammar maps `dim` to the modifier",
    },
];

/// Scans the non-test Rust source in `sources` for raw styles.
fn raw_style_scan(sources: &[Source], exceptions: &[Exception]) -> Scan {
    let mut used = vec![false; exceptions.len()];
    let mut violations = Vec::new();
    let mut visited = 0;
    for source in sources
        .iter()
        .filter(|source| is_rust_file(&source.path) && !is_test_path(&source.path))
    {
        visited += 1;
        let code = strip_test_items(&sanitize_rust(&source.text));
        for (index, line) in code.lines().enumerate() {
            for needle in NEEDLES {
                if !contains_at_word_start(line, needle.text) {
                    continue;
                }
                let excused = exceptions
                    .iter()
                    .position(|entry| entry.path == source.path && entry.term == needle.text);
                match excused {
                    Some(index) => used[index] = true,
                    None => violations.push(format!(
                        "{}:{}: `{}` {}",
                        source.path,
                        index + 1,
                        needle.text,
                        needle.why
                    )),
                }
            }
        }
    }
    violations.extend(stale_exceptions("raw-style", exceptions, &used));
    Scan {
        visited,
        violations,
    }
}

#[test]
fn framework_source_draws_only_with_theme_tokens() {
    let scan = raw_style_scan(&load_sources(crate_root()), EXCEPTIONS);
    assert_clean("raw-style", &scan, MIN_FILES);
}

fn problems(path: &str, text: &str) -> Vec<String> {
    raw_style_scan(&[Source::new(path, text)], &[]).violations
}

#[test]
fn the_guard_fails_on_every_needle() {
    for needle in NEEDLES {
        let leak = format!("fn leak() {{ let _ = {}; }}\n", needle.text);
        assert!(
            !problems("src/control.rs", &leak).is_empty(),
            "`{}` did not trip the raw-style guard",
            needle.text
        );
    }
}

#[test]
fn the_guard_catches_realistic_leaks() {
    let leaks = [
        "let style = Style::default().fg(Color::Yellow);",
        "let style = Style::new().bg(ratatui::style::Color::DarkGray);",
        "use ratatui::style::Color::{Cyan, Gray};",
        "let rgb = Color::Rgb(1, 2, 3);",
        "let dim = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);",
        "let dim = span!(Modifier::DIM; \"note\");",
        "use ratatui::style::Stylize;",
    ];
    for leak in leaks {
        assert!(
            !problems("src/control.rs", leak).is_empty(),
            "no raw-style violation for: {leak}"
        );
    }
}

#[test]
fn the_guard_allows_theme_reads_other_modifiers_and_prose() {
    let clean = r#"
use ratatui::style::{Color, Modifier, Style};
/// Draws in `Color::Yellow` unless the theme says otherwise; never `Modifier::DIM`.
fn draw(theme: &Theme, color: Color) -> Style {
    // Color::Red in a comment
    let _ = "Color::Blue in a string";
    Style::default()
        .fg(theme.accent)
        .add_modifier(theme.muted | Modifier::BOLD | Modifier::ITALIC | Modifier::REVERSED)
        .bg(color)
}
"#;
    assert_eq!(problems("src/control.rs", clean), Vec::<String>::new());
}

#[test]
fn the_guard_skips_test_code_and_reports_the_file_line_and_needle() {
    let source = "
fn live() {}

#[cfg(test)]
mod tests {
    fn fixture() -> Color { Color::Red }
}

fn after() -> Style { Style::default().fg(Color::Green) }
";
    assert_eq!(
        problems("src/control.rs", source),
        ["src/control.rs:9: `Color::` is a raw color; read a theme token or a named style"]
    );
    let scan = raw_style_scan(
        &[
            Source::new("src/control_tests.rs", "fn f() { Color::Red; }"),
            Source::new("tests/rig.rs", "fn f() { Color::Red; }"),
            Source::new("docs/notes.md", "Color::Red"),
        ],
        &[],
    );
    assert_eq!(scan.visited, 0);
    assert!(scan.violations.is_empty());
}

#[test]
fn an_exception_excuses_one_file_and_one_needle_and_goes_stale() {
    let exception = [Exception {
        path: "src/palette.rs",
        term: "Color::",
        reason: "test",
    }];
    let sources = [
        Source::new(
            "src/palette.rs",
            "const A: Color = Color::Red;\nconst B: Modifier = Modifier::DIM;\n",
        ),
        Source::new("src/control.rs", "const A: Color = Color::Red;\n"),
    ];
    let scan = raw_style_scan(&sources, &exception);
    assert_eq!(scan.violations.len(), 2, "{:?}", scan.violations);
    assert!(scan.violations[0].starts_with("src/palette.rs:2: `Modifier::DIM`"));
    assert!(scan.violations[1].starts_with("src/control.rs:1: `Color::`"));

    let stale = raw_style_scan(&[Source::new("src/palette.rs", "fn f() {}\n")], &exception);
    assert_eq!(stale.violations.len(), 1);
    assert!(stale.violations[0].contains("stale"));
}

#[test]
fn a_scan_over_nothing_trips_its_tripwire() {
    let scan = raw_style_scan(&[], EXCEPTIONS);
    assert!(tripwire("raw-style", scan.visited, MIN_FILES).is_some());
}

#[test]
fn every_exception_names_a_needle_and_states_a_reason() {
    for entry in EXCEPTIONS {
        assert!(
            NEEDLES.iter().any(|needle| needle.text == entry.term),
            "`{}` is not a needle",
            entry.term
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "{} has no reason",
            entry.path
        );
    }
}
