use chrono::TimeZone;

use super::*;
use crate::viewer::model::{TranscriptMeta, TranscriptTurn};

fn turn(role: TurnRole, kind: TurnKind, body: &str) -> TranscriptTurn {
    TranscriptTurn {
        role,
        kind,
        body: body.to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    }
}

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 1, hour, 0, 0).unwrap()
}

fn doc(turns: Vec<TranscriptTurn>) -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta::default(),
        turns,
    }
}

fn query(text: &str) -> ContentQuery {
    ContentQuery::new(text, ContentKinds::default())
}

fn hit(turn_index: usize, kind: TurnKind, timestamp: Option<DateTime<Utc>>) -> Hit {
    Hit {
        turn_index,
        role: TurnRole::Assistant,
        kind,
        timestamp,
        snippet: "x".into(),
        highlight: 0..1,
    }
}

#[test]
fn matching_ignores_ascii_case_in_both_directions() {
    assert_eq!(find_ci("The Quick Brown Fox", "quick"), Some(4));
    assert_eq!(find_ci("the quick brown fox", "QUICK"), Some(4));
    assert_eq!(find_ci("abc", "abcd"), None);
    assert_eq!(find_ci("", "a"), None);
    assert_eq!(find_ci("abc", ""), None);
}

#[test]
fn non_ascii_text_matches_exactly() {
    assert_eq!(
        find_ci("Café au lait", "CAFÉ"),
        None,
        "É is not folded to é"
    );
    assert_eq!(
        find_ci("Café au lait", "CAFé"),
        Some(0),
        "ASCII letters still fold"
    );
    assert_eq!(find_ci("naïve Straße", "Straße"), Some(7));
    assert_eq!(find_ci("STRASSE", "straße"), None);
}

#[test]
fn a_match_always_starts_and_ends_on_a_character_boundary() {
    let text = "日本語のテキスト and 😀 emoji";
    for needle in ["本語", "😀", "emoji", "テキスト"] {
        let start = find_ci(text, needle).expect(needle);
        assert!(text.is_char_boundary(start), "{needle}");
        assert!(text.is_char_boundary(start + needle.len()), "{needle}");
    }
}

#[test]
fn an_empty_or_blank_query_matches_nothing() {
    let d = doc(vec![turn(TurnRole::User, TurnKind::Message, "hello")]);
    assert!(match_document(&d, &query("")).is_empty());
    assert!(match_document(&d, &query("   \n")).is_empty());
}

#[test]
fn the_query_is_trimmed_but_matched_as_one_substring() {
    let d = doc(vec![turn(
        TurnRole::User,
        TurnKind::Message,
        "please fix the flaky test",
    )]);
    assert_eq!(match_document(&d, &query("  flaky test \n")).len(), 1);
    assert!(match_document(&d, &query("flaky   test")).is_empty());
}

#[test]
fn a_hit_carries_the_turn_role_kind_time_and_a_highlight() {
    let mut t = turn(
        TurnRole::Assistant,
        TurnKind::Message,
        "Use the Frobnicator.",
    );
    t.timestamp = Some(at(9));
    let d = doc(vec![turn(TurnRole::User, TurnKind::Message, "hi"), t]);

    let hits = match_document(&d, &query("frobnicator"));

    let [h] = hits.as_slice() else {
        panic!("{hits:?}");
    };
    assert_eq!(h.turn_index, 1);
    assert_eq!(h.role, TurnRole::Assistant);
    assert_eq!(h.kind, TurnKind::Message);
    assert_eq!(h.timestamp, Some(at(9)));
    assert_eq!(&h.snippet[h.highlight.clone()], "Frobnicator");
}

#[test]
fn a_turn_yields_one_hit_at_its_first_match() {
    let d = doc(vec![turn(
        TurnRole::User,
        TurnKind::Message,
        "alpha beta alpha gamma alpha",
    )]);
    let hits = match_document(&d, &query("alpha"));
    assert_eq!(hits.len(), 1);
    assert!(hits[0].snippet.starts_with("alpha beta"));
}

