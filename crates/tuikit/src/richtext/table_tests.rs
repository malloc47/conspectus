use super::*;

#[test]
fn segment_body_with_no_table_returns_one_markdown_segment() {
    let body = "just prose\n\nwith two paragraphs";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 1);
    match &segs[0] {
        BodySegment::Markdown(s) => assert_eq!(*s, body),
        BodySegment::Table(_) => panic!("expected markdown segment"),
    }
}

#[test]
fn segment_body_splits_prose_around_a_table() {
    let body = "intro line\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\noutro";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 3, "expected md + table + md, got {segs:?}");
    match &segs[0] {
        BodySegment::Markdown(s) => assert!(s.starts_with("intro line"), "got {s:?}"),
        BodySegment::Table(_) => panic!(),
    }
    match &segs[1] {
        BodySegment::Table(t) => {
            assert_eq!(t.header, vec!["a", "b"]);
            assert_eq!(t.rows, vec![vec!["1".to_string(), "2".to_string()]]);
        }
        BodySegment::Markdown(_) => panic!(),
    }
    match &segs[2] {
        BodySegment::Markdown(s) => assert!(s.trim_start().starts_with("outro"), "got {s:?}"),
        BodySegment::Table(_) => panic!(),
    }
}

#[test]
fn segment_body_ignores_pipes_inside_fenced_code_block() {
    let body = "```\n| not | a | table |\n|---|---|---|\n| x | y | z |\n```\n";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 1, "fenced pipes stay as markdown: {segs:?}");
    assert!(matches!(segs[0], BodySegment::Markdown(_)));
}

#[test]
fn segment_body_handles_table_at_end_of_body() {
    let body = "intro\n\n| a | b |\n|---|---|\n| 1 | 2 |";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 2);
    assert!(matches!(segs[1], BodySegment::Table(_)));
}

#[test]
fn segment_body_handles_table_at_start_of_body() {
    let body = "| a | b |\n|---|---|\n| 1 | 2 |\n\ntail";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 2, "got {segs:?}");
    assert!(matches!(segs[0], BodySegment::Table(_)));
}

#[test]
fn segment_body_rejects_separator_row_without_dashes() {
    // A row of all colons isn't a separator; it's body content.
    let body = "| a | b |\n|:::|:::|\n| 1 | 2 |";
    let segs = segment_body(body);
    assert_eq!(segs.len(), 1);
    assert!(matches!(segs[0], BodySegment::Markdown(_)));
}

#[test]
fn split_pipe_row_strips_outer_pipes_and_trims_cells() {
    let cells = split_pipe_row("|  a  |  b  |").unwrap();
    assert_eq!(cells, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn split_pipe_row_handles_missing_trailing_pipe() {
    let cells = split_pipe_row("| a | b").unwrap();
    assert_eq!(cells, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn parse_separator_row_extracts_alignment_markers() {
    // |:--|---|---:|:---:|
    let aligns = parse_separator_row("|:--|---|---:|:---:|", 4).unwrap();
    assert_eq!(
        aligns,
        vec![
            TableAlign::Left,
            TableAlign::Left,
            TableAlign::Right,
            TableAlign::Center
        ]
    );
}

#[test]
fn parse_separator_row_rejects_count_mismatch() {
    assert!(parse_separator_row("|---|---|", 3).is_none());
}

#[test]
fn try_parse_pipe_table_tolerates_column_count_drift() {
    let lines = vec![
        "| a | b | c |",
        "|---|---|---|",
        "| 1 | 2 |",         // short row — should pad
        "| x | y | z | w |", // long row — should truncate
    ];
    let (parsed, consumed) = try_parse_pipe_table(&lines, 0).expect("parse");
    assert_eq!(consumed, 4);
    assert_eq!(parsed.header, vec!["a", "b", "c"]);
    assert_eq!(
        parsed.rows[0],
        vec!["1".to_string(), "2".to_string(), String::new()]
    );
    assert_eq!(
        parsed.rows[1],
        vec!["x".to_string(), "y".to_string(), "z".to_string()]
    );
}

#[test]
fn render_table_produces_borders_at_target_width() {
    let parsed = ParsedTable {
        header: vec!["col1".into(), "col2".into()],
        alignments: vec![TableAlign::Left, TableAlign::Left],
        rows: vec![
            vec!["alpha".into(), "beta".into()],
            vec!["gamma".into(), "delta".into()],
        ],
    };
    let theme = Theme::default();
    let lines = render_table(&parsed, 40, &theme);
    let text: Vec<String> = lines.iter().map(line_to_string).collect();
    assert!(
        text.iter()
            .any(|l| l.contains("col1") && l.contains("col2")),
        "header row present: {text:?}"
    );
    assert!(
        text.iter()
            .any(|l| l.contains("alpha") && l.contains("beta")),
        "body row present: {text:?}"
    );
    assert!(
        text.iter().any(|l| l.contains('┆')),
        "column separators present (UTF8_NO_BORDERS uses ┆): {text:?}"
    );
}

#[test]
fn render_table_wraps_wide_cells_to_fit_target_width() {
    let parsed = ParsedTable {
        header: vec!["short".into(), "long content column".into()],
        alignments: vec![TableAlign::Left, TableAlign::Left],
        rows: vec![vec![
            "x".into(),
            "this is a very long sentence that needs wrapping across multiple lines".into(),
        ]],
    };
    let theme = Theme::default();
    let lines = render_table(&parsed, 30, &theme);
    for line in &lines {
        let width: usize = line
            .spans
            .iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
            .sum();
        assert!(
            width <= 30,
            "every line stays within target width 30; got {width} from {:?}",
            line_to_string(line)
        );
    }
}

fn line_to_string(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
