# `Feedback` and `Toasts`

| | |
| --- | --- |
| Module | `tuikit::feedback` |
| Layer | 2 (convention), with `Toasts` as its layer 1 control |
| Type parameters | `T`: the host's payload on each log entry (what it was about, what ran); `W`: the sink the clipboard writes to |
| Cargo feature | none |
| Outcomes | none: nothing here handles keys. A host posts feedback in response to its own messages and draws it |

## Purpose

The module is how a program tells the user what happened, at the right
volume, and how loud each report is. Every report has a `Severity` (info,
success, warning, error), and each of the three channels styles from the
theme by it, so a failure never looks like a success:

- **The status line** holds one `Feedback` until the host clears it
  (usually on the next move). `Feedback::span` draws its text in the
  severity's color.
- **Toasts** float at the bottom of the screen and go when their time is
  up. `Toasts` keeps them, each with its own lifetime (`Feedback::ttl`,
  from `Severity::ttl`: errors stay longest), and draws each in a box
  whose border takes the severity's color.
- **The log** keeps what happened for later reading. `Log<T>` is bounded
  and newest-last, collapses a run of identical entries into one with a
  repeat count, and counts the warnings and errors the user has not looked
  at. Each `Entry<T>` has a time, a severity, a one-line summary, and the
  host's payload `T`.

`clipboard::copy` writes the OSC 52 sequence that puts text on the
terminal's clipboard to a sink the host supplies, since the framework
writes to nothing of its own.

The module replaces what a program grows channel by channel: a status
line that paints every message in the warning color, a third-party toast
whose border is always the success green whatever it reports and that
ignores the theme, a log wired to the program's own domain types, and a
clipboard write that goes straight to the process's output, so no test
can see it.

The host keeps which severity each report has and which channel it takes,
the words, its log's payload type and how an entry's detail reads, when
the status line clears, and the sink the clipboard writes to.

## Usage example

```rust
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::feedback::{Entry, Feedback, Log, Severity, Toasts, clipboard};
use tuikit::theme::Theme;

/// The host's payload: which document an outcome concerns.
#[derive(Clone, Debug, Default, PartialEq)]
struct About {
    document: Option<String>,
}

let theme = Theme::default();
let now = Instant::now();

// A hint and a refusal on the status line, each in its own color.
let hint = Feedback::info("type to filter");
let refusal = Feedback::warning("nothing to undo");
assert_eq!(hint.span(&theme).style.fg, Some(theme.info));
assert_eq!(refusal.span(&theme).style.fg, Some(theme.warning));

// A toast: a new one replaces the one showing, and an error stays longer.
let mut toasts = Toasts::new();
toasts.show(Feedback::success("saved"), now);
toasts.show(Feedback::error("save failed: disk full"), now);
assert_eq!(toasts.len(), 1);
let area = Rect::new(0, 0, 80, 24);
let mut buf = Buffer::empty(area);
toasts.render(area, &mut buf, &theme);
let [rect] = toasts.areas(area)[..] else { unreachable!() };
assert_eq!(rect.bottom(), area.bottom(), "toasts sit on the bottom edge");
assert_eq!(buf[(rect.x, rect.y)].fg, theme.error, "the border says how bad");
toasts.tick(now + Severity::Success.ttl());
assert!(!toasts.is_empty(), "an error outlives a confirmation's time");
toasts.tick(now + Severity::Error.ttl());
assert!(toasts.is_empty());

// A brief toast of any severity.
toasts.show(Feedback::info("copied").with_ttl(Duration::from_millis(500)), now);

// The log, with the host's payload on each entry.
let mut log: Log<About> = Log::new(200);
let about = |name: &str| About { document: Some(name.to_string()) };
log.push(Entry::new(1_000, Severity::Error, "save failed", about("notes")));
log.push(Entry::new(1_005, Severity::Error, "save failed", about("notes")));
log.push(Entry::new(1_010, Severity::Success, "saved", about("plan")));
assert_eq!(log.len(), 2, "the repeat collapsed");
assert_eq!(log.unseen(), 1, "and counts as one failure");
let latest_about_notes = log
    .newest_where(|entry| entry.item.document.as_deref() == Some("notes"))
    .unwrap();
assert_eq!(latest_about_notes.repeats, 2);
log.mark_seen();

// The clipboard writes to the sink it is given.
let mut sink = Vec::new();
clipboard::copy(&mut sink, "foo").unwrap();
assert_eq!(sink, b"\x1b]52;c;Zm9v\x07");
```

### The API

