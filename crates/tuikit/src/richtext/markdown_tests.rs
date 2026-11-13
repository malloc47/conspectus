use super::*;

fn texts(lines: &[Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

#[test]
fn a_loose_list_keeps_each_number_beside_its_text() {
    let lines = texts(&markdown(
        "1. first item\n\n2. second item",
        40,
        &Theme::default(),
    ));
    assert!(
        lines.iter().any(|line| line.ends_with("1. first item")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.ends_with("2. second item")),
        "{lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.trim() == "1." || line.trim() == "2."),
        "no marker on a line of its own: {lines:?}"
    );
}

#[test]
fn a_loose_bullet_list_keeps_each_bullet_beside_its_text() {
    let lines = texts(&markdown("- one\n\n- two", 40, &Theme::default()));
    assert!(
        lines.iter().any(|line| line.ends_with("- one")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.ends_with("- two")),
        "{lines:?}"
    );
}

#[test]
fn every_line_fits_the_width() {
    let body = "A paragraph long enough to wrap more than once at a narrow width.\n\n\
                | key | value |\n|---|---|\n| first | a value that is long enough to wrap |\n\n\
                ```\nshort\n```";
    for width in [12u16, 20, 33] {
        for line in markdown(body, width, &Theme::default()) {
            assert!(line.width() <= usize::from(width), "{width}: {line:?}");
        }
    }
}

#[test]
fn a_table_between_paragraphs_is_laid_out_with_its_borders_receding() {
    let theme = Theme::default();
    let lines = markdown(
        "Before.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nAfter.",
        30,
        &theme,
    );
    let text = texts(&lines);
    let before = text
        .iter()
        .position(|line| line == "Before.")
        .expect("prose before");
    let after = text
        .iter()
        .position(|line| line == "After.")
        .expect("prose after");
    let rule = text
        .iter()
        .position(|line| line.contains('\u{2550}'))
        .expect("the header rule");
    assert!(before < rule && rule < after, "{text:?}");
    let border = lines[rule]
        .spans
        .iter()
        .find(|span| span.content.contains('\u{2550}'))
        .expect("a border span");
    assert_eq!(border.style, crate::chip::Tone::Inert.style(&theme));
}

#[test]
fn pipes_inside_a_code_fence_are_not_a_table() {
    let lines = texts(&markdown(
        "```\n| a | b |\n|---|---|\n```",
        40,
        &Theme::default(),
    ));
    assert!(lines.iter().any(|line| line == "| a | b |"), "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.contains('\u{2550}')),
        "{lines:?}"
    );
}

#[test]
fn fenced_code_in_a_known_language_is_highlighted() {
    let lines = markdown("```rust\nfn main() {}\n```", 40, &Theme::default());
    let code = lines
        .iter()
        .find(|line| line.to_string().contains("fn main"))
        .expect("the code line");
    assert!(
        code.spans.iter().any(|span| span.style.fg.is_some()),
        "highlighted: {code:?}"
    );
}

#[test]
fn code_gives_one_line_per_source_line_without_the_fences() {
    let lines = code("fn main() {\n    let x = 1;\n}", "rust");
    assert_eq!(
        texts(&lines),
        ["fn main() {", "    let x = 1;", "}"],
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.style.fg.is_some()))
    );
}

#[test]
fn code_in_a_language_the_highlighter_does_not_know_is_plain() {
    let lines = code("some words", "no-such-language");
    assert_eq!(texts(&lines), ["some words"]);
    assert!(
        lines[0].spans.iter().all(|span| span.style.fg.is_none()),
        "{lines:?}"
    );
}

#[test]
fn code_holding_a_fence_of_its_own_stays_whole() {
    let source = "before\n```\ninside\n```\nafter";
    assert_eq!(
        texts(&code(source, "markdown")),
        ["before", "```", "inside", "```", "after"]
    );
}