#[test]
fn by_default_only_user_and_assistant_messages_and_summaries_match() {
    let d = doc(vec![
        turn(TurnRole::User, TurnKind::Message, "needle user"),
        turn(TurnRole::Assistant, TurnKind::Message, "needle assistant"),
        turn(
            TurnRole::Assistant,
            TurnKind::CompactionSummary,
            "needle summary",
        ),
        turn(TurnRole::Assistant, TurnKind::ToolUse, "needle tool call"),
        turn(TurnRole::User, TurnKind::ToolResult, "needle tool output"),
        turn(TurnRole::Assistant, TurnKind::Thinking, "needle thought"),
        turn(TurnRole::System, TurnKind::Message, "needle system"),
    ]);

    let indices: Vec<usize> = match_document(&d, &query("needle"))
        .iter()
        .map(|h| h.turn_index)
        .collect();

    assert_eq!(indices, [0, 1, 2]);
}

#[test]
fn tools_and_thinking_are_opt_in() {
    let d = doc(vec![
        turn(TurnRole::User, TurnKind::Message, "needle"),
        turn(TurnRole::Assistant, TurnKind::ToolUse, "needle"),
        turn(TurnRole::User, TurnKind::ToolResult, "needle"),
        turn(TurnRole::Assistant, TurnKind::Thinking, "needle"),
    ]);
    let search = |kinds: ContentKinds| -> Vec<usize> {
        match_document(&d, &ContentQuery::new("needle", kinds))
            .iter()
            .map(|h| h.turn_index)
            .collect()
    };

    let tools = ContentKinds {
        tools: true,
        ..ContentKinds::default()
    };
    let thinking = ContentKinds {
        thinking: true,
        ..ContentKinds::default()
    };
    let only_thinking = ContentKinds {
        messages: false,
        thinking: true,
        ..ContentKinds::default()
    };

    assert_eq!(search(tools), [0, 1, 2]);
    assert_eq!(search(thinking), [0, 3]);
    assert_eq!(search(only_thinking), [3]);
}

#[test]
fn aborted_and_hidden_turns_are_excluded_unless_asked_for() {
    let mut aborted = turn(TurnRole::User, TurnKind::Message, "needle aborted");
    aborted.aborted = true;
    let mut hidden = turn(TurnRole::User, TurnKind::Message, "needle hidden");
    hidden.hidden = true;
    let d = doc(vec![
        turn(TurnRole::User, TurnKind::Message, "needle plain"),
        aborted,
        hidden,
    ]);
    let search =
        |kinds: ContentKinds| match_document(&d, &ContentQuery::new("needle", kinds)).len();

    assert_eq!(search(ContentKinds::default()), 1);
    assert_eq!(
        search(ContentKinds {
            aborted: true,
            ..ContentKinds::default()
        }),
        2
    );
    assert_eq!(
        search(ContentKinds {
            hidden: true,
            ..ContentKinds::default()
        }),
        2
    );
    assert_eq!(
        search(ContentKinds {
            aborted: true,
            hidden: true,
            ..ContentKinds::default()
        }),
        3
    );
}

#[test]
fn a_snippet_is_capped_and_keeps_the_match_with_markers_on_cut_sides() {
    let body = format!("{} needle {}", "before ".repeat(80), "after ".repeat(80));
    let d = doc(vec![turn(TurnRole::User, TurnKind::Message, &body)]);

    let hits = match_document(&d, &query("needle"));

    let snippet = &hits[0].snippet;
    assert!(
        snippet.chars().count() <= SNIPPET_MAX_CHARS + 2,
        "{} chars",
        snippet.chars().count()
    );
    assert!(
        snippet.starts_with('…') && snippet.ends_with('…'),
        "{snippet}"
    );
    assert_eq!(&snippet[hits[0].highlight.clone()], "needle");
}

