use super::*;
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use std::path::PathBuf;

fn sample_document() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "0b34e59c".to_string(),
            cwd: Some("/p/proj".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "what's up?".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "not much".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
        ],
    }
}

fn unavailable_document() -> TranscriptDocument {
    TranscriptDocument::unavailable(&SessionLocator {
        harness_key: "claude-code".to_string(),
        session_key: "missing".to_string(),
        state_root: PathBuf::from("/x"),
    })
}

fn long_document() -> TranscriptDocument {
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "long".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: Vec::new(),
    };
    for i in 0..30 {
        doc.turns.push(TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: format!("turn {i}"),
            timestamp: None,
            aborted: false,
            hidden: false,
        });
    }
    doc
}

fn document_with_compaction() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "compact".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "before".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::CompactionSummary,
                body: "summary of prior turns".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "after".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
        ],
    }
}

fn document_with_tools() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tools".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "what files are here?".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolUse,
                body: "Glob: **/*.rs".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolResult,
                body: "src/lib.rs\nsrc/main.rs".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "two Rust files in src/.".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
        ],
    }
}

/// Drop the trailing whitespace each row pads to width — keeps
/// snapshots terminal-width-agnostic to padding noise.
fn snapshot_string(buf: &ratatui::buffer::Buffer) -> String {
    let raw = buffer_to_string(buf);
    raw.lines()
        .map(|l| l.trim_end_matches(' '))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn normal_open_lands_on_last_turn_with_gutter_chips() {
    let doc = sample_document();
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let buf = render_to_buffer(&mut state, &theme, 60, 12);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn jump_to_start_shows_top_of_long_document() {
    let theme = Theme::default();
    let mut state = ViewerState::new(long_document());
    let _ = render_to_buffer(&mut state, &theme, 60, 10);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
    let buf = render_to_buffer(&mut state, &theme, 60, 10);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn empty_document_renders_unavailable_banner() {
    let theme = Theme::default();
    let mut state = ViewerState::new(unavailable_document());
    let buf = render_to_buffer(&mut state, &theme, 60, 8);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn compaction_summary_turn_renders_with_compact_chip() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_compaction());
    let buf = render_to_buffer(&mut state, &theme, 60, 14);
    insta::assert_snapshot!(snapshot_string(&buf));
}

fn document_with_table() -> TranscriptDocument {
    let body = "Three-way comparison:\n\n\
            | Option | Effort | Risk |\n\
            |---|---|---|\n\
            | A | Small | Low |\n\
            | B | Medium | Medium |\n\
            | C | Large | High |\n\n\
            Recommend B.";
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tbl".to_string(),
            cwd: Some("/proj".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
            hidden: false,
        }],
    }
}

#[test]
fn table_snapshot_at_80_cols_wide_enough_to_fit_naturally() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_table());
    let buf = render_to_buffer(&mut state, &theme, 80, 18);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn table_snapshot_at_40_cols_forces_dynamic_wrap() {
    // Use a fixture with cell content that exceeds the
    // available content width (terminal 40 minus 10-cell gutter
    // minus the 3-cell ` │ ` separator = 27 cells). comfy-table's
    // ContentArrangement::Dynamic should shrink the widest
    // column and wrap its cells.
    let body = "Comparison of long-form options:\n\n\
            | Option | Description |\n\
            |---|---|\n\
            | Alpha | a sufficiently detailed description that demands wrapping |\n\
            | Beta | shorter |";
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "wrap".to_string(),
            cwd: Some("/proj".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
            hidden: false,
        }],
    };
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let buf = render_to_buffer(&mut state, &theme, 40, 20);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn tools_visible_at_full_detail_show_call_and_result_chips() {
    let theme = Theme::default();
    let state = ViewerState::new(document_with_tools());
    // Cycle to Full detail (3 t's: Hidden → Sum → Trunc → Full)
    // so the ToolUse + ToolResult turns render with full bodies.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert_eq!(state.tool_detail, ToolDetail::Full);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
    let buf = render_to_buffer(&mut state, &theme, 60, 16);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn narrow_terminal_wraps_body_but_keeps_chip_alignment() {
    let theme = Theme::default();
    let mut state = ViewerState::new(TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "wrap".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: "one two three four five six seven eight nine ten eleven twelve thirteen"
                .to_string(),
            timestamp: None,
            aborted: false,
            hidden: false,
        }],
    });
    let buf = render_to_buffer(&mut state, &theme, 40, 12);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn footer_shows_position_and_toggle_state() {
    let theme = Theme::default();
    let state = ViewerState::new(sample_document());
    // Cycle tool to Summary; toggle thinking on.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleThinking);
    let buf = render_to_buffer(&mut state, &theme, 100, 8);
    let s = buffer_to_string(&buf);
    assert!(
        s.contains("tools·sum"),
        "footer reports tools detail (sum): {s}"
    );
    assert!(s.contains("think·on"), "footer reports thinking state: {s}");
    assert!(s.contains("(t)"), "tools chip carries the t key hint");
    assert!(s.contains("(T)"), "thinking chip carries the T key hint");
    assert!(s.contains("[ "), "footer carries position counter");
}

