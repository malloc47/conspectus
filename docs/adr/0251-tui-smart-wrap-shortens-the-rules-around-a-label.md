# ADR 0251: Smart Wrap Shortens The Rules Around A Label

## Status

Accepted. Amends [ADR 0106](0106-preview-pane-wrap-modes.md) (decision 2,
the smart mode) as the framework carries it since
[ADR 0181](0181-tui-rich-text-is-a-framework-feature-and-the-preview-scrolls-in-a-region.md).
Framework-owned: restated in the framework's record 0058. No new key, setting,
or write path.

## Context

Smart wrap keeps an agent's rules and box borders on one row by cutting a
line at the Preview's edge when everything past the edge is decoration or
blank. Claude Code writes the session's name near the right end of the rule
above its prompt (`───…─── claude ─`, as wide as the pane). The label sits
past the edge of a narrower Preview, so the overflow is not all decoration.
The line has no padding to squeeze either, so it word-wraps, and its
unbroken run of `─` breaks into rows of rule fragments: three rows for a
155-column rule in a 60-column Preview. The operator (2026-11-18) saw the
rules still spilling and expected smart wrap to keep them on one row. Other
full-width banners with a label wrap the same way: pytest's
`===== 3 passed in 0.21s =====`, a box top with its title at the right end.

## Decision

1. **A rule is a run of one decoration character at least four cells
   long.** The decoration characters are the ones smart wrap already cuts
   (box drawing, block elements, dashes and rule characters). Shorter runs
   are content: `--flag`, `===`, `-->`.
2. **Shorten rules when the overflow carries a label.** When a line does not
   fit, its overflow is not all decoration, and squeezing its padding cannot
   fit it, smart wrap shortens its rules, widest first, each to no fewer
   than one cell, until the line fits on one row. The label and everything
   else on the line stay as they are.
3. **A shortened rule keeps evenly spaced cells.** A run drawn in two
   styles, such as a progress bar's done and remaining parts, keeps their
   proportions.
4. **Otherwise wrap as before.** When even one-cell rules cannot fit the
   line (a label wider than the Preview), the line word-wraps untouched.

## Consequences

- Claude Code's labeled prompt rule takes one row and keeps its label:
  `──── claude ─`. A centered banner keeps its words between shorter rules.
- A line whose overflow is only decoration is still cut at the edge;
  padding still squeezes first; the plain and none modes are unchanged.
- A table whose separator row carries a non-decoration character (`|---|`)
  now shortens its dashes rather than wrapping. Its other rows were already
  squeezed, so the columns were already out of line.
- A line that needs both its padding squeezed and its rules shortened to
  fit still wraps; neither is tried with the other.

## Alternatives Considered

- **Cut the line at the edge and lose the label.** The label can be the only
  content on the line (a test summary), and a rule cut short of it reads as
  an unlabeled rule.
- **Drop rule lines that carry a label.** The rule separates the agent's
  output from its prompt; ADR 0106 kept that separation on purpose.
- **Shorten any run of decoration, however short.** Two- and three-cell
  runs are common in content (`--all-targets`, `===`, `->`); shortening
  them changes what the line says.
- **Shorten the runs in proportion to their length.** It keeps a progress
  bar's parts in proportion across runs too. Widest first is how padding
  already squeezes, and it leaves the short tail of a labeled rule alone.
