# ADR 0153: TUI Theme Roles, Alias Keys, And Named Styles

## Status

Accepted. Amends ADR 0032 (TUI theme configuration): the `Theme` value
moves into the framework crate's `theme` module (ADR 0150) as typed role
tokens plus a registry of named styles, five muted keys become two, and
the older keys load as aliases. Applies house rule 13 of ADR 0151.

## Context

ADR 0032 gave the TUI one `Theme` struct with a field per `[tui.theme]`
key. By the time the framework extraction began (ADR 0150) it had 41
fields, and three problems blocked moving it into a crate that may not
name Conspectus's domain:

- **Domain and generic tokens were mixed.** 19 fields were generic roles
  (accent, selection, status colors, muted text); 22 were Conspectus roles
  (harness colors, mux state, PR state, node kinds, edge states). Some
  generic roles wore domain names (`link_id` colored mux names,
  `mux_unmuxed` was just a dim, `panel_focus_accent` was the accent of
  every popup).
- **Four tokens and a raw modifier meant "muted".** `placeholder` and
  `divider` (DIM), `mux_unmuxed` (DIM), `secondary_text` and `ghost`
  (dark gray), and 21 raw `Modifier::DIM` sites. The forms took no theme
  at all and hard-coded 17 colors, so `[tui.theme]` never reached them.
- **The key list lived in three hand-kept places** (the struct, a
  `known_keys` table, and `set_*` match arms) with a one-way drift test;
  `docs/operations.md` documented about half the keys.

## Decision

1. **Typed tokens in the framework, named styles from the host.**
   `tuikit::theme::Theme` holds typed fields for the roles the framework's
   controls draw: `accent`, `panel_focus_accent`, `disclosure`, `muted`,
   `inert`, `selection_active`, `selection_inactive`, `success`,
   `warning`, `error`, `info`, `link`, `match_highlight`, `badge`,
   `badge_width`, the four `recency_*` steps, `surface`, `field`,
   `field_active`, and `field_cursor`. Conspectus registers its own roles
   in the theme's named-style map (`register(name, value, about)`) and
   reads them through accessors on its `tui::Theme`, which wraps the
   framework theme and dereferences to it. Domain roles keep their key
   names (`pr_open`, `node_repo`, `mux_attached`, …); per-harness colors
   are `harness.<key>`, which `[tui.theme.harness]` sets.

2. **Two muted looks.** `muted` is a modifier (`dim` by default) for text
   that should recede: hints, placeholders, notes, rules, footers.
   `inert` is a color (dark gray by default) for text that must recede
   even where `dim` draws nothing, and for gone or disabled items. The
   split follows the old defaults, so the default look does not change:
   `placeholder`, `divider`, and `mux_unmuxed` become aliases of `muted`;
   `secondary_text` and `ghost` become aliases of `inert`. Every raw
   `Modifier::DIM` in TUI code reads `muted`.

3. **Alias keys and their precedence.** The framework supports alias keys
   that also set a token or name. Within one table, a target takes its own
   key's value when present and valid, otherwise the first valid alias in
   registration order; a value that fails to parse is reported and
   skipped. Conspectus registers: `placeholder`, `divider`, `mux_unmuxed`
   → `muted` (in that order); `secondary_text`, `ghost` → `inert`;
   `link_id` → `link`; `panel_focus_accent` → `accent`; and the flat
   `harness_claude`, `harness_codex`, `harness_opencode`, `harness_aider`,
   `harness_pi` → `harness.<key>`. An alias that loses to a different
   value produces a diagnostic naming the winner, so an operator who set
   several older keys differently learns they merged. A later config file
   overrides an earlier one whichever key each used.

4. **Accent and focus cue.** `accent` colors popup borders and titles,
   help keys, the active tab, chosen chips, and hints; `panel_focus_accent`
   is the focused pane's marker (house rule 9). The framework makes
   `accent` also set `panel_focus_accent` unless a table sets it, and
   Conspectus makes `panel_focus_accent` also set `accent`, so an older
   file that set only `panel_focus_accent` looks as it did.

