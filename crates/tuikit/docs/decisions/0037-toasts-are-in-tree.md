# 0037: Toasts are in-tree and take their time from the host

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/feedback.md](../controls/feedback.md) |

## Decision

- `Toasts` keeps the toasts on screen with their lifetimes. `show` replaces
  them, so the newest report is the one shown; `push` stacks one above the
  others.
- A toast is a quadrant-block border in its severity's color, a cell of
  padding, and the text at most 46 cells wide, wrapped by the text module's
  rules (at the screen's width less the box on a screen narrower than a
  toast), centered on the bottom edge with the odd cell before it, on the
  `surface` background in `toast_text`.
- Time comes from the host: `show(feedback, now)` and `tick(now)`.

## Rationale

The toast crate it replaces drew its borders in a fixed mapping that ignored
the theme, had no notion of severity, and brought three crates of its own.
Taking the time as an argument keeps tests free of sleeps and frames
independent of when they are drawn.

## Consequences

- The toast crate and its wrapping dependencies left the build (record
  [0008](0008-the-third-party-crates.md)).

## Alternatives considered

- **Keeping the toast crate and wrapping it.** Theming its borders would
  need a fork.
- **Its balanced wrap.** It breaks paths nicely, but it is a second wrapping
  rule, and the crate left with the toaster.
- **The framework reading the clock.** Fewer parameters, but tests would
  sleep.

## Origin

- host ADR 0163, "Feedback is one type with four severities, and toasts are
  in-tree": decision 5.
- host ADR 0152, "Who owns the framework's third-party crates": decision 4
  (the toast crate replaced with in-tree code).