#[test]
fn a_match_near_the_start_is_not_given_a_leading_marker() {
    let body = format!("needle {}", "tail ".repeat(100));
    let d = doc(vec![turn(TurnRole::User, TurnKind::Message, &body)]);
    let hits = match_document(&d, &query("needle"));
    assert!(!hits[0].snippet.starts_with('…'));
    assert!(hits[0].snippet.ends_with('…'));
    assert_eq!(hits[0].highlight.start, 0);
}

#[test]
fn a_match_deep_in_a_huge_body_is_still_cheap_and_correct() {
    let body = format!("{}needle{}", "x".repeat(2_000_000), "y".repeat(2_000_000));
    let d = doc(vec![turn(TurnRole::Assistant, TurnKind::ToolResult, &body)]);
    let kinds = ContentKinds {
        tools: true,
        ..ContentKinds::default()
    };

    let hits = match_document(&d, &ContentQuery::new("needle", kinds));

    assert_eq!(hits.len(), 1);
    assert!(hits[0].snippet.chars().count() <= SNIPPET_MAX_CHARS + 2);
    assert_eq!(&hits[0].snippet[hits[0].highlight.clone()], "needle");
}

#[test]
fn snippets_collapse_whitespace_and_drop_control_sequences() {
    let body = "line one\n\n  line\ttwo \u{1b}[31mred\u{1b}[0m \u{1b}]0;title\u{7}done\u{0}\u{8}";
    let d = doc(vec![turn(TurnRole::User, TurnKind::Message, body)]);

    let hits = match_document(&d, &query("two"));

    assert_eq!(hits[0].snippet, "line one line two red done");
    assert_eq!(&hits[0].snippet[hits[0].highlight.clone()], "two");
}

#[test]
fn a_match_inside_an_escape_free_span_survives_surrounding_escapes() {
    let body = "\u{1b}[1mbold\u{1b}[0m needle \u{1b}[4munderlined\u{1b}[0m";
    let d = doc(vec![turn(TurnRole::Assistant, TurnKind::Message, body)]);
    let hits = match_document(&d, &query("needle"));
    assert_eq!(hits[0].snippet, "bold needle underlined");
    assert_eq!(&hits[0].snippet[hits[0].highlight.clone()], "needle");
}

/// A deterministic generator, so the sweep below is reproducible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound
    }
}

#[test]
fn snippets_and_highlights_hold_their_invariants_over_generated_text() {
    // One to four bytes per character, whitespace the snippet collapses,
    // and an escape sequence that must never be cut or shown.
    const FILLER: [&str; 11] = [
        "a",
        "Z",
        "é",
        "日",
        "😀",
        " ",
        "\n",
        "\t",
        "\u{1b}[31m",
        "\u{1b}[0m",
        "word ",
    ];
    // What a query is made of: no whitespace, so the snippet's collapsing
    // of it can't change the match.
    const NEEDLE: [&str; 6] = ["n", "Q", "ñ", "日", "😀", "x"];
    let mut rng = Lcg(0x5eed);
    for case in 0..2000 {
        let mut body = String::new();
        let before = rng.next(400);
        let after = rng.next(400);
        for _ in 0..before {
            body.push_str(FILLER[rng.next(FILLER.len())]);
        }
        let needle: String = (0..1 + rng.next(4))
            .map(|_| NEEDLE[rng.next(NEEDLE.len())])
            .collect();
        body.push_str(&needle);
        for _ in 0..after {
            body.push_str(FILLER[rng.next(FILLER.len())]);
        }
        let turn = turn(TurnRole::User, TurnKind::Message, &body);
        let hit = match_turn(0, &turn, &needle)
            .unwrap_or_else(|| panic!("case {case}: the needle is in the body"));
        let (snippet, range) = (&hit.snippet, hit.highlight.clone());
        assert!(
            snippet.is_char_boundary(range.start) && snippet.is_char_boundary(range.end),
            "case {case}: {snippet:?} {range:?}"
        );
        assert!(
            snippet.chars().count() <= SNIPPET_MAX_CHARS + 2,
            "case {case}: {} chars",
            snippet.chars().count()
        );
        assert!(
            !snippet.contains('\u{1b}') && !snippet.contains('['),
            "case {case}: a cut escape or control survived: {snippet:?}"
        );
        assert!(
            !snippet.contains(['\n', '\t']),
            "case {case}: whitespace isn't collapsed: {snippet:?}"
        );
        // The highlight is a match, though not always the first in the
        // body (a different one can sit in the window), so it names the
        // needle by value.
        assert!(
            snippet[range.clone()].eq_ignore_ascii_case(&needle),
            "case {case}: {:?} is not {needle:?} in {snippet:?}",
            &snippet[range]
        );
    }
}