| Item | What it is |
| --- | --- |
| `Severity` | `Info`, `Success`, `Warning`, `Error`, ordered from least to most urgent; `ALL` lists them |
| `Severity::color(theme)`, `style(theme)` | the `info`, `success`, `warning`, or `error` token, and text in it |
| `Severity::tone()` | the `chip::Tone` of the same color, so a status chip agrees |
| `Severity::mark()` | `•`, `√`, `▲`, `×` (`marks::BULLET`, `OK`, `WARNING`, `FAIL`): what a list of reports leads each row with |
| `Severity::needs_attention()` | warnings and errors |
| `Severity::ttl()` | a toast's default lifetime: 1.5 s for info and success, 3 s for a warning, 5 s for an error |
| `Feedback { severity, text, ttl }` | one report; `Feedback::new(severity, text)` and `info`, `success`, `warning`, `error` take the severity's lifetime; `with_ttl` sets another |
| `Feedback::span(theme)` | the status line's rendering: the text in its severity's color |
| `Toasts` | the toasts on screen; `show` replaces them, `push` stacks one above them (the oldest goes when `with_capacity` are shown, 4 by default), `tick(now)` retires the expired, `render(area, buf, theme)` draws them, `areas(area)` says where |
| `Log<T>` | `new(capacity)` (or `default`, 200), `push`, `newest_first`, `newest_where`, `unseen`, `mark_seen`, `len`, `capacity` |
| `Entry<T> { at, severity, summary, item, repeats }` | one logged outcome; `Entry::new(at, severity, summary, item)`; `feedback()` is its severity and summary as a `Feedback` |
| `clipboard::copy(sink, text)` | writes `ESC ] 52 ; c ; <base64> BEL` to `sink` and flushes it |

### A toast's look

A toast is drawn after everything else, over whatever is under it, at the
bottom edge of the area it is given and centered across it (an odd cell
left over goes before it, as a modal's does). Its box is a border of
quadrant blocks in the severity's color, one cell of padding inside it,
and the text in the `toast_text` token on the `surface` token. The text
wraps by the `text` module's rules at `Toasts::MAX_TEXT_WIDTH` (46) cells,
or the area's width less the box's four cells when that is narrower, so
no word is cut off on a narrow screen. The box is as wide as the widest
line of text and as tall as its rows. Stacked toasts sit one on top of
another, the newest at the bottom; a toast with no room left above the
newer ones is not drawn.

A toast is not a modal: it takes no keys, and it is placed by its own
rule, not a `ModalSize`, so it does not draw in a `ModalFrame`.

### Time

Nothing here reads the clock. Posting takes the moment it happens and
`tick` the moment to check against, so a test posts at `now` and ticks at
`now + Severity::Error.ttl()` without sleeping, and a frame draws exactly
what the last tick left. A log entry's `at` is the host's Unix time in
seconds; a repeat moves it to the latest.

## Key table

None: no part of the module handles keys. The control that shows the log
(a reader over its entries) has the keys; a toast is dismissed by time.

## Theme tokens

| Token | Used for |
| --- | --- |
| `info` | an info report on the status line, its toast's border, its mark in a log |
| `success` | the same, for a success |
| `warning` | the same, for a warning |
| `error` | the same, for an error |
| `surface` | a toast's background |
| `toast_text` | a toast's text (white by default), the same for every severity |

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 11 | Feedback severity | Four severities; the status line, toasts, and the log's marks all style from the four tokens; an error's toast stays 5 s and a warning's 3 s against 1.5 s for a confirmation |
| 13 | Theme | Every color is a token: the border from the severity, the text from `toast_text`, the background from `surface`; no raw color |
| 14 | Width | A toast measures and wraps its text in cells through the `text` module, and never runs past the area it is given |

Not applicable: 1 to 4, 6, 7, 10, 12, 15, and 16 govern lists, closing,
confirming, forms, paging, and editing; nothing here has a cursor, keys,
or fields. 5 (cancelling is silent) is the host's: it posts no feedback
for a close. 8 (generated hints) is the host's too: a hint it shows on the
status line is generated from its key tables before it becomes a
`Feedback`. 9 (the focus cue) is a pane's, 17 (labels) the host's copy,
and 18 a pager's.

## Seams left open

- **Dismissing with a key or a click.** `Toasts::areas` says where each
  toast is, which is what a click would be hit-tested against, and a key
  that dismisses them would call `clear`. Neither is bound.
- **Sticky toasts.** A toast lives until its `ttl`; one whose lifetime
  runs past what the clock can count (`Duration::MAX`) stays until
  `clear`, which a dismissing key (above) would call.
- **Placement.** Toasts sit on the bottom edge; another corner or keeping
  clear of a region (a status line that must stay readable) is a change to
  the free area `render` is given, which a host can make today by passing
  a smaller area.
- **A reader for the log.** `Log` is the model; the `LogList` reader that
  draws its rows (severity mark, age, summary, and the repeat count) is a
  later control.
- **Other clipboard routes.** The sequence is OSC 52 only; wrapping it for
  a terminal multiplexer's passthrough, or a platform clipboard, would be
  another function writing to the same kind of sink.
