//! Every boundary of the default ladder and of the recency scale, the
//! weeks ladder and a host's own units, clock skew, and the age cell.

use ratatui::style::{Color, Modifier, Style};

use super::*;
use crate::theme::StyleSpec;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

fn days(secs: u64) -> String {
    Ladder::DAYS.label(Age::from_secs(secs))
}

fn weeks(secs: u64) -> String {
    Ladder::WEEKS.label(Age::from_secs(secs))
}

#[test]
fn the_default_ladder_steps_up_at_each_unit_and_rounds_down() {
    let cases = [
        (0, "0s"),
        (1, "1s"),
        (59, "59s"),
        (MINUTE, "1m"),
        (MINUTE + 59, "1m"),
        (HOUR - 1, "59m"),
        (HOUR, "1h"),
        (DAY - 1, "23h"),
        (DAY, "1d"),
        (2 * DAY + 12 * HOUR, "2d"),
        (WEEK, "7d"),
        (365 * DAY, "365d"),
        (999 * DAY, "999d"),
        (1000 * DAY, "1000d"),
    ];
    for (secs, label) in cases {
        assert_eq!(days(secs), label, "{secs}s");
    }
}

#[test]
fn the_weeks_ladder_agrees_below_a_week_and_goes_on_to_weeks() {
    for secs in [0, 59, MINUTE, HOUR - 1, HOUR, DAY - 1, DAY, WEEK - 1] {
        assert_eq!(weeks(secs), days(secs), "{secs}s");
    }
    assert_eq!(weeks(WEEK - 1), "6d");
    assert_eq!(weeks(WEEK), "1w");
    assert_eq!(weeks(20 * DAY), "2w");
    assert_eq!(weeks(52 * WEEK), "52w");
}

#[test]
fn the_default_ladder_is_days() {
    assert_eq!(Ladder::default(), Ladder::DAYS);
    assert_eq!(Age::from_secs(20 * DAY).label(), "20d");
    assert_eq!(Age::from_secs(20 * DAY).to_string(), "20d");
    let suffixes: Vec<&str> = Ladder::DAYS
        .units()
        .iter()
        .map(|unit| unit.suffix)
        .collect();
    assert_eq!(suffixes, ["s", "m", "h", "d"]);
}

#[test]
fn a_host_ladder_sets_its_own_thresholds() {
    const COARSE: Ladder = Ladder::new(&[Unit::MINUTE, Unit::new(10 * DAY, "dek")]);
    let coarse = |secs| COARSE.label(Age::from_secs(secs));
    assert_eq!(
        coarse(0),
        "0m",
        "an age under the first unit counts 0 of it"
    );
    assert_eq!(coarse(59), "0m");
    assert_eq!(coarse(90), "1m");
    assert_eq!(coarse(10 * DAY - 1), "14399m");
    assert_eq!(coarse(25 * DAY), "2dek");
}

#[test]
#[should_panic(expected = "grow from the smallest")]
fn a_ladder_out_of_order_is_refused() {
    static BACKWARDS: [Unit; 2] = [Unit::HOUR, Unit::MINUTE];
    let _ = Ladder::new(&BACKWARDS);
}

#[test]
#[should_panic(expected = "at least one unit")]
fn an_empty_ladder_is_refused() {
    let _ = Ladder::new(&[]);
}

#[test]
#[should_panic(expected = "at least a second")]
fn a_unit_of_no_length_is_refused() {
    static NOTHING: [Unit; 1] = [Unit::new(0, "z")];
    let _ = Ladder::new(&NOTHING);
}

#[test]
fn ago_adds_the_one_prose_suffix() {
    assert_eq!(Age::from_secs(0).ago(), "0s ago");
    assert_eq!(Age::from_secs(5 * MINUTE).ago(), "5m ago");
    assert_eq!(Age::from_secs(30 * HOUR).ago(), "1d ago");
    assert_eq!(Ladder::WEEKS.ago(Age::from_secs(3 * WEEK)), "3w ago");
}

