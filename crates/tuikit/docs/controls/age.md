# `age`

| | |
| --- | --- |
| Module | `tuikit::age` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: every item is a pure value or function |

## Purpose

`age` says how long ago something happened, one way everywhere: as
compact text in the largest unit it has reached (`45s`, `17m`, `3h`,
`12d`), as prose (`5m ago`), as a tint that carries its freshness in
color (fresh, active, recent, cold), and as a fixed-width cell that keeps
a column of ages aligned. The units and the tint's bounds are values a
host may replace.

The module replaces what a program grows one surface at a time: a
formatter per surface, one stopping at hours and another going on to
weeks, `just now` in one place and `0s` in the next, thresholds written as
literals inside a list's row code, and the word "ago" appended by hand.
The host keeps what an age counts from (when an item was last active,
created, or seen), when the clock is read, and which surfaces tint.

## Usage example

A list that formats its ages when it builds its rows and tints them when
it draws, a detail field in prose, and a host's own thresholds.

```rust
use ratatui::text::{Line, Span};
use tuikit::age::{self, Age, Ladder, Recency, RecencyScale, Unit};
use tuikit::theme::Theme;

let theme = Theme::default();
let now = 1_790_000_000;

// Build: the label once, from the item's time. An unknown time has none.
struct Row {
    name: &'static str,
    at: Option<i64>,
    age: Option<String>,
}
let rows: Vec<Row> = [("first", Some(now - 42)), ("second", None)]
    .into_iter()
    .map(|(name, at)| Row { name, at, age: Age::of(at, Some(now)).map(Age::label) })
    .collect();

// Draw: tint by the age at the moment of drawing.
let draw_now = now + 1;
let lines: Vec<Line> = rows
    .iter()
    .map(|row| {
        let recency = Age::of(row.at, Some(draw_now)).map(Age::recency);
        Line::from(vec![
            Span::raw(format!("{:<8}", row.name)),
            age::cell(row.age.as_deref(), recency, age::CELL_WIDTH, &theme),
        ])
    })
    .collect();
assert_eq!(lines[0].to_string(), "first    42s");
assert_eq!(lines[1].to_string(), "second     —");
assert_eq!(lines[0].spans[1].style, Recency::Fresh.style(&theme));

// Prose, and the weeks ladder for a surface with no fixed column.
let created = Age::between(now - 20 * 86_400, now);
assert_eq!(created.ago(), "20d ago");
assert_eq!(Ladder::WEEKS.ago(created), "2w ago");

// A host's own units and bounds.
const COARSE: Ladder = Ladder::new(&[Unit::MINUTE, Unit::HOUR]);
assert_eq!(COARSE.label(Age::from_secs(30)), "0m");
let strict = RecencyScale {
    fresh: Age::from_secs(60),
    ..RecencyScale::default()
};
assert_eq!(strict.recency(Age::from_secs(90)), Recency::Active);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Age::from_secs(secs)`, `secs()` | an age in whole seconds | never negative |
| `Age::between(then, now)` | the age at `now` of `then`, both Unix seconds | a `then` in the future (clock skew) is 0 |
| `Age::of(then, now)` | `between` when both are known | `None` otherwise |
| `label()`, `ago()`, `recency()`, `Display` | the default ladder's text, as prose, and the default scale's step | `45s`, `5m ago`, `Recency::Fresh` |
| `Unit { seconds, suffix }`, `Unit::SECOND` .. `Unit::WEEK` | one step of a ladder | `Unit::new(seconds, suffix)` for others |
| `Ladder::DAYS` (default), `Ladder::WEEKS` | the units an age is written in | days are the largest unit, or weeks |
| `Ladder::new(units)` | a host's ladder | panics (at compile time in a constant) unless the units are non-empty, at least a second, and increasing |
| `label(age)`, `ago(age)`, `units()` | the age in the largest unit reached, rounded down; as prose; the units | below the first unit, a count of 0 of it |
| `Recency` (`Fresh`, `Active`, `Recent`, `Cold`), `ALL` | the four steps | `style(theme)` reads `recency_*` |
| `RecencyScale { fresh, active, recent }` | where each step ends | default 5 minutes, 1 hour, 1 day; `recency(age)` |
| `cell(label, recency, width, theme)` | an age cell | right-aligned, cut with `…`, the no-value mark when unknown, `muted` when untinted |
| `CELL_WIDTH` | 4 | the widest default label under a thousand days, `999d` |

### The ladder

An age is written as a whole count of the largest unit it has reached,
rounded down, with no space before the suffix. On `Ladder::DAYS`:

| Age | Text |
| --- | --- |
| 0 to 59 seconds | `0s` to `59s` |
| 1 minute to 59 minutes 59 seconds | `1m` to `59m` |
| 1 hour to 23 hours 59 minutes | `1h` to `23h` |
| 1 day or more | `1d`, `2d`, … `365d` (no larger unit) |

`Ladder::WEEKS` is the same below a week and then counts weeks (`1w`,
`2w`, …): twenty days is `2w`. Every unit boundary is the first age of the
next unit, so 60 seconds is `1m` and 24 hours is `1d`. `ago` appends ` ago`
and nothing else: there is no `just now`, so a fresh item reads `0s ago`
wherever an age is written as prose.

### The tint

`Recency` sorts an age by the scale's bounds; each bound is the first age
of the next step:

| Step | Default ages | Token (default style) |
| --- | --- | --- |
| `Fresh` | under 5 minutes | `recency_fresh` (light green, bold) |
| `Active` | 5 minutes to under 1 hour | `recency_active` (green) |
| `Recent` | 1 hour to under 1 day | `recency_recent` (yellow) |
| `Cold` | 1 day or more | `recency_cold` (dim) |

### The cell

`cell` right-aligns a label in a fixed width, cuts a wider one with the
ellipsis mark rather than widening the column, shows the no-value mark for
an unknown age, and styles it from its recency, or with the theme's
`muted` modifier when the recency is unknown. The label is passed in so a
list can format each age once when it builds its rows and tint it with the
clock at each draw.

## Key table

None: the module handles no keys.

## Theme tokens

| Token | Used for |
| --- | --- |
| `recency_fresh`, `recency_active`, `recency_recent`, `recency_cold` | the four steps of the tint |
| `muted` | an age cell whose recency is unknown |

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 13 | Theme | The tint reads the `recency_*` tokens and an untinted cell reads `muted`; no color is named |
| 14 | Width | `cell` measures and cuts in cells through `text` and ends a cut in `marks::ELLIPSIS`, so a column of ages never widens |

Not applicable: 1 to 12 and 15 to 18 govern how controls move, confirm,
close, edit, page, give feedback, and label. The module only formats and
tints; the lists and fields that show ages follow those rules.

## Seams left open

- **Larger units on demand.** A surface whose ages run to years could pass
  a ladder with a year unit; the cell width would then follow its widest
  label.
- **Two units.** A precise surface could want `1h 5m`; a ladder option to
  write the next unit down would carry it.
- **Future times.** A time ahead of the clock is an age of zero. A host
  that wants to say so (`in 5m`, `now`) checks the order before it asks for
  an age.
- **Localization.** The suffixes and ` ago` are English and are the
  ladder's values; a host ladder can carry other suffixes, and the prose
  form would take its word from the ladder too.
- **Ticking.** An age is computed from the clock the host reads; a list
  that wants its ages to advance between refreshes redraws them.