#[test]
fn the_viewer_leaves_drawing_its_help_to_the_host() {
    // The help page is drawn by the TUI with the main help's frame
    // (`tui::widgets::viewer_help`); the viewer itself draws only the
    // transcript, so an open help flag changes nothing here.
    let theme = Theme::default();
    let mut closed = ViewerState::new(sample_document());
    let mut open = ViewerState::new(sample_document());
    open.show_help = true;
    let without = snapshot_string(&render_to_buffer(&mut closed, &theme, 70, 18));
    let with = snapshot_string(&render_to_buffer(&mut open, &theme, 70, 18));
    assert_eq!(with, without);
}

#[test]
fn render_cache_persists_across_draws_until_toggle_invalidates() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_tools());
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache = state
        .rendered
        .as_ref()
        .expect("cache populated on first draw");
    let cached_lines = cache.lines.len();
    let cached_width = cache.content_width;

    // Second draw at the same width must hit the cache. We can't
    // *prove* it from the outside without instrumentation, but
    // we can at least assert the cache is still populated and
    // the metrics are stable.
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache_after = state.rendered.as_ref().expect("cache still up");
    assert_eq!(cache_after.lines.len(), cached_lines);
    assert_eq!(cache_after.content_width, cached_width);

    // Cycling tool detail must invalidate via the reducer.
    // The next draw rebuilds.
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert!(
        state.rendered.is_none(),
        "CycleToolDetail invalidates cache"
    );
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache_post = state.rendered.as_ref().expect("cache repopulated");
    assert_ne!(
        cache_post.tool_detail,
        ToolDetail::Hidden,
        "rebuilt cache reflects new tool detail"
    );
    assert!(
        cache_post.lines.len() > cached_lines,
        "non-Hidden tool detail adds tool turns; lines should grow ({} → {})",
        cached_lines,
        cache_post.lines.len()
    );
}

#[test]
fn render_cache_invalidates_on_width_change() {
    let theme = Theme::default();
    let mut state = ViewerState::new(sample_document());
    let _ = render_to_buffer(&mut state, &theme, 80, 12);
    let lines_at_80 = state.rendered.as_ref().unwrap().lines.len();
    let _ = render_to_buffer(&mut state, &theme, 40, 12);
    let cache = state.rendered.as_ref().unwrap();
    let inner = state.transcript.gutter_inner_width();
    let content_width_at_40 = 40u16 - leader_width(inner);
    assert_eq!(
        cache.content_width, content_width_at_40,
        "cache reports the latest content_width"
    );
    // Narrower terminals usually produce more wrapped lines but
    // may also be the same for short bodies; just confirm the
    // cache was rebuilt (different width key).
    let _ = lines_at_80;
}

#[test]
fn draw_writes_viewport_height_and_total_lines_to_state() {
    let theme = Theme::default();
    let mut state = ViewerState::new(sample_document());
    let _ = render_to_buffer(&mut state, &theme, 40, 12);
    assert_eq!(
        state.viewport_height, 8,
        "12 total - 4 chrome (header/2 seps/footer)"
    );
    assert!(state.total_lines > 0);
}

#[test]
fn cycle_tool_detail_makes_tool_turns_visible() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_tools());
    let before = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
    assert!(!before.contains("Glob:"), "tools hidden by default");

    // Cycle three times so we land on Full detail.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let after = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
    assert!(after.contains("Glob:"), "tool turns visible at Full detail");
    assert!(after.contains("tool  │"), "tool chip label visible");
}