#[test]
fn an_age_is_measured_between_two_unix_times() {
    let now = 1_000_000;
    assert_eq!(Age::between(now - 90, now).secs(), 90);
    assert_eq!(Age::between(now, now).secs(), 0);
    assert_eq!(
        Age::between(now + 500, now).secs(),
        0,
        "a time in the future (clock skew) is an age of zero"
    );
    assert_eq!(Age::between(i64::MIN, i64::MAX).secs(), i64::MAX as u64);
    assert_eq!(Age::between(i64::MAX, i64::MIN).secs(), 0);
}

#[test]
fn an_unknown_time_has_no_age() {
    assert_eq!(Age::of(None, Some(100)), None);
    assert_eq!(Age::of(Some(100), None), None);
    assert_eq!(Age::of(None, None), None);
    assert_eq!(Age::of(Some(40), Some(100)), Some(Age::from_secs(60)));
}

#[test]
fn each_recency_bound_is_the_first_age_of_the_next_step() {
    let cases = [
        (0, Recency::Fresh),
        (MINUTE, Recency::Fresh),
        (5 * MINUTE - 1, Recency::Fresh),
        (5 * MINUTE, Recency::Active),
        (30 * MINUTE, Recency::Active),
        (HOUR - 1, Recency::Active),
        (HOUR, Recency::Recent),
        (12 * HOUR, Recency::Recent),
        (DAY - 1, Recency::Recent),
        (DAY, Recency::Cold),
        (WEEK, Recency::Cold),
    ];
    for (secs, recency) in cases {
        assert_eq!(Age::from_secs(secs).recency(), recency, "{secs}s");
    }
}

#[test]
fn a_host_scale_sets_its_own_bounds() {
    let scale = RecencyScale {
        fresh: Age::from_secs(10),
        active: Age::from_secs(20),
        recent: Age::from_secs(30),
    };
    let steps: Vec<Recency> = [0, 9, 10, 19, 20, 29, 30]
        .into_iter()
        .map(|secs| scale.recency(Age::from_secs(secs)))
        .collect();
    assert_eq!(
        steps,
        [
            Recency::Fresh,
            Recency::Fresh,
            Recency::Active,
            Recency::Active,
            Recency::Recent,
            Recency::Recent,
            Recency::Cold,
        ]
    );
}

#[test]
fn each_recency_step_styles_from_its_theme_token() {
    let mut theme = Theme::default();
    let defaults: Vec<Style> = Recency::ALL
        .iter()
        .map(|recency| recency.style(&theme))
        .collect();
    assert_eq!(
        defaults,
        [
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Green),
            Style::default().fg(Color::Yellow),
            Style::default().add_modifier(Modifier::DIM),
        ]
    );
    theme.recency_recent = StyleSpec::fg(Color::Magenta);
    assert_eq!(
        Recency::Recent.style(&theme),
        Style::default().fg(Color::Magenta)
    );
}

#[test]
fn the_cell_right_aligns_tints_and_cuts() {
    let theme = Theme::default();
    let fresh = cell(Some("5s"), Some(Recency::Fresh), CELL_WIDTH, &theme);
    assert_eq!(fresh.content, "  5s");
    assert_eq!(fresh.style, Recency::Fresh.style(&theme));
    assert_eq!(cell(Some("999d"), None, CELL_WIDTH, &theme).content, "999d");
    assert_eq!(
        cell(Some("1000d"), Some(Recency::Cold), CELL_WIDTH, &theme).content,
        "100…",
        "a label wider than the cell is cut, never widens the column"
    );
    assert_eq!(cell(Some("5s"), None, 6, &theme).content, "    5s");
}

#[test]
fn an_unknown_age_is_the_no_value_mark_and_recedes() {
    let theme = Theme::default();
    let unknown = cell(None, None, CELL_WIDTH, &theme);
    assert_eq!(unknown.content, format!("   {}", marks::NO_VALUE));
    assert_eq!(unknown.style, Style::default().add_modifier(theme.muted));
    let untinted = cell(Some("3h"), None, CELL_WIDTH, &theme);
    assert_eq!(untinted.style, Style::default().add_modifier(theme.muted));
}
