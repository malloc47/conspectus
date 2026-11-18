# 0072: Smart wrap takes its settings, and a fit for each line, from the host

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/richtext.md](../controls/richtext.md) |

## Decision

- `richtext::SmartWrap` holds smart wrap's settings: the `decoration`
  characters (inclusive ranges), the `min_rule_run` that makes a rule, and
  whether a labeled line's rules shorten (`shorten_rules`). Its `Default`
  is the framework's choice: box drawing, block elements, dashes and rule
  characters, a rule of four, and shortening on.
- `richtext::LineFit` is how one line fits under smart wrap: `Smart` (the
  settings' own fit), `Cut` (clipped at the edge), `Wrap` (word-wrapped as
  `Plain` does), `RightAlign` (leading padding shrunk until the line ends at
  the edge, on one row; text that alone is wider fits the smart way), or
  `Drop` (left out).
- `richtext::layout_capture_with(lines, mode, source_width, width, smart,
  fit)` lays a capture out with those settings, asking `fit` once per line
  under `Smart` only, before trailing blank lines are dropped, so a dropped
  line leaves no blank rows below the newest output. `layout_capture` is
  the same call with the defaults and every line fitted the smart way.

## Rationale

A source's screen has quirks the framework cannot know: a status line one
program right-aligns behind padding wider than the area, a hint not worth a
row. The host knows which program a capture came from and which lines are
its quirks; the framework knows how to fit a line. Taking the settings as a
value and the fit as a closure keeps the knowledge on each side, and lets a
host make both configurable without the framework reading configuration.

## Consequences

- A host that wants the old behavior calls `layout_capture` and changes
  nothing.
- A host that matches lines by pattern brings its own matcher; the framework
  adds no dependency for it.
- The `Plain` and `None` modes ignore the settings and the fits.

## Alternatives considered

- **Patterns in the framework.** The framework would take a list of
  patterns and fits, and need a matcher it has no other use for; a closure
  lets the host match however it likes.
- **A fit per row instead of per line.** Rows exist only after the fit has
  run.

## Origin

- host ADR 0252, "Smart-wrap settings and cleanup rules come from
  configuration": decision 7 (the settings and the fit as values) and the
  fits of decision 2.
