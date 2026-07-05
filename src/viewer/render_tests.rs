// Extracted from render.rs H-HYG-011 rolling wave via #[path = "render_tests.rs"] mod tests;
use super::*;
use crate::viewer::model::{TranscriptTurn, TurnKind, TurnRole};
use crate::viewer::state::ToolDetail;

fn turn(role: TurnRole, kind: TurnKind, body: &str) -> TranscriptTurn {
    TranscriptTurn {
        role,
        kind,
        body: body.to_string(),
        timestamp: None,
        aborted: false,
    }
}

fn flat_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

/// Visible-text expectation of the chip pill for a given label,
/// matching `format!(" {label:>width$} ")`. The pill
/// is followed by ` │ ` (no extra space — the pill's trailing
/// space + separator's leading space give two cells between
/// the label and the rule).
fn chip_pill(label: &str, inner_width: usize) -> String {
    format!(" {label:>inner_width$} ")
}

#[test]
fn first_line_carries_chip_pill_subsequent_blank_gutter() {
    // Use a paragraph break (blank line) so the Markdown
    // renderer keeps the two lines separate — a single \n is a
    // CommonMark soft break and collapses to a space.
    let t = turn(TurnRole::User, TurnKind::Message, "hello\n\nworld");
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 40, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    let expected_first = format!("{} │ hello", chip_pill("you", inner));
    assert_eq!(texts[0], expected_first);
    let blank_pad = " ".repeat(gutter_width(inner) as usize);
    assert!(
        texts.iter().any(|l| l == &format!("{blank_pad} │ world")),
        "expected blank-gutter `world` line, got {texts:?}"
    );
    // Last line is the inter-turn spacer.
    assert_eq!(texts.last().unwrap(), "");
}

#[test]
fn assistant_role_uses_ai_chip() {
    let t = turn(TurnRole::Assistant, TurnKind::Message, "hi");
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 40, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    assert!(
        texts[0].contains(&format!("{} │ hi", chip_pill("ai", inner))),
        "got {:?}",
        texts[0]
    );
}

#[test]
fn thinking_chip_label() {
    let t = turn(TurnRole::Assistant, TurnKind::Thinking, "musing");
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 40, ToolDetail::Full, inner);
    assert!(
        flat_text(&out)[0].contains(&format!("{} │", chip_pill("thinking", inner))),
        "got {:?}",
        flat_text(&out)[0]
    );
}

#[test]
fn tool_use_and_tool_result_chips() {
    let theme = Theme::default();
    let inner = 8;
    let call_turn = turn(TurnRole::Assistant, TurnKind::ToolUse, "ls");
    let call = render_turn(&call_turn, &theme, 40, ToolDetail::Full, inner);
    assert!(
        flat_text(&call)[0].contains(&format!("{} │ ls", chip_pill("tool", inner))),
        "got {:?}",
        flat_text(&call)[0]
    );
    let res_turn = turn(TurnRole::Assistant, TurnKind::ToolResult, "ok");
    let res = render_turn(&res_turn, &theme, 40, ToolDetail::Full, inner);
    // Tool result uses the corner-arrow glyph.
    assert!(
        flat_text(&res)[0].contains(&format!("{} │ ok", chip_pill("↳ result", inner))),
        "got {:?}",
        flat_text(&res)[0]
    );
}

#[test]
fn compaction_summary_uses_compact_chip() {
    let t = turn(TurnRole::User, TurnKind::CompactionSummary, "summary");
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 40, ToolDetail::Full, inner);
    assert!(
        flat_text(&out)[0].contains(&format!("{} │ summary", chip_pill("compact", inner))),
        "got {:?}",
        flat_text(&out)[0]
    );
}

#[test]
fn plain_text_wraps_at_content_width() {
    // 60-char body, content_width=20, should wrap.
    let body = "one two three four five six seven eight nine ten";
    let t = turn(TurnRole::Assistant, TurnKind::ToolResult, body);
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 20, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    // Drop the spacer line.
    let body_lines: Vec<&String> = texts.iter().take(texts.len() - 1).collect();
    assert!(
        body_lines.len() >= 3,
        "expected wrapped lines, got {body_lines:?}"
    );
    for line in &body_lines {
        let leader = leader_width(inner) as usize;
        assert!(
            line.chars().count() >= leader,
            "line shorter than leader: {line:?}"
        );
    }
}

#[test]
fn hard_break_handles_oversized_tokens() {
    // 30-char "word" with no whitespace; content_width=10.
    let body = "abcdefghijklmnopqrstuvwxyz1234";
    let t = turn(TurnRole::Assistant, TurnKind::ToolResult, body);
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 10, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    // Should produce ≥ 3 body lines (30/10 = 3). +1 spacer = 4.
    assert!(texts.len() >= 4, "got {texts:?}");
}

