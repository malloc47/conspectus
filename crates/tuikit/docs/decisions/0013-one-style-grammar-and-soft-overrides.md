# 0013: Style values have one grammar, and overrides never stop a program

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/theme.md](../controls/theme.md) |

## Decision

- **One grammar.** A style value is a string: a named color (the eight ANSI
  names and their `bright_` forms, `light_gray` for ANSI 7, `default` for the
  terminal's own), `ansi256:N`, or `#RRGGBB`; an optional background after
  `on` (`white on dark_gray`, `on blue`); and modifiers joined by commas
  (`bold`, `dim`, `italic`, `underline`, `reversed`, `crossed_out`), alone or
  after a color. Whitespace is trimmed and case ignored.
- **A soft walk.** The framework applies a table of `(key, value)` pairs that
  the host reads from its own configuration format, and returns the
  problems. An unknown key or a value that does not parse is reported and
  keeps its default; nothing aborts.
- **Styles never change a width.** Widths are measured on text; a style
  wraps what was measured, and the mark that ends a cut takes the style of
  the text it ends.
- **No downgrade at parse time.** A truecolor value stays truecolor;
  ratatui and the terminal degrade it.

## Rationale

The vocabulary is the one users already know from terminal tools: ANSI
names, a 256-color index, hex as most modern configuration files spell it,
and comma-joined modifiers as common styling libraries print them. A broken
value should leave a working program in its default look with a visible
warning, not a failed start. Keeping the walk in the framework gives every
host the same rules.

## Consequences

- The host owns its configuration file's format, its tables, and the
  wording of the diagnostics it shows.
- Theme presets, honoring `NO_COLOR`, and downgrading to the terminal's color
  depth are on the [roadmap](../roadmap.md).

## Alternatives considered

- **Nested tables per category.** More typing, no easier to scan; key
  prefixes carry the same grouping.
- **Named variants with a selector** (`theme = "dark"`). Solves palette
  switching, a different need; it can be layered on later.
- **Environment-variable overrides.** The key set would flood the
  environment; files are easier to share.
- **Detecting the terminal's background.** Unreliable over remote shells and
  terminal multiplexers.
- **A third-party theme crate.** None matched the roles the controls need.

## Origin

- host ADR 0032, "The interface's theme configuration": decisions 3 and 4
  (the grammar and the soft validation), and decision 6 (presets and
  variants deferred).
- host ADR 0153, "Theme role tokens, alias keys, and named styles":
  decisions 6 and 8 (backgrounds with `on`, and the configuration walk in
  the framework).
- host ADR 0022, "Terminal color and styling for table output": the named
  color vocabulary, and measuring cells on unstyled text so styling never
  changes a width.
