# ADR 0032: TUI Theme Configuration

## Status

Accepted

## Context

ADR 0022 established the table renderer's palette and explicitly deferred
TUI theming with the note: "A future TUI surface (if any) would build on
the same `Style` values, so the renderer's color choice is the 'canonical'
source." That deferral has come due. The Phase 8 TUI is live and the
[styling overhaul plan](../../plans/let-s-brainstorm-improvements-to-cozy-bengio.md)
aims to make `conspectus tui` visually competitive with agent-deck and
lunemis/mux: dense header chips, multi-section detail dividers (ADR 0033),
recency color buckets, harness badges. All of that depends on a
centralized palette that the operator can override.

Today every TUI color is a `Color::*` literal scattered across `ui.rs`,
`rows/sessions.rs`, and the overlay widgets. There is no `Theme` type,
no config schema, and no way for an operator to change a harness color
or remap the mux glyph colors. The light-theme work
(c17bf1e, d93cde3, 44e1770) only relocated selection highlighting to
`REVERSED`; the foreground palette stayed hardcoded.

Three things need to be settled at once because they share a single
config-file shape and a single in-process `Theme` value:

1. **Schema shape** — flat `[tui.theme]` map, nested per-element, or
   variant-keyed (`[tui.theme.dark]` / `[tui.theme.light]`).
2. **Color spec grammar** — what syntactic forms the value of each
   theme key accepts (named ANSI, indexed 256-color, truecolor, modifier
   sets).
3. **Validation policy** — how the loader handles unknown keys,
   malformed values, and incompatible terminals.

This ADR settles those. The companion ADR 0033 covers how the resulting
`Theme` is consumed by the new detail-pane section model; the styling
overhaul plan covers downstream call-site changes.

## Decision

### 1. Centralized `Theme` value

Introduce `src/tui/theme.rs` exposing a single `Theme` struct that owns
every color and modifier the TUI render reads. `Theme::default()`
reproduces today's inline literals byte-for-byte so existing buffer
snapshots stay stable when the indirection lands.

The struct is plumbed through `App` (one owned field, accessor
`app.theme()`). The renderer reads from `app.theme()` rather than
referencing module-level constants.

The `Theme` is not exposed through `conspectus::api`; it is internal to
the TUI. The table renderer keeps the ADR 0022 palette (constructed
independently) for plain-text CLI output. Where the two palettes
overlap (PR states, short-id blue, dash placeholder) the two values
agree by default — but the TUI palette is a superset and operators
override it independently of the table CLI.

### 2. Config schema — flat `[tui.theme]` map

Add a single sibling table under `[tui]`:

```toml
[tui.theme]
harness_claude   = "magenta"
harness_codex    = "cyan"
harness_opencode = "green"
harness_aider    = "red"
harness_unknown  = "default"

recency_fresh    = "bright_green"
recency_active   = "green"
recency_recent   = "yellow"
recency_cold     = "dim"

mux_attached     = "green"
mux_ambiguous    = "yellow"
mux_unmuxed      = "dim"

selection_active   = "reversed,bold"
selection_inactive = "bold"

cwd_mark           = "cyan"
link_id            = "blue"
placeholder        = "dim"
divider            = "dim"
warning            = "yellow"
error              = "red"
success            = "green"
panel_focus_accent = "cyan"

pr_open   = "green"
pr_closed = "red"
pr_merged = "magenta"
pr_draft  = "yellow"

badge      = "reversed,bold"
```

Every key is optional. Absent keys retain the `Theme::default()` value.
Unknown keys produce a `ConfigWarning` (per the existing
`outcome.warnings` channel used by `[tui.views.*]`) and are otherwise
ignored — they do **not** abort load. This matches the precedent set by
`[tui.views.<name>.filters].mux_state` and friends in `src/config.rs`.

The flat shape is locked rather than nesting by category
(`[tui.theme.harness] claude = "magenta"`) because:

- The set of keys is small (~25), shallow lookups beat nested ones for
  scanning a config file.
- Most operators will override a handful of keys, not entire
  categories; the flat layout makes single-key overrides terse.