#[test]
fn markdown_table_in_message_body_renders_as_aligned_table() {
    // Real-shape body modeled on a Claude assistant turn: a
    // brief prose intro, a GFM pipe-table, then a prose
    // outro. The table must render as an aligned block with
    // column separators (UTF8_BORDERS_ONLY preset) and the
    // surrounding prose must stay intact.
    let body = "Comparison of options:\n\n\
            | Option | Effort | Risk |\n\
            |---|---|---|\n\
            | A | Small | Low |\n\
            | B | Medium | Medium |\n\
            | C | Large | High |\n\n\
            Recommend B.";
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tbl".to_string(),
            cwd: None,
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
            hidden: false,
        }],
    };
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 20));
    assert!(s.contains("Comparison of options"), "prose intro: {s}");
    assert!(s.contains("Recommend B"), "prose outro: {s}");
    assert!(
        s.contains("Option") && s.contains("Effort") && s.contains("Risk"),
        "header cells present: {s}"
    );
    assert!(s.contains("Medium"), "body cell present: {s}");
    // UTF8_NO_BORDERS preset uses `┆` as the column separator
    // glyph; presence confirms aligned rendering rather than
    // raw pipe fallthrough. The gutter rule (`│`) appears
    // separately for every line, so we look specifically for the
    // table's dotted column separator.
    assert!(
        s.contains('┆'),
        "column separators rendered (not raw pipes): {s}"
    );
    // And the raw GFM separator row must not survive into the
    // output — that would mean tui-markdown got the table.
    assert!(
        !s.contains("|---|"),
        "raw pipe-separator row should not leak: {s}"
    );
}

#[test]
fn aborted_turns_hidden_by_default_visible_after_toggle() {
    let theme = Theme::default();
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "abrt".to_string(),
            cwd: None,
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "first prompt".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "first reply".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "interrupted thought".to_string(),
                timestamp: None,
                aborted: true,
                hidden: false,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "replacement prompt".to_string(),
                timestamp: None,
                aborted: false,
                hidden: false,
            },
        ],
    };
    let mut state = ViewerState::new(doc);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 14));
    assert!(
        s.contains("first prompt"),
        "non-aborted user msg shows: {s}"
    );
    assert!(s.contains("replacement prompt"), "next prompt shows");
    assert!(
        !s.contains("interrupted thought"),
        "aborted user msg hidden by default: {s}"
    );

    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleAborted);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 14));
    assert!(
        s.contains("interrupted thought"),
        "aborted msg revealed after toggle: {s}"
    );
}

#[test]
fn summary_tool_detail_aggregates_consecutive_tool_runs() {
    let theme = Theme::default();
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "agg".to_string(),
            cwd: None,
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: "do the thing".to_string(),
            timestamp: None,
            aborted: false,
            hidden: false,
        }],
    };
    // Run: Read x2, Bash x4, Edit x2 (each ToolUse + ToolResult).
    let categories = [("Read", 2), ("Bash", 4), ("Edit", 2)];
    for (name, count) in categories {
        for i in 0..count {
            doc.turns.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolUse,
                body: format!("{name}: arg{i}"),
                timestamp: None,
                aborted: false,
                hidden: false,
            });
            doc.turns.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolResult,
                body: format!("result {i}"),
                timestamp: None,
                aborted: false,
                hidden: false,
            });
        }
    }
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        body: "done".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    let state = ViewerState::new(doc);
    // Cycle once: Hidden → Summary.
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert_eq!(state.tool_detail, ToolDetail::Summary);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    assert!(s.contains("tool  │"), "tool chip present at Summary");
    // The aggregated phrase replaces 16 per-turn lines.
    assert!(
        s.contains("Read 2 files, ran 4 shell commands, edited 2 files"),
        "aggregate phrase visible: {s}"
    );
    // Surrounding non-tool turns still render normally.
    assert!(s.contains("do the thing"), "user message stays");
    assert!(s.contains("done"), "ai closing message stays");
    // No per-call chip body should leak through Summary —
    // operator wouldn't see individual "Bash: arg0" lines.
    assert!(
        !s.contains("Bash: arg"),
        "individual call args hidden in Summary: {s}"
    );
}

