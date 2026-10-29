# `theme`

| | |
| --- | --- |
| Module | `tuikit::theme` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: the theme is data a control reads; [`Theme::apply_overrides`](#applying-a-configuration-table) reports the entries it could not apply |

## Purpose

`Theme` holds the design tokens every control draws with, so a host changes
the look of all of them in one place and no control carries a color of its
own. It has three parts:

- **Typed tokens** for the roles the framework's controls draw: the accent,
  muted and inert text, the selection pair, the status colors, links, search
  matches, chips, the recency scale, surfaces, and input fields. Each is a
  public field (`theme.accent`), so the compiler checks every use.
- **Named styles** the host registers for roles only it knows, such as one
  color per kind of record or per state of an external item. A control that
  draws such a role is handed its name and looks it up.
- **Aliases**: extra configuration keys that also set a token, so an older
  configuration keeps loading after tokens are merged or renamed.

The module also owns the style-spec grammar (`parse_color`,
`parse_modifier`, `parse_style_spec`) and the walk that applies a
configuration table. The host keeps everything else: where the table comes
from, its file format, the names and defaults of its own roles, the aliases
its history needs, and the wording of its diagnostics.

The token list is written once, as a table inside the module. The fields,
their defaults, their configuration keys, and `Theme::keys()` all come from
that table, so a token cannot be added to one and missed in another. A host
that lists its keys for its users (a reference page, a preview) reads
`keys()` and `aliases()` instead of keeping its own copy.

## Usage example

```rust
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use tuikit::theme::{OverrideValue, StyleSpec, Theme};

// The host builds its theme once: the framework defaults, its own roles,
// and the aliases older configuration files still use.
let mut theme = Theme::default();
theme.register("state_open", Color::Green, "an open item's state glyph");
theme.register("chip_fill", StyleSpec::on(Color::White, Color::Black), "a filled chip");
theme.register_alias("faint", "muted");

// It applies the table it read from its configuration, converting each
// value to an `OverrideValue`, and reports what did not apply.
let table = [
    ("accent", OverrideValue::Text("magenta")),
    ("faint", OverrideValue::Text("italic")),
    ("state_open", OverrideValue::Text("#22aa66")),
];
for error in theme.apply_overrides(table) {
    eprintln!("theme key `{}` {}", error.key, error.problem);
}

// A control reads typed tokens directly and named roles by name.
let hint = Span::styled("Tab: complete", Style::default().fg(theme.accent));
let note = Span::styled("(optional)", Style::default().add_modifier(theme.muted));
let state = Span::styled("open", theme.style("state_open"));
```

### Applying a configuration table

`Theme::apply_overrides` takes `(key, OverrideValue)` pairs: `Text` for
colors, modifiers, and style specs, `Integer` for widths, and `Other` for
anything else the host's format holds. Each key is a typed token, a
registered name, or an alias. Within one table:

1. A target takes the value of its own key when that key is present and
   valid.
2. Otherwise it takes the value of the first of its aliases, in registration
   order, that is present and valid.
3. Otherwise it keeps its value.

A value that does not parse is reported (`OverrideProblem::Invalid`,
`NotText`, or `NotWidth`) and treated as absent, so the next key in line
applies. An alias whose value lost to a different value is reported as
`Shadowed`; aliases that agree are not. An unknown key is `UnknownKey`. A
later table overrides an earlier one whichever key each used, so a host
layers a user file and a project file by applying them in order.

The theme starts with one alias of its own: `accent` also sets
`panel_focus_accent`, so the focus cue follows the accent unless a table
sets the cue itself.

### The style-spec grammar

| Form | Example | Means |
| --- | --- | --- |
| Named color | `red`, `bright_green`, `dark_gray`, `light_gray` | an ANSI color |
| Terminal default | `default`, `reset` | the terminal's own color |
| Indexed color | `ansi256:42` | a color from the 256-color palette |
| True color | `#ff8800` | an RGB color |
| Modifier list | `bold`, `dim,italic` | added emphasis |
| Style spec | `green,bold`, `white on dark_gray`, `on blue,italic` | colors (`fg`, `fg on bg`, or `on bg`) then modifiers |

A color key takes only a color, a modifier key only a modifier list, and a
style key any of the forms. Names ignore case and surrounding spaces.

## Key table

None: the theme handles no keys.

## Theme tokens

The typed tokens and their defaults. `Theme::keys()` lists them with a short
description of each.

| Token | Kind | Default | Used for |
| --- | --- | --- | --- |
| `accent` | color | cyan | the primary accent: popup borders and titles, the key column of a help page, a chosen chip, scroll markers, completion hints |
| `panel_focus_accent` | color | cyan | the focused pane's title marker (house rule 9); follows `accent` in a table that does not set it |
| `disclosure` | color | cyan | expand and collapse arrows |
| `muted` | modifier | `dim` | text that should recede: hints, placeholders, notes, rules, footers |
| `inert` | color | dark gray | text that must recede even where `dim` draws nothing, and gone or disabled items |
| `selection_active` | modifier | `reversed,bold` | the selected row of a focused list |
| `selection_inactive` | modifier | `bold` | the selected row of an unfocused list |
| `success` | color | green | success, something that exists or is on |
| `warning` | color | yellow | warnings and pending changes |
| `error` | color | red | errors and refusals |
| `info` | color | green | informational messages |
| `link` | color | blue | identifiers and links |
| `match_highlight` | color | yellow | text a search matched, drawn bold and underlined (`Theme::match_style`) |
| `badge` | modifier | `reversed,bold` | the chip look |
| `badge_width` | width, at least 2 | 8 | a chip's label width, in cells |
| `recency_fresh` | style | `bright_green,bold` | the recency scale, freshest step |
| `recency_active` | style | `green` | the recency scale, second step |
| `recency_recent` | style | `yellow` | the recency scale, third step |
| `recency_cold` | style | `dim` | the recency scale, oldest step |
| `surface` | color | `default` | the background of a floating surface (a modal frame, a toast) |
| `toast_text` | color | white | a toast's text; its border takes its severity's color |
| `field` | style | `light_gray` | an input field's value when the field is not focused |
| `field_active` | style | `white on dark_gray` | an input field's value when the field is focused |
| `field_cursor` | style | `black on white` | the cell under the text cursor |

Named-style lookups: `Theme::style(name)`, `Theme::color(name)`, and
`Theme::modifier(name)` read a registered name (or a typed token by its key)
and return an empty style, `None`, or no modifiers when nothing is stored
there, so a control never fails on a role its host did not register.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | Supplies `selection_active` and `selection_inactive`; the `selection` module applies them |
| 9 | Focus cue | Supplies `panel_focus_accent`, which follows `accent` unless set |
| 10 | Unfocused cursor | Supplies the inactive half of the selection pair |
| 11 | Feedback severity | Supplies `info`, `success`, `warning`, and `error` |
| 13 | Theme | Defines the tokens every control takes; `muted` and `inert` are the two muted looks; the raw-style guard (`tests/raw_styles.rs`) fails on a raw `Color::` or `Modifier::DIM` anywhere else in the crate |
| 14 | Width | `badge_width` counts terminal cells |

Not applicable: 2 to 8, 12, and 15 to 18 govern how controls move, confirm,
close, page, edit, and label; the theme only supplies their colors.

## Seams left open

- **Presets.** A preset is a configuration table; a host can ship one and
  apply it before the user's own table. Nothing in the theme needs to change
  for that.
- **Light and dark variants.** Defaults rely on reversed video and the
  sixteen ANSI colors so they read on either; a host that detects the
  terminal background can apply a variant table first.
- **More token kinds.** `TokenKind` and `TokenValue` are closed enums today;
  a new kind (an underline color, say) adds a variant and a grammar form.
- **Runtime switching.** A theme is a plain value; a host can rebuild and swap
  it between frames.