- Categories already exist implicitly via key prefixes (`harness_*`,
  `recency_*`, `mux_*`, `pr_*`), so the visual grouping in a config
  file does not require physical nesting.

### 3. Color spec grammar

Each value is a string with the following accepted forms:

- **Named ANSI (16-color)**: `"black" | "red" | "green" | "yellow" |
  "blue" | "magenta" | "cyan" | "white"` plus the `"bright_*"`
  variants (`"bright_red"`, etc.). Mapped to `ratatui::style::Color`'s
  `Black`/`Red`/…/`LightRed` constants. The string `"default"` maps to
  `Color::Reset` (terminal-default fg).
- **256-color indexed**: `"ansi256:N"` where `0 ≤ N ≤ 255`. Mapped to
  `Color::Indexed(N)`.
- **Truecolor hex**: `"#RRGGBB"` (case-insensitive). Mapped to
  `Color::Rgb`. Operators on 16-color-only terminals will see ratatui's
  built-in degradation; we document the caveat under operations.md but
  do **not** silently downgrade at parse time. Truecolor expressivity
  is opt-in by the operator.
- **Modifier-only fields** (selection, badge, recency_cold,
  placeholder, divider): comma-joinable subset of `"bold" | "dim" |
  "italic" | "underline" | "reversed" | "crossed_out"`. Empty string
  means "no modifier" (renders default fg). The same modifier names
  are valid as a suffix on color values (e.g.,
  `"bright_green,bold"`) — the parser splits on the first comma,
  resolves the color, then OR-folds the remaining tokens into a
  `Modifier`.

Whitespace around tokens is trimmed. Casing is normalized to
lowercase before matching. Any token that does not match a known form
produces a per-key warning and falls back to the default.

Reasons for this grammar:

- Named ANSI is the same vocabulary ADR 0022 already publishes; it is
  the lowest-friction surface for operators who do not care about
  terminal capabilities.
- `ansi256:N` exposes the 256-color palette the table renderer already
  uses (e.g., the dim-dash `ansi256:244` from ADR 0022) without
  inventing a new namespace.
- `#RRGGBB` matches the spelling every other modern config tool uses
  (Helix, Alacritty, Zellij, Starship). The lift to support it is
  trivial because ratatui exposes `Color::Rgb` natively.
- Comma-joined modifiers are the same form anstyle's `Effects`
  serializes to (`bold,italic`). Operators copying snippets between
  config files for related tools will recognize the shape.

### 4. Validation and warnings

The config loader (`src/config.rs`) treats theme parsing as a
soft-failure layer:

- An unparseable spec produces `Warning { key: "tui.theme.<field>",
  message: "<reason>" }` and the field's default value is kept.
- Unknown keys under `[tui.theme]` produce a single warning naming the
  offending key.
- Warnings are surfaced through the existing `outcome.warnings`
  channel and displayed on the TUI status bar at startup (existing
  warnings infrastructure handles this).
- The TUI never aborts on theme errors. A bad config produces a
  working default-theme TUI with a visible warning, not a
  startup-failure dialog.

No external schema-validation tooling is required; the parser is
hand-rolled in the same file as the rest of the `[tui]` block to keep
it close to its siblings.

### 5. Interaction with ADR 0022

ADR 0022's table palette lives in `anstyle::Style` form inside
`src/output/table.rs`. This ADR does **not** alter the table palette
or its renderer:

- `conspectus table` continues to use the ADR 0022 palette regardless
  of `[tui.theme]`.
- `conspectus tui` reads `[tui.theme]` and ignores any future
  `[table.theme]` block (none exists today).
- Where the two palettes overlap, the *defaults* match (e.g.,
  `pr_open` is green in both, `link_id` is blue in both). The intent
  is visual consistency between table and TUI without coupling the
  two configuration surfaces.

A future ADR may unify the two palettes if operators ask for shared
theme configuration. That is out of scope here.

### 6. Variants and presets — deliberately deferred

This ADR ships the schema only. It does **not**:

- Ship preset theme variants (Tokyo Night, Dracula, Solarized). Those
  are tracked in backlog item `T8-023` as config snippets the
  operator can paste into their `[tui.theme]` block. Adding a preset
  system later is purely additive over this schema.