5. **New tokens with today's defaults.** The forms' fixed fills become
   `field` (light gray), `field_active` (white on dark gray), and
   `field_cursor` (black on white); the command chip's white text becomes
   the host role `command_badge_text`; the toast background is `surface`
   (the terminal's own); `info` (green, the color Info entries already
   used) separates informational messages from `success`. The PR node
   kind's `Color::Reset` sentinel becomes the `pr_open` hue every caller
   already substituted.

6. **Grammar.** A style value may name a background with `on`
   (`white on dark_gray`, `on blue`, `black on white,bold`), and
   `light_gray` names ANSI 7. Every value the older grammar accepted
   parses the same.

7. **One key list.** The framework declares its tokens once, in a table
   that generates the fields, defaults, keys, and `Theme::keys()`; the
   host declares its roles, harness colors, and aliases in one table each.
   The config loader, the widget preview, and a two-way test against the
   `[tui.theme]` example in `docs/operations.md` (every key documented
   with its default, every documented key real, the older-key table equal
   to the alias list) all read those tables.

8. **The configuration walk lives in the framework.** The host converts
   its TOML into `(key, value)` pairs and formats the problems the walk
   returns; it still owns `[tui.theme.icons]`, the validation of
   `[tui.theme.harness]` keys against the adapter registry, and the
   wording of its diagnostics.

9. **A guard.** A framework test fails on `Color::`, `Modifier::DIM`, or
   ratatui's `Stylize` shorthands in non-test framework source outside the
   theme module.

## Consequences

- The default theme draws byte-identical frames before and after; forms,
  previews, and the transcript viewer now follow `[tui.theme]`.
- A custom theme that set only one of the merged keys now changes more:
  setting `placeholder` also restyles rules and the unlinked glyph, and
  setting `secondary_text` also recolors gone rows. Setting `success` no
  longer recolors Info entries in Messages; `info` does.
- Within one file, a `[tui.theme.harness]` entry now beats the flat
  `harness_<name>` key it shadows (the flat key used to win by
  alphabetical accident).
- Domain roles are looked up by name at render time; the accessors keep
  call sites typed and the names in one module.
- Preview loaders, which run off the UI thread, take the theme's `muted`
  modifier rather than the theme.

## Alternatives Considered

- **A theme that is only a map of named styles.** Adopters could add roles
  freely and the loader would be trivial, but the framework's own controls
  would lose compile-time checks on the tokens they read. Rejected; the
  named map is kept for host roles only.
- **`muted` as a full style spec (colors and modifiers).** More expressive,
  but `muted` is combined with other emphasis at about 60 sites, and
  half the old keys were colors and half modifiers; a modifier `muted`
  and a color `inert` keep every old value meaning what it meant.
- **Keep the five keys separate.** No behavior change for custom themes,
  but it keeps four ways to say "recede" and copies them into the
  framework. Rejected by house rule 13.
- **Last key wins, in file order.** TOML tables are read in sorted order,
  so "last" would be alphabetical and arbitrary. Rejected for a fixed,
  documented precedence.
- **Warn on every alias.** Would nag every older config into churn. Only
  a conflicting alias is reported.
- **Map the forms' fills to `inert` and reversed video.** Would read
  better on light terminals but changes the default look; the new field
  tokens keep it and let an operator choose.

## Open Questions Answered

- *What happens to an operator who set several older muted keys
  differently?* They merge by the documented precedence, and each losing
  key gets a warning naming the winner.
- *Do older configs still load?* Yes: every key the TUI accepted before
  this change loads, and a test checks the full older list.
- *Which tokens are the framework's and which are Conspectus's?* Section 1
  lists them; the framework's control page (`crates/tuikit/docs/controls/theme.md`)
  is the reference for the typed tokens.