#[test]
fn summary_aggregation_separates_runs_around_messages() {
    let theme = Theme::default();
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "split".to_string(),
            cwd: None,
        },
        turns: Vec::new(),
    };
    // Run 1: Read x1.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolUse,
        body: "Read: file.rs".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolResult,
        body: "contents".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    // Message between the runs.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        body: "inspecting".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    // Run 2: Bash x1.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolUse,
        body: "Bash: ls".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolResult,
        body: "out".to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    });
    let state = ViewerState::new(doc);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    // Both aggregates appear (singular forms).
    assert!(s.contains("Read 1 file"), "first run aggregate: {s}");
    assert!(
        s.contains("Ran 1 shell command"),
        "second run aggregate: {s}"
    );
}

#[test]
fn transcripts_longer_than_u16_lines_scroll_to_the_end() {
    let body = (0..70_000)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut doc = sample_document();
    doc.turns = vec![TranscriptTurn {
        role: TurnRole::User,
        kind: TurnKind::Message,
        body,
        timestamp: None,
        aborted: false,
        hidden: false,
    }];
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let buf = render_to_buffer(&mut state, &theme, 60, 12);
    let text = snapshot_string(&buf);
    assert!(state.total_lines > usize::from(u16::MAX));
    assert!(
        text.contains("line 69999"),
        "the end of the transcript should be visible:\n{text}"
    );
}

#[test]
fn hidden_turns_are_filtered_until_toggled() {
    let theme = Theme::default();
    let turn = |body: &str, hidden| TranscriptTurn {
        role: if hidden {
            TurnRole::System
        } else {
            TurnRole::User
        },
        kind: TurnKind::Message,
        body: body.to_string(),
        timestamp: None,
        aborted: false,
        hidden,
    };
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "codex".to_string(),
            session_key: "hidden".to_string(),
            cwd: None,
        },
        turns: vec![
            turn("developer instructions", true),
            turn("visible prompt", false),
        ],
    };
    let mut state = ViewerState::new(doc);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    assert!(s.contains("visible prompt"), "{s}");
    assert!(
        !s.contains("developer instructions"),
        "hidden by default: {s}"
    );
    assert!(s.contains("hidden·hide (H)"), "footer chip: {s}");

    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleHidden);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    assert!(
        s.contains("developer instructions"),
        "revealed after H: {s}"
    );
    assert!(s.contains("hidden·show (H)"), "footer chip: {s}");
}

// ---------------------------------------------------------------------
// The transcript as the framework pager's content.
// ---------------------------------------------------------------------

mod content {
    use tuikit::pager::Pager;
    use tuikit::testkit::buffer;
    use tuikit::testkit::keys::script;

    use super::*;

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

