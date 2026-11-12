use ratatui::style::Modifier;

use super::*;
use crate::model::live::LiveDetail;

fn cell(state: LiveState, confidence: LiveConfidence) -> LiveCell {
    LiveCell {
        state,
        detail: None,
        confidence,
        since_ms: 0,
        subagents: 0,
    }
}

#[test]
fn every_state_has_the_mark_the_operator_chose() {
    let cases = [
        (LiveState::Waiting, LiveMark::Waiting, "!"),
        (LiveState::Working, LiveMark::Working, "»"),
        (LiveState::Compacting, LiveMark::Working, "»"),
        (LiveState::Starting, LiveMark::Working, "»"),
        (LiveState::Ready, LiveMark::Ready, "›"),
        (LiveState::Error, LiveMark::Error, "×"),
        (LiveState::Unknown, LiveMark::Unknown, "?"),
        (LiveState::Exited, LiveMark::None, "·"),
    ];
    for state in LiveState::ALL {
        assert!(
            cases.iter().any(|(case, _, _)| *case == state),
            "{state:?} has no case"
        );
    }
    for (state, mark, glyph) in cases {
        assert_eq!(LiveMark::of_state(state), mark, "{state:?}");
        assert_eq!(mark.glyph(), glyph, "{state:?}");
    }
}

#[test]
fn every_mark_has_its_own_shape_so_none_relies_on_color() {
    let glyphs: Vec<&str> = LiveMark::ALL.iter().map(|mark| mark.glyph()).collect();
    for (index, glyph) in glyphs.iter().enumerate() {
        assert!(
            !glyphs[index + 1..].contains(glyph),
            "`{glyph}` stands for two marks"
        );
    }
}

#[test]
fn only_the_four_states_the_operator_acts_on_have_words() {
    let words: Vec<_> = LiveMark::ALL.map(LiveMark::word).into();
    assert_eq!(
        words,
        [
            Some("waiting"),
            Some("working"),
            Some("ready"),
            Some("error"),
            None,
            None
        ]
    );
}

#[test]
fn marks_take_the_themes_status_colors_and_the_muted_look() {
    let theme = Theme::default();
    let fg = |mark: LiveMark| mark.style(&theme).fg;
    assert_eq!(fg(LiveMark::Waiting), Some(theme.warning));
    assert_eq!(fg(LiveMark::Working), Some(theme.info));
    assert_eq!(fg(LiveMark::Ready), Some(theme.success));
    assert_eq!(fg(LiveMark::Error), Some(theme.error));
    for mark in [LiveMark::Unknown, LiveMark::None] {
        assert_eq!(fg(mark), None, "{mark:?}");
        assert!(mark.style(&theme).add_modifier.contains(theme.muted));
    }
}

#[test]
fn a_custom_theme_recolors_the_marks() {
    let mut theme = Theme::default();
    theme.warning = ratatui::style::Color::LightRed;
    theme.info = ratatui::style::Color::Cyan;
    assert_eq!(
        LiveMark::Waiting.style(&theme).fg,
        Some(ratatui::style::Color::LightRed)
    );
    assert_eq!(
        LiveMark::Working.style(&theme).fg,
        Some(ratatui::style::Color::Cyan)
    );
}

#[test]
fn a_quiet_state_draws_the_same_glyph_dimmed() {
    let theme = Theme::default();
    let fresh = LiveSign::of_session(Some(cell(LiveState::Working, LiveConfidence::Fresh)));
    let quiet = LiveSign::of_session(Some(cell(LiveState::Working, LiveConfidence::Quiet)));
    assert_eq!(fresh.glyph(), quiet.glyph());
    assert!(!fresh.style(&theme).add_modifier.contains(Modifier::DIM));
    assert!(quiet.style(&theme).add_modifier.contains(theme.muted));
    assert_eq!(quiet.style(&theme).fg, fresh.style(&theme).fg);
}

#[test]
fn a_session_without_evidence_and_a_stale_one_differ() {
    let none = LiveSign::of_session(None);
    let stale = LiveSign::of_session(Some(LiveCell {
        detail: Some(LiveDetail::Stale),
        ..cell(LiveState::Unknown, LiveConfidence::Stale)
    }));
    assert_eq!(none, LiveSign::NONE);
    assert_eq!(none.glyph(), "·");
    assert_eq!(stale.glyph(), "?");
    assert!(!stale.quiet, "stale is its own mark, not a dimmed one");
}