- Support a `[tui.theme.<name>]` variant-keyed schema with a
  top-level `theme = "name"` selector. The flat schema is enough for
  v1; nothing in it precludes adding variant keying later behind a
  separate ADR.
- Detect light vs dark terminals at runtime. Selection still relies
  on `REVERSED` (per the c17bf1e/d93cde3 work) so the default theme
  is legible on both. Operators who want per-terminal palettes can
  ship per-machine `.conspectus.toml` overrides.

## Consequences

- TUI styling becomes a single-source value (`Theme`) instead of
  ~50 scattered color literals. The styling-overhaul plan's Phase 1
  collapses to "extract literals → `Theme` defaults" without
  touching behavior.
- Operators can override any palette element by adding 1–25 lines to
  their `~/.config/conspectus/config.toml` or project
  `.conspectus.toml`. No rebuild, no CLI flag, no environment
  variable.
- The `[tui.theme]` block follows the same warning posture as the
  rest of `[tui]`: malformed config never breaks the TUI, it
  surfaces as a status-bar warning.
- Binary size is unchanged. No new dependencies — `ratatui` already
  provides `Color` and `Modifier`; the parser is ~100 LOC of
  hand-rolled string matching.
- Future work has clear seams: theme presets (T8-023) become config
  snippets, runtime theme switching (`:set theme dark`) becomes a
  reducer message that swaps the `Theme` in `App`, table↔TUI palette
  unification gets its own ADR.

## Alternatives Considered

- **Nested `[tui.theme.<category>]` schema.** Rejected. The category
  set is small enough that nesting adds typing without improving
  scannability; key prefixes (`harness_*`, `mux_*`) carry the same
  information.
- **Variant-keyed `[tui.theme.<name>]` with a top-level selector.**
  Rejected for v1. Solves a different problem (named palette swaps)
  than the v1 need (single-key overrides). Can layer on top of this
  schema later if operators ask.
- **Environment-variable overrides (`CONSPECTUS_TUI_HARNESS_CLAUDE=
  magenta`).** Rejected. The 25-key surface would explode env-var
  namespace; config-file overrides are easier to share across
  machines via dotfiles.
- **Adopt `anstyle::Style` everywhere, including in the TUI.**
  Rejected. `ratatui::Style` is the native renderer type; converting
  through `anstyle` would add a translation layer for no behavioral
  gain. ADR 0022's table renderer keeps `anstyle` because its sink
  is ANSI byte output; the TUI's sink is a `ratatui::Buffer`.
- **Ship preset variants in v1.** Rejected. Variants are
  configuration *content*, not configuration *schema*; shipping them
  in this ADR would couple two unrelated decisions. Backlog item
  T8-023 carries them as follow-on work.
- **Detect terminal background and auto-select a palette.** Rejected.
  The detection (OSC 11 query, `COLORFGBG` env) is unreliable across
  multiplexers and SSH; the user-confirmed scope keeps selection on
  the `REVERSED` modifier (light/dark-safe by construction) and
  leaves palette choice to the operator.
- **Use a third-party theme crate (e.g., `syntect` color schemes).**
  Rejected. No existing crate matches the TUI's vocabulary
  (per-harness, per-mux-state, per-PR-state); a thin schema in our
  own code is smaller and avoids a dep policy decision.

## Open Questions Answered

- The TUI theme is centralized in `src/tui/theme.rs` and configured
  under `[tui.theme]`, not derived from `[table]` or `[colors]`.
- The schema is flat — one key per palette element — not nested by
  category.
- The color spec grammar accepts named ANSI, `ansi256:N`,
  `#RRGGBB`, and comma-joined modifier suffixes.
- Validation is soft: bad values produce status-bar warnings and
  fall back to defaults; the TUI never aborts on theme errors.
- Preset variants (Tokyo Night etc.) and variant-keyed schemas are
  deliberately deferred to follow-on ADRs / config snippets.
- ADR 0022's table palette is unchanged and continues to govern
  `conspectus table` output.
