use super::*;

fn glyphs(source: &str) -> Vec<(usize, char)> {
    drawn_glyphs(source)
        .into_iter()
        .map(|drawn| (drawn.line, drawn.glyph))
        .collect()
}

#[test]
fn a_literal_is_reported_with_its_line() {
    let source = "fn a() {}\nconst B: &str = \"→\";\n\nconst C: char = '◆';\n";
    assert_eq!(glyphs(source), [(2, '→'), (4, '◆')]);
}

#[test]
fn every_character_on_a_line_is_reported_in_order() {
    assert_eq!(glyphs("const A: &str = \"▲ and ▼\";"), [(1, '▲'), (1, '▼')]);
}

#[test]
fn ascii_and_an_empty_source_draw_nothing() {
    assert_eq!(glyphs(""), []);
    assert_eq!(glyphs("fn main() { let _ = \"plain text\"; }"), []);
}

#[test]
fn comments_of_every_kind_are_skipped() {
    let source = "\
// → a line comment
/// → a doc comment
//! → an inner doc comment
const A: &str = \"x\"; // → a trailing comment
/* → a block comment */
/* → outer /* → nested */ → still outer */
const B: &str = \"y\";
";
    assert_eq!(glyphs(source), []);
}

#[test]
fn a_comment_marker_inside_a_literal_does_not_start_a_comment() {
    assert_eq!(
        glyphs("const A: &str = \"// → not a comment\";"),
        [(1, '→')]
    );
    assert_eq!(glyphs("const A: &str = \"/* → */\";"), [(1, '→')]);
}

#[test]
fn escapes_and_raw_and_byte_strings_are_read_to_their_end() {
    let source = "\
const A: &str = \"say \\\"→\\\" //\";
const B: &str = r#\"a \"→\" b\"#;
const C: &[u8] = b\"ascii\";
const D: &[u8] = br#\"x\"#;
const E: &str = \"←\";
";
    assert_eq!(glyphs(source), [(1, '→'), (2, '→'), (5, '←')]);
}

#[test]
fn a_character_literal_is_not_a_string_and_a_lifetime_is_not_a_character() {
    let source = "\
const QUOTE: char = '\"';
const ARROW: char = '→';
const ESCAPED: char = '\\'';
fn pick<'a>(text: &'a str) -> &'a str { \"◆\" }
";
    assert_eq!(glyphs(source), [(2, '→'), (4, '◆')]);
}

#[test]
fn an_identifier_ending_in_r_is_not_a_raw_string() {
    assert_eq!(glyphs("let for_r = \"→\"; let r#type = 1;"), [(1, '→')]);
}

#[test]
fn a_test_item_is_skipped_and_the_code_after_it_is_not() {
    let source = "\
const A: &str = \"▲\";

#[cfg(test)]
mod tests {
    const FIXTURE: &str = \"}\";
    fn glyph() -> &'static str { \"→\" }
}

const B: &str = \"▼\";
";
    assert_eq!(glyphs(source), [(1, '▲'), (9, '▼')]);
}

#[test]
fn a_body_less_test_module_with_attributes_is_skipped() {
    let source = "\
const A: &str = \"▲\";
#[cfg(test)]
#[path = \"widget_tests.rs\"]
mod tests;
const B: &str = \"▼\";
";
    assert_eq!(glyphs(source), [(1, '▲'), (5, '▼')]);
}

#[test]
fn a_test_gated_function_is_skipped() {
    let source = "\
#[cfg(test)]
fn helper(values: [&str; 2]) -> &str { \"→\" }
const A: &str = \"←\";
";
    assert_eq!(glyphs(source), [(3, '←')]);
}

#[test]
fn an_unclosed_literal_or_comment_ends_at_the_end_of_the_source() {
    assert_eq!(glyphs("const A: &str = \"→"), [(1, '→')]);
    assert_eq!(glyphs("/* → never closed"), []);
}