#[test]
fn a_mux_shows_the_state_its_agents_rank_first() {
    let counts = |states: &[LiveState]| states.iter().copied().collect::<LiveCounts>();
    let cases = [
        (vec![], LiveMark::None),
        (vec![LiveState::Exited], LiveMark::None),
        (
            vec![LiveState::Exited, LiveState::Unknown],
            LiveMark::Unknown,
        ),
        (
            vec![LiveState::Working, LiveState::Starting],
            LiveMark::Working,
        ),
        (vec![LiveState::Working, LiveState::Ready], LiveMark::Ready),
        (
            vec![LiveState::Working, LiveState::Ready, LiveState::Error],
            LiveMark::Error,
        ),
        (
            vec![
                LiveState::Working,
                LiveState::Working,
                LiveState::Error,
                LiveState::Waiting,
            ],
            LiveMark::Waiting,
        ),
    ];
    for (states, mark) in cases {
        let sign = LiveSign::of_mux(counts(&states));
        assert_eq!(sign.mark, mark, "{states:?}");
        assert!(!sign.quiet);
    }
}

#[test]
fn a_mux_shows_the_operators_order_whatever_the_counts() {
    use LiveState::{Compacting, Error, Exited, Ready, Starting, Unknown, Waiting, Working};
    // The order the operator chose for a mux: what needs them first,
    // then a finished turn, then work under way, then the rest. One agent
    // in a state earlier in the order outranks any number in a later one.
    let order = [
        Waiting, Error, Ready, Working, Compacting, Starting, Unknown, Exited,
    ];
    for (rank, first) in order.iter().enumerate() {
        for later in &order[rank + 1..] {
            let mut counts = LiveCounts::default();
            counts.add(*first);
            for _ in 0..9 {
                counts.add(*later);
            }
            let sign = LiveSign::of_mux(counts);
            assert_eq!(
                sign.mark,
                LiveMark::of_state(*first),
                "one {first:?} beside nine {later:?}"
            );
            assert!(!sign.quiet, "a mux's mark is never dimmed");
        }
    }
}

#[test]
fn a_mux_with_one_two_or_mixed_agents_reads_its_agents() {
    use LiveState::{Error, Ready, Waiting, Working};
    let mux = |states: &[LiveState]| LiveSign::of_mux(states.iter().copied().collect()).mark;
    // One agent: its own mark.
    assert_eq!(mux(&[Ready]), LiveMark::Ready);
    assert_eq!(mux(&[Working]), LiveMark::Working);
    // Two in one state: that state.
    assert_eq!(mux(&[Working, Working]), LiveMark::Working);
    assert_eq!(mux(&[Waiting, Waiting]), LiveMark::Waiting);
    // Mixed: the first by the order.
    assert_eq!(mux(&[Working, Waiting]), LiveMark::Waiting);
    assert_eq!(mux(&[Ready, Error]), LiveMark::Error);
    assert_eq!(mux(&[Working, Ready]), LiveMark::Ready);
}

#[test]
fn a_mark_counts_the_agents_that_draw_it() {
    use LiveState::{Compacting, Exited, Starting, Waiting, Working};
    let counts: LiveCounts = [Waiting, Waiting, Working, Compacting, Starting, Exited]
        .into_iter()
        .collect();
    assert_eq!(LiveMark::Waiting.count_in(counts), 2);
    assert_eq!(LiveMark::Working.count_in(counts), 3, "work under way");
    assert_eq!(LiveMark::None.count_in(counts), 1, "exited");
    assert_eq!(LiveMark::Ready.count_in(counts), 0);
    assert_eq!(LiveMark::Waiting.count_in(LiveCounts::default()), 0);
}

#[test]
fn a_marks_tone_draws_its_style() {
    let theme = Theme::default();
    for mark in LiveMark::ALL {
        assert_eq!(mark.style(&theme), mark.tone().style(&theme), "{mark:?}");
    }
}

#[test]
fn a_mixed_row_reads_the_sign_of_what_it_stands_for() {
    let session = cell(LiveState::Ready, LiveConfidence::Fresh);
    assert_eq!(
        LiveSign::of_mixed(MixedLive::Session(session)),
        LiveSign::of_session(Some(session))
    );
    let counts: LiveCounts = [LiveState::Waiting].into_iter().collect();
    assert_eq!(
        LiveSign::of_mixed(MixedLive::Mux(counts)).mark,
        LiveMark::Waiting
    );
    assert_eq!(LiveSign::of_mixed(MixedLive::None), LiveSign::NONE);
}