    /// Twelve messages (turns 2, 5, and 9 mention "needle"), then a tool
    /// result and a thinking turn that mention it too.
    fn needles() -> TranscriptDocument {
        let mut turns: Vec<TranscriptTurn> = (0..12)
            .map(|i| {
                let role = if i % 2 == 0 {
                    TurnRole::User
                } else {
                    TurnRole::Assistant
                };
                let body = if [2, 5, 9].contains(&i) {
                    format!("turn {i} has the Needle in it")
                } else {
                    format!("turn {i} is plain")
                };
                turn(role, TurnKind::Message, &body)
            })
            .collect();
        turns.push(turn(
            TurnRole::Assistant,
            TurnKind::ToolResult,
            "output: a needle in tool output",
        ));
        turns.push(turn(
            TurnRole::Assistant,
            TurnKind::Thinking,
            "thinking about the needle",
        ));
        TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "find".to_string(),
                cwd: Some("/p".to_string()),
            },
            turns,
        }
    }

    fn pager(document: TranscriptDocument) -> Pager<Transcript> {
        Pager::new(Transcript::new(document)).at_end()
    }

    fn draw(pager: &mut Pager<Transcript>, width: u16, height: u16) -> String {
        let theme = Theme::default();
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        pager.draw(area, &mut buf, &theme);
        buffer::text(&buf)
    }

    fn press(pager: &mut Pager<Transcript>, keys: &str) {
        for key in script(keys) {
            let _ = pager.handle_key(key);
        }
    }

    fn first_line_of(pager: &Pager<Transcript>, turn: usize) -> usize {
        pager.content().rows_of(turn).start
    }

    #[test]
    fn the_pager_draws_the_header_the_rows_and_the_toggle_chips() {
        let mut viewer = pager(sample_document());
        let screen = draw(&mut viewer, 100, 12);
        let rows: Vec<&str> = screen.lines().collect();
        assert!(
            rows[0].starts_with(" claude-code:0b34e59c · /p/proj · 2 turns"),
            "{screen}"
        );
        assert!(screen.contains(" you  │ what's up?"), "{screen}");
        let footer = rows.last().unwrap();
        for part in [
            "tools·off   (t)",
            "think·off (T)",
            "aborted·hide (I)",
            "/ find",
            "? help",
            "Esc/q close",
        ] {
            assert!(footer.contains(part), "{part}: {footer}");
        }
        assert!(!footer.contains("hidden·"), "no hidden turns: {footer}");
    }

    #[test]
    fn a_header_too_wide_keeps_its_title() {
        let mut viewer = pager(sample_document());
        let screen = draw(&mut viewer, 26, 8);
        assert_eq!(
            screen.lines().next().unwrap().trim_end(),
            " claude-code:0b34e59c"
        );
    }

    #[test]
    fn it_opens_at_the_end_and_the_ends_move_it() {
        let mut viewer = pager(long_document());
        let screen = draw(&mut viewer, 60, 10);
        assert!(screen.contains("turn 29"), "{screen}");
        press(&mut viewer, "g");
        let screen = draw(&mut viewer, 60, 10);
        assert!(screen.contains("turn 0"), "{screen}");
        assert!(!screen.contains("turn 29"), "{screen}");
    }

    #[test]
    fn a_toggle_lays_the_rows_out_again() {
        let mut viewer = pager(document_with_tools());
        draw(&mut viewer, 60, 16);
        let before = viewer.content().rendered.as_ref().unwrap().lines.len();
        press(&mut viewer, "ttt");
        assert_eq!(viewer.content().tool_detail, ToolDetail::Full);
        let screen = draw(&mut viewer, 60, 16);
        let after = viewer.content().rendered.as_ref().unwrap().lines.len();
        assert!(after > before, "{before} → {after}");
        assert!(screen.contains("tools·all"), "{screen}");
        assert!(screen.contains("src/main.rs"), "{screen}");
        press(&mut viewer, "T");
        assert!(viewer.content().show_thinking);
        press(&mut viewer, "IH");
        assert!(viewer.content().show_aborted && viewer.content().show_hidden);
    }

    #[test]
    fn a_find_matches_shown_turns_and_lands_a_row_above_the_turn() {
        let mut viewer = pager(needles());
        draw(&mut viewer, 80, 8);
        press(&mut viewer, "g/needle<Enter>");
        let screen = draw(&mut viewer, 80, 8);
        assert_eq!(viewer.find().matches(), [2, 5, 9]);
        assert_eq!(viewer.find().current_unit(), Some(2));
        assert_eq!(viewer.window().offset(), first_line_of(&viewer, 2) - 1);
        assert!(screen.contains("find \"needle\" 1/3"), "{screen}");
        press(&mut viewer, "n");
        draw(&mut viewer, 80, 8);
        assert_eq!(viewer.find().current_unit(), Some(5));
        assert_eq!(viewer.window().offset(), first_line_of(&viewer, 5) - 1);
    }

    #[test]
    fn matches_the_toggles_hide_are_counted_in_the_chip() {
        let mut viewer = pager(needles());
        draw(&mut viewer, 120, 8);
        press(&mut viewer, "/tool output<Enter>");
        let screen = draw(&mut viewer, 120, 8);
        assert!(
            screen.contains("find \"tool output\" no match · 1 in hidden turns"),
            "{screen}"
        );
        assert_eq!(
            viewer.content().find_detail("needle").as_deref(),
            Some("2 in hidden turns")
        );
    }

    #[test]
    fn showing_a_kind_brings_its_matches_into_the_find() {
        let mut viewer = pager(needles());
        draw(&mut viewer, 80, 8);
        press(&mut viewer, "g/needle<Enter>T");
        assert_eq!(viewer.find().matches(), [2, 5, 9, 13]);
        assert_eq!(viewer.find().current_unit(), Some(2), "stays on its turn");
        press(&mut viewer, "ttt");
        assert_eq!(viewer.find().matches(), [2, 5, 9, 12, 13]);
    }

    #[test]
    fn the_current_turn_is_painted_in_the_selection_look_after_the_gutter() {
        let theme = Theme::default();
        let mut viewer = pager(needles());
        let area = Rect::new(0, 0, 80, 8);
        let mut buf = Buffer::empty(area);
        viewer.draw(area, &mut buf, &theme);
        press(&mut viewer, "g/needle<Enter>");
        viewer.draw(area, &mut buf, &theme);
        let current = buffer::style_at(&buf, "Needle").expect("the match is drawn");
        assert!(
            current.add_modifier.contains(theme.selection_active),
            "{current:?}"
        );
        let rows = buffer::rows(&buf);
        let label = rows.iter().find(|row| row.contains("Needle")).unwrap();
        assert!(label.contains("│ turn 2"), "{label}");
    }

    #[test]
    fn a_collapsed_tool_run_is_one_unit_of_rows_for_all_its_turns() {
        let mut document = sample_document();
        document.turns.extend([
            turn(TurnRole::Assistant, TurnKind::ToolUse, "Read: /a"),
            turn(TurnRole::Assistant, TurnKind::ToolResult, "ok"),
            turn(TurnRole::Assistant, TurnKind::Thinking, "hmm"),
            turn(TurnRole::Assistant, TurnKind::Message, "done"),
        ]);
        let mut transcript: Transcript = Transcript::new(document);
        transcript.tool_detail = ToolDetail::Summary;
        let lines = transcript.lines(&Theme::default(), 80).len();
        let run = transcript.rows_of(2);
        assert!(!run.is_empty());
        assert_eq!(transcript.rows_of(3), run, "one summary line for the run");
        assert_eq!(transcript.rows_of(1).end, run.start);
        // A turn the toggles hide takes no rows, where the next one starts.
        let hidden = transcript.rows_of(4);
        assert!(hidden.is_empty(), "{hidden:?}");
        assert_eq!(hidden.start, transcript.rows_of(5).start);
        assert_eq!(transcript.rows_of(5).end, lines);
        assert_eq!(transcript.rows_of(9), lines..lines, "past the last turn");
    }

    #[test]
    fn the_hidden_chip_shows_only_for_a_transcript_with_hidden_turns() {
        let mut document = sample_document();
        let mut viewer = pager(document.clone());
        assert_eq!(viewer.content().chips(&Theme::default()).len(), 3);
        document.turns[0].hidden = true;
        viewer = pager(document);
        let screen = draw(&mut viewer, 120, 8);
        assert!(screen.contains("hidden·hide (H)"), "{screen}");
    }

    #[test]
    fn an_empty_body_says_why() {
        let mut viewer = pager(unavailable_document());
        let screen = draw(&mut viewer, 60, 8);
        assert!(screen.contains("transcript unavailable"), "{screen}");
        let mut only_thinking = sample_document();
        for turn in &mut only_thinking.turns {
            turn.kind = TurnKind::Thinking;
        }
        let mut viewer = pager(only_thinking);
        let screen = draw(&mut viewer, 60, 8);
        assert!(screen.contains("every turn here is hidden"), "{screen}");
    }

    #[test]
    fn the_help_page_lists_the_toggles_and_the_viewers_words() {
        let mut viewer = pager(sample_document());
        press(&mut viewer, "?");
        let screen = draw(&mut viewer, 100, 40);
        for line in [
            "Showing more or less",
            "Cycle tool detail: hidden, summary, truncated, full",
            "Show or hide records the harness's own UI never showed",
            "Jump to the start of the transcript",
            "Jump to the end, where the viewer opens, and stay there",
            "Find in transcript; a match in hidden content turns that content on",
        ] {
            assert!(screen.contains(line), "{line}: {screen}");
        }
        let sections = viewer.help_sections();
        let titles: Vec<&str> = sections
            .iter()
            .map(|section| section.title.as_ref())
            .collect();
        assert_eq!(
            titles,
            ["Moving", "Finding", "Showing more or less", "Viewer"]
        );
        assert!(tuikit::help::undescribed(crate::viewer::keys::TRANSCRIPT_KEYS).is_empty());
    }
}