#[test]
fn the_highlight_is_the_match_the_viewer_opens_on() {
    // The viewer opens on the first matching turn and jumps to the first
    // match in it; the snippet's highlight is that match, not a later
    // one the window happened to show.
    let body = format!(
        "{} needle one {} needle two",
        "pad ".repeat(60),
        "pad ".repeat(60)
    );
    let hit = match_turn(0, &turn(TurnRole::User, TurnKind::Message, &body), "needle").unwrap();
    assert!(hit.snippet.contains("needle one"), "{}", hit.snippet);
    assert_eq!(
        &hit.snippet[hit.highlight.clone()],
        "needle",
        "{}",
        hit.snippet
    );
    let after_highlight = &hit.snippet[hit.highlight.end..];
    assert!(
        after_highlight.trim_start().starts_with("one"),
        "{after_highlight}"
    );
}

#[test]
fn matching_turns_agrees_with_the_hits_without_building_snippets() {
    let d = doc(vec![
        turn(TurnRole::User, TurnKind::Message, "a needle"),
        turn(TurnRole::Assistant, TurnKind::ToolUse, "needle in a tool"),
        turn(TurnRole::Assistant, TurnKind::Message, "nothing"),
        turn(TurnRole::Assistant, TurnKind::Message, "NEEDLE again"),
    ]);
    let q = query("needle");
    let from_hits: Vec<_> = match_document(&d, &q)
        .iter()
        .map(|h| h.turn_index)
        .collect();
    assert_eq!(matching_turns(&d, &q).collect::<Vec<_>>(), from_hits);
    assert_eq!(from_hits, [0, 3], "the tool call is out of scope");
    assert_eq!(matching_turns(&d, &query("  ")).count(), 0);
}

#[test]
fn a_session_summary_built_from_a_document_matches_one_built_from_its_hits() {
    let d = doc(vec![
        turn(TurnRole::User, TurnKind::Message, "needle one"),
        turn(TurnRole::Assistant, TurnKind::Message, "needle two"),
    ]);
    let q = query("needle");
    let direct = SessionHits::from_document(&d, &q).unwrap();
    let via_hits = SessionHits::from_hits(&match_document(&d, &q)).unwrap();
    assert_eq!(direct.count, via_hits.count);
    assert_eq!(direct.best, via_hits.best);
    assert!(SessionHits::from_document(&d, &query("absent")).is_none());
}

#[test]
fn hidden_system_messages_match_only_when_hidden_text_is_asked_for() {
    let mut note = turn(TurnRole::System, TurnKind::Message, "model: acme/turbo-9");
    note.hidden = true;
    let d = doc(vec![note]);
    assert!(match_document(&d, &query("turbo")).is_empty());
    let kinds = ContentKinds {
        hidden: true,
        ..ContentKinds::default()
    };
    assert_eq!(
        match_document(&d, &ContentQuery::new("turbo", kinds)).len(),
        1
    );
}