#[test]
fn wide_word_with_tiny_first_budget_does_not_hard_break_per_char() {
    // Regression: a styled inline span ("ccview") arriving at
    // wrap_styled_line with only 1 cell remaining on the
    // current line used to hard-break the whole word at the
    // first-budget width (1 cell per piece), so the word
    // rendered as one character per line. The fix: switch to
    // the rest budget before deciding whether to hard-break.
    let body = "leading prefix that takes most of the line ccview tail";
    let t = turn(TurnRole::Assistant, TurnKind::ToolResult, body);
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 50, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    // The word "ccview" should appear intact on a single line
    // somewhere in the wrapped output (without inter-character
    // breaks).
    assert!(
        texts.iter().any(|l| l.contains("ccview")),
        "ccview should appear intact, got:\n{}",
        texts.join("\n")
    );
    // And specifically no line should be a single-character body.
    for line in &texts {
        // Strip the leader (gutter + ` │ `) and check the
        // post-separator content.
        if let Some(rest) = line.split_once('│') {
            let content = rest.1.trim();
            assert!(
                content.len() != 1 || !content.chars().next().unwrap().is_alphabetic(),
                "single-alpha-char body line is the hard-break bug: {line:?}"
            );
        }
    }
}

#[test]
fn list_marker_does_not_orphan_to_its_own_line() {
    // Reproduces the orphan: a Markdown numbered list item
    // whose body text is long enough to exceed content_width.
    // tui-markdown emits the marker `1. ` as a small span and
    // the body as a separate (larger) span; the wrap slow path
    // used to flush the marker onto its own line before the
    // body could attach.
    let body = "1. some text that is just long enough to need wrapping";
    let t = turn(TurnRole::Assistant, TurnKind::Message, body);
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 30, ToolDetail::Full, inner);
    let texts = flat_text(&out);
    // The list marker must share a line with at least the
    // first body word — no `"        you │ 1."` then
    // `"            │ some text..."`.
    let first = &texts[0];
    let last_pipe = first.rfind('│').expect("separator present");
    let after = &first[last_pipe + '│'.len_utf8()..];
    assert!(
        after.trim().starts_with("1.")
            && after
                .trim()
                .chars()
                .skip("1.".len())
                .any(|c| !c.is_whitespace()),
        "marker should not be alone on the first line: {first:?}"
    );
}

#[test]
fn categorize_known_claude_and_codex_tool_names() {
    assert_eq!(categorize_tool_name("Read"), ToolCategory::Read);
    assert_eq!(categorize_tool_name("read"), ToolCategory::Read);
    assert_eq!(categorize_tool_name("Bash"), ToolCategory::Shell);
    assert_eq!(categorize_tool_name("exec_command"), ToolCategory::Shell);
    assert_eq!(categorize_tool_name("Edit"), ToolCategory::Edit);
    assert_eq!(categorize_tool_name("MultiEdit"), ToolCategory::Edit);
    assert_eq!(categorize_tool_name("Write"), ToolCategory::Edit);
    assert_eq!(categorize_tool_name("Glob"), ToolCategory::Search);
    assert_eq!(categorize_tool_name("Grep"), ToolCategory::Search);
    assert_eq!(categorize_tool_name("WebFetch"), ToolCategory::Web);
    assert_eq!(categorize_tool_name("web_search"), ToolCategory::Web);
    assert_eq!(categorize_tool_name("TodoWrite"), ToolCategory::Other);
}

#[test]
fn tool_name_extracted_from_name_colon_args_body() {
    assert_eq!(tool_name_from_body("Read: /path/to/file"), "Read");
    assert_eq!(tool_name_from_body("Bash"), "Bash");
    assert_eq!(
        tool_name_from_body("exec_command: {\"cmd\":\"ls\"}"),
        "exec_command"
    );
}

#[test]
fn aggregate_phrase_matches_claude_history_examples() {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<ToolCategory, usize> = BTreeMap::new();
    counts.insert(ToolCategory::Read, 2);
    counts.insert(ToolCategory::Shell, 2);
    assert_eq!(
        aggregate_tool_phrase(&counts),
        "Read 2 files, ran 2 shell commands"
    );

    let mut counts: BTreeMap<ToolCategory, usize> = BTreeMap::new();
    counts.insert(ToolCategory::Shell, 1);
    assert_eq!(aggregate_tool_phrase(&counts), "Ran 1 shell command");

    let mut counts: BTreeMap<ToolCategory, usize> = BTreeMap::new();
    counts.insert(ToolCategory::Edit, 1);
    assert_eq!(aggregate_tool_phrase(&counts), "Edited 1 file");

    let mut counts: BTreeMap<ToolCategory, usize> = BTreeMap::new();
    counts.insert(ToolCategory::Read, 2);
    counts.insert(ToolCategory::Shell, 4);
    counts.insert(ToolCategory::Edit, 2);
    assert_eq!(
        aggregate_tool_phrase(&counts),
        "Read 2 files, ran 4 shell commands, edited 2 files"
    );
}

#[test]
fn empty_body_still_renders_the_chip() {
    let t = turn(TurnRole::User, TurnKind::Message, "");
    let theme = Theme::default();
    let inner = 8;
    let out = render_turn(&t, &theme, 40, ToolDetail::Full, inner);
    // Empty body yields just the chip line; no spacer (the
    // alternative would waste vertical space for what is
    // already a degenerate turn).
    assert_eq!(out.len(), 1);
    assert!(
        flat_text(&out)[0].contains(&format!("{} │", chip_pill("you", inner))),
        "got {:?}",
        flat_text(&out)[0]
    );
}
