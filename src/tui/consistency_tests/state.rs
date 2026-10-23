//! State is drawn one way: a glyph in views of one primary entity, a
//! word in views that mix kinds, the same pin marker pair and the same
//! age for the same entity in every view, and no `gone` anywhere the
//! operator reads.

use super::*;

/// The words the mixed views (Pinned, History) use for status.
const STATUS_WORDS: [&str; 10] = [
    "live",
    "stopped",
    "removed",
    "missing",
    "planned",
    "unknown",
    "resumable",
    "merged",
    "closed",
    "draft",
];

/// Whitespace-separated words of a line.
fn words(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

fn is_age(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    word == "—"
        || (chars.len() > 1
            && chars[..chars.len() - 1].iter().all(char::is_ascii_digit)
            && matches!(chars[chars.len() - 1], 's' | 'm' | 'h' | 'd' | 'w' | '…'))
}

fn lines_of(view: View) -> Vec<String> {
    let mut app = app_listing_all(view);
    left_pane_lines(&frame(&mut app, 220, 40), 220)
}

/// The row mentioning `label`, or a panic naming the frame.
fn row<'a>(lines: &'a [String], label: &str) -> &'a str {
    lines
        .iter()
        .skip(1)
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("no row for {label}:\n{}", lines.join("\n")))
}

fn age_of(lines: &[String], label: &str) -> String {
    words(row(lines, label))
        .into_iter()
        .find(|word| is_age(word))
        .unwrap_or_else(|| panic!("no age on the {label} row: {}", row(lines, label)))
        .to_string()
}

#[test]
fn views_of_one_primary_entity_show_state_only_as_glyphs() {
    for view in [View::Sessions, View::Mux, View::Prs] {
        for line in lines_of(view).iter().skip(1) {
            for word in words(line) {
                assert!(
                    !STATUS_WORDS.contains(&word) && word != "open" && word != "gone",
                    "{view:?} spells state as the word `{word}`: {line}"
                );
            }
        }
    }
}

#[test]
fn mixed_views_use_only_the_shared_status_vocabulary() {
    let allowed = [
        "live",
        "stopped",
        "removed",
        "missing",
        "planned",
        "unknown",
        "in",
        "mux",
        "resumable",
        "open",
        "draft",
        "merged",
        "closed",
    ];
    for view in [View::Pinned, View::History] {
        let lines = lines_of(view);
        let mut seen = 0;
        for line in lines.iter().skip(1) {
            // The status is the first word after the label that is a
            // known state word or looks like one.
            if words(line).iter().any(|w| allowed.contains(w)) {
                seen += 1;
            }
            assert!(!words(line).contains(&"gone"), "{view:?} says gone: {line}");
        }
        assert!(
            seen > 0,
            "{view:?} showed no status words:\n{}",
            lines.join("\n")
        );
    }
}

#[test]
fn nothing_the_operator_reads_says_gone() {
    for view in [
        View::Sessions,
        View::Mux,
        View::Prs,
        View::Pinned,
        View::History,
    ] {
        let mut app = app_listing_all(view);
        let shown = frame(&mut app, 220, 40);
        for line in shown.lines() {
            let lower = line.to_lowercase();
            assert!(
                !lower
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == "gone"),
                "{view:?} says gone: {line}"
            );
        }
    }
}

#[test]
fn the_pin_marker_is_the_same_pair_in_every_view() {
    // (view, row label, whether the pin's entity is running)
    let cases = [
        (View::Mux, "conspectus-main", true),
        (View::Mux, "config-agent3", false),
        (View::Pinned, "conspectus-main", true),
        (View::Pinned, "e44d01cb", true),
        (View::Pinned, "config-agent3", false),
        (View::Pinned, "nightly", false),
        (View::History, "config-agent3", false),
    ];
    for (view, label, running) in cases {
        let lines = lines_of(view);
        let line = row(&lines, label);
        let before: String = line.split(label).next().unwrap_or_default().to_string();
        let (want, other) = if running {
            ('◉', '◎')
        } else {
            ('◎', '◉')
        };
        assert!(
            before.contains(want) && !before.contains(other),
            "{view:?} {label}: expected {want} before the label: {line}"
        );
    }
}

#[test]
fn the_same_entity_shows_the_same_age_in_every_view() {
    let mux = [
        age_of(&lines_of(View::Mux), "conspectus-main"),
        age_of(&lines_of(View::Pinned), "conspectus-main"),
    ];
    assert_eq!(mux[0], mux[1], "a mux's age differs between Mux and Pinned");

    let stopped = [
        age_of(&lines_of(View::Mux), "config-agent3"),
        age_of(&lines_of(View::Pinned), "config-agent3"),
        age_of(&lines_of(View::History), "config-agent3"),
    ];
    assert!(
        stopped.iter().all(|age| *age == stopped[0]),
        "a stopped mux's age differs between views: {stopped:?}"
    );

    let session = [
        age_of(&lines_of(View::Sessions), "e44d01cb"),
        age_of(&lines_of(View::Pinned), "e44d01cb"),
    ];
    assert_eq!(
        session[0], session[1],
        "a session's age differs between Sessions and Pinned"
    );
}

#[test]
fn an_unknown_age_is_a_dash_in_every_view() {
    for view in [View::Mux, View::Pinned] {
        let lines = lines_of(view);
        for line in lines.iter().skip(1) {
            // A row (it has a kind glyph) with no age token at all would
            // leave the cell blank.
            if line.contains(['▣', '●', '◇', '◆', '⇄']) {
                assert!(
                    words(line).iter().any(|w| is_age(w)),
                    "{view:?} row without an age or a dash: {line}"
                );
            }
        }
    }
}
