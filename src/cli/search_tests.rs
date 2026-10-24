use super::*;

fn info(label: Option<&str>) -> SessionInfo {
    SessionInfo {
        harness: "claude-code".into(),
        session: "abc123".into(),
        label: label.map(str::to_string),
    }
}

fn hit(turn: usize, text: &str, range: std::ops::Range<usize>) -> Hit {
    Hit {
        turn_index: turn,
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        timestamp: DateTime::from_timestamp(1_700_000_000, 0),
        snippet: text.to_string(),
        highlight: range,
    }
}

fn found(group: Grouping, rows: Vec<Row>) -> Found {
    Found {
        group,
        kinds: ContentKinds::default(),
        progress: Progress {
            scanned: 3,
            total: 4,
            skipped: 1,
        },
        parserless: 2,
        rows,
    }
}

fn session_row(
    label: Option<&str>,
    count: usize,
    text: &str,
    range: std::ops::Range<usize>,
) -> Row {
    let best = hit(0, text, range);
    Row::Session {
        info: info(label),
        hits: SessionHits {
            count,
            latest: best.timestamp,
            best,
        },
    }
}

const NOW: i64 = 1_700_000_000 + 2 * 3600;

#[test]
fn the_session_table_aligns_columns_and_ends_with_the_snippet() {
    let f = found(
        Grouping::Session,
        vec![
            session_row(Some("Fix the parser"), 12, "found the needle here", 10..16),
            session_row(None, 1, "another needle", 8..14),
        ],
    );
    let text = render_table(&f, None, false, NOW);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("SESSION"), "{text}");
    assert!(lines[0].ends_with("SNIPPET"), "{text}");
    assert!(lines[1].contains("Fix the parser"), "{text}");
    assert!(lines[1].contains("12"), "{text}");
    assert!(lines[1].contains("2h"), "{text}");
    assert!(
        lines[2].contains("abc123"),
        "an unlabelled session shows its id: {text}"
    );
    let snippet_col = lines[0].find("SNIPPET").unwrap();
    assert_eq!(lines[1].find("found").unwrap(), snippet_col, "{text}");
    assert_eq!(lines[2].find("another").unwrap(), snippet_col, "{text}");
}

#[test]
fn the_message_table_shows_turn_role_kind_and_time() {
    let f = found(
        Grouping::Message,
        vec![Row::Message {
            info: info(Some("Fix the parser")),
            hit: Hit {
                kind: TurnKind::ToolResult,
                ..hit(7, "the needle", 4..10)
            },
        }],
    );
    let text = render_table(&f, None, false, NOW);
    let header = text.lines().next().unwrap();
    for column in [
        "SESSION", "HARNESS", "TURN", "ROLE", "KIND", "TIME", "SNIPPET",
    ] {
        assert!(header.contains(column), "{header}");
    }
    let row = text.lines().nth(1).unwrap();
    assert!(row.contains("#7"), "{row}");
    assert!(row.contains("assistant"), "{row}");
    assert!(row.contains("tool result"), "{row}");
}

#[test]
fn no_rows_print_nothing() {
    assert_eq!(
        render_table(&found(Grouping::Session, vec![]), None, false, NOW),
        ""
    );
}

#[test]
fn a_width_cuts_only_the_snippet_and_never_below_a_floor() {
    let long = "x".repeat(200);
    let f = found(
        Grouping::Session,
        vec![session_row(Some("s"), 1, &long, 0..1)],
    );
    let text = render_table(&f, Some(60), false, NOW);
    let row = text.lines().nth(1).unwrap();
    assert!(display_width(row) <= 60, "{row}");
    assert!(row.ends_with('…'), "{row}");
    let tiny = render_table(&f, Some(5), false, NOW);
    let snippet = tiny.lines().nth(1).unwrap().rsplit("  ").next().unwrap();
    assert!(display_width(snippet) >= MIN_SNIPPET_WIDTH, "{tiny}");
}

#[test]
fn color_bolds_the_match_only_when_it_survives_the_cut() {
    let text = "the quick brown needle jumps";
    let range = 16..22;
    let plain = fit_snippet(text, &range, None, false);
    assert_eq!(plain, text);
    let bold = fit_snippet(text, &range, None, true);
    assert!(bold.contains("\x1b[1mneedle\x1b[0m"), "{bold:?}");
    let cut = fit_snippet(text, &range, Some(12), true);
    assert!(!cut.contains('\x1b'), "the match was cut off: {cut:?}");
    assert!(cut.ends_with('…'));
    let empty = fit_snippet(text, &(0..0), None, true);
    assert_eq!(empty, text);
}

#[test]
fn json_documents_the_query_grouping_kinds_and_counts() {
    let f = found(
        Grouping::Session,
        vec![session_row(Some("Fix"), 2, "a needle", 2..8)],
    );
    let value: Value = serde_json::from_str(&render_json("  needle ", &f).unwrap()).unwrap();
    assert_eq!(value["query"], "needle");
    assert_eq!(value["group"], "session");
    assert_eq!(value["kinds"], json!(["messages"]));
    assert_eq!(
        value["searched"],
        json!({"sessions": 3, "total": 4, "skipped": 1, "without_parser": 2})
    );
    let result = &value["results"][0];
    assert_eq!(result["hits"], 2);
    assert_eq!(result["label"], "Fix");
    assert_eq!(result["latest"], "2023-11-14T22:13:20Z");
    assert_eq!(result["snippet"]["highlight"], json!([2, 8]));
}

#[test]
fn json_per_message_rows_carry_the_turn() {
    let f = found(
        Grouping::Message,
        vec![Row::Message {
            info: info(None),
            hit: hit(3, "x needle", 2..8),
        }],
    );
    let value: Value = serde_json::from_str(&render_json("needle", &f).unwrap()).unwrap();
    let result = &value["results"][0];
    assert_eq!(result["turn"], 3);
    assert_eq!(result["role"], "assistant");
    assert_eq!(result["kind"], "message");
    assert_eq!(result["label"], Value::Null);
    assert!(result.get("hits").is_none());
}

#[test]
fn the_width_flag_wins_and_a_pipe_is_unbounded() {
    assert_eq!(table_width(Some(90), false), Some(90));
    assert_eq!(table_width(Some(90), true), Some(90));
    assert_eq!(table_width(None, false), None);
}