#[test]
fn ranges_that_are_not_on_boundaries_or_outside_the_text_yield_no_highlight() {
    let text = "naïve 日本語";
    let inside_char = 3..4; // splits the two bytes of `ï`
    for range in [inside_char, 0..0, 5..500, 40..41] {
        let snippet = snippet_around(text, range.clone(), 100);
        assert_eq!(snippet.highlight, None, "{range:?}");
        let cut = snippet_around(&text.repeat(30), range.clone(), 20);
        assert_eq!(cut.highlight, None, "{range:?}");
    }
    let ok = snippet_around(text, 0..1, 100);
    assert_eq!(ok.highlight, Some(0..1));
}

#[test]
fn a_match_made_only_of_control_characters_highlights_nothing() {
    let d = doc(vec![turn(
        TurnRole::User,
        TurnKind::Message,
        "a\u{1}\u{2}b",
    )]);
    let hits = match_document(&d, &query("\u{1}\u{2}"));
    assert_eq!(hits.len(), 1);
    assert!(hits[0].highlight.is_empty());
}

#[test]
fn grouping_parses_the_config_words() {
    assert_eq!(Grouping::parse("session"), Some(Grouping::Session));
    assert_eq!(Grouping::parse(" message "), Some(Grouping::Message));
    assert_eq!(Grouping::parse("turn"), None);
    assert_eq!(Grouping::default(), Grouping::Session);
    assert_eq!(Grouping::Message.as_str(), "message");
}

#[test]
fn a_session_aggregates_count_latest_and_a_best_message_hit() {
    let hits = [
        hit(7, TurnKind::ToolResult, Some(at(12))),
        hit(4, TurnKind::Message, Some(at(9))),
        hit(2, TurnKind::Thinking, Some(at(15))),
        hit(9, TurnKind::Message, Some(at(10))),
    ];

    let agg = SessionHits::from_hits(&hits).unwrap();

    assert_eq!(agg.count, 4);
    assert_eq!(agg.latest, Some(at(15)));
    assert_eq!(
        agg.best.turn_index, 4,
        "earliest message beats tool and thinking"
    );
    assert!(SessionHits::from_hits(&[]).is_none());
}

#[test]
fn a_session_with_no_timestamps_has_no_latest_hit() {
    let agg = SessionHits::from_hits(&[hit(1, TurnKind::Message, None)]).unwrap();
    assert_eq!(agg.latest, None);
}

fn session(count: usize, latest: Option<DateTime<Utc>>) -> SessionHits {
    SessionHits {
        count,
        best: hit(0, TurnKind::Message, latest),
        latest,
    }
}

#[test]
fn sessions_rank_by_hit_count_then_latest_hit() {
    let mut entries = [
        ("few-new", session(1, Some(at(20)))),
        ("many-old", session(5, Some(at(1)))),
        ("many-new", session(5, Some(at(9)))),
        ("many-undated", session(5, None)),
    ];

    entries.sort_by(|a, b| cmp_sessions(&a.1, &b.1));

    let order: Vec<_> = entries.iter().map(|e| e.0).collect();
    assert_eq!(order, ["many-new", "many-old", "many-undated", "few-new"]);
}

#[test]
fn recency_ranking_puts_the_latest_hit_first_whatever_the_count() {
    let mut entries = [
        ("many-old", session(9, Some(at(1)))),
        ("few-new", session(1, Some(at(20)))),
        ("undated", session(50, None)),
    ];

    entries.sort_by(|a, b| cmp_sessions_by_recency(&a.1, &b.1));

    let order: Vec<_> = entries.iter().map(|e| e.0).collect();
    assert_eq!(order, ["few-new", "many-old", "undated"]);
}

#[test]
fn messages_rank_newest_first_with_ties_in_turn_order() {
    let mut hits = [
        hit(5, TurnKind::Message, Some(at(9))),
        hit(2, TurnKind::Message, Some(at(9))),
        hit(8, TurnKind::Message, Some(at(11))),
        hit(1, TurnKind::Message, None),
    ];

    hits.sort_by(cmp_hits);

    let order: Vec<_> = hits.iter().map(|h| h.turn_index).collect();
    assert_eq!(order, [8, 2, 5, 1]);
}
