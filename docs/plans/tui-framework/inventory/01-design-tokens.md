# Inventory 01: Design Tokens and Primitives

Slice: theme, glyphs, chips, popup chrome, text-width helpers, cursor math,
wrap and snippet fitting, scrollbars, age formatting, hint and footer
formatting, plus the consistency tests that guard them. Everything below was
read from the `worktree-tui-framework` checkout; paths are relative to the
repo root and line numbers refer to that checkout. Counts come from `grep`
over `src/tui/` with test files (`*_tests.rs`, `consistency_tests/`) excluded
unless stated. "Raw" means a style, color, or modifier literal that does not
come from `Theme`.

Reading guide: the primitives in this slice are functions, constants, and one
config-facing struct rather than stateful controls, so "state struct" and "key
handling" entries are mostly N/A. The only key-adjacent items are the cursor
stepping helpers (section 7) and the footer/key-label formatting (section 12);
the key tables themselves belong to the interaction-conventions inventory.
Tests are listed per section and summarized in section 15. Each section ends
with a framework verdict using the rubric labels (extract as-is, extract after
refactor, split into generic core plus Conspectus adapter, stays in
Conspectus); section 18 collects them.

## 0. Headline numbers

| Measure | Count | Where |
| --- | --- | --- |
| `Theme` fields | 41 (19 generic tokens, 22 Conspectus-domain) | `src/tui/theme.rs:36-149` |
| `[tui.theme]` keys | 43 in `Theme::known_keys` (33 Color, 6 Modifier, 4 StyleSpec) + 3 special keys (`icons`, `harness`, `badge_width`) | `theme.rs:335-511`, `src/config.rs:1348-1432` |
| Raw `Color::` literals in production code | 20 lines (17 in `widgets/form.rs`, 2 in `widgets/pin_edit.rs`, 1 in `widgets/badge.rs:44`), plus 2 `Color::Reset` (`toast.rs:60`, `icons.rs:227` sentinel) | see section 2 |
| Raw `Modifier::` literals outside `theme.rs`/`snapshot.rs` | BOLD 74, REVERSED 26 (about 20 are cursor rows), DIM 21, ITALIC 13 | section 2 |
| `Style::default()` / `Style::new()` call sites | 235 across 31 files (builder start points; only 109 `.fg(theme...)` and 28 `.add_modifier(theme...)` follow them) | section 2 |
| Glyph literals duplicated instead of using `icons::marks`/`state` | ~25 (13 `▲`, 4 `►`, 3 `√`, 2 `×`, 2 `◄` lines, 1 `▼`/`▶` pair) | section 3.4 |
| `centered_modal_rect`-style sizing rules | 13 distinct rules; the `min(cap, w-4).max(n)` formula appears 10 times (plus a clamp variant in `messages.rs`) | section 5.3 |
| Hand-built popup frames bypassing `themed_popup` | 4 (`row_menu`, `mux_menu`, `worktree_menu`, `clean_up`) | section 5.2 |
| Truncate/ellipsize/pad implementations | 13 (see section 6) | section 6 |
| Word-wrap implementations | 6 (5 hand-written + ratatui's) | section 6.3 |
| Scrollbar call sites | 5 (4 through shared helper, 1 copy in `widgets/help.rs:273-286`) | section 10 |
| Age/elapsed formatters | 4 in TUI (`format_recency`, `format_elapsed`, `age_label`, `relative_epoch`) + 1 CLI (`output/render.rs:1168`) | section 11 |
| "Muted text" mechanisms | 4 theme tokens (`placeholder`, `secondary_text`, `ghost`, `divider`) + 21 raw `Modifier::DIM` + 5 local `dim()` helpers | section 2.3 |

## 1. Theme (`src/tui/theme.rs`, 729 LOC; `theme_tests.rs`, 150 LOC)

### 1.1 Public surface

- `pub struct Theme` (Clone, Debug, PartialEq, Eq), 41 `pub` fields, `Default`
  at `theme.rs:197-265`. No builder, no setters beyond the three config-facing
  `set_color(&str, Color) -> bool`, `set_modifier(&str, Modifier) -> bool`,
  `set_style_spec(&str, StyleSpec) -> bool` (`theme.rs:516-593`).
- Two convenience methods only: `Theme::match_style()` (`:271`, fg
  `match_highlight` + BOLD|UNDERLINED, used at 8 call sites) and
  `Theme::harness_color(&str)` (`:284`, 4 call sites outside theme.rs).
  Everything else is read as a bare field (about 235 reads, tabulated in 1.2),
  and each call site rebuilds its own
  `Style::default().fg(theme.x).add_modifier(...)` (109 `.fg(theme...)` and 28
  `.add_modifier(theme...)` sites). There is no semantic-style layer
  (`theme.accent_style()`, `theme.muted()`, ...).
- `pub struct StyleSpec { color: Option<Color>, modifier: Modifier }`
  (`:157`), `const fn fg / modifier / fg_mod`, `into_style()`. Only the four
  `recency_*` fields use it (via `RecencyBucket::style`, `rows/mod.rs:1150`).
- `ThemeKeyKind {Color, Modifier, StyleSpec}` and `ThemeKey {name, kind}`
  (`:315-329`), `Theme::known_keys() -> &'static [ThemeKey]` (`:335`).
- Pure string parsers with no `crate::` imports: `parse_color(&str) ->
  Result<Color, String>` (`:599`; named 16 ANSI + `bright_*` + `default|reset`,
  `ansi256:N`, `#RRGGBB`), `parse_modifier` (`:660`; 9 modifier names plus
  aliases), `parse_style_spec` (`:694`; `"color"`, `"color,mod,mod"`, `"mod,mod"`).
- Plumbing: the loaded `Theme` lives on `TuiConfig.theme` (`src/config.rs:301`),
  is copied into `App`, and widgets take `&Theme` explicitly
  (`App::theme()` at `app.rs:993`). No globals, no thread-locals. The popup and
  menu widgets all take `&Theme` (or an `Option<&Theme>`); `widgets/form.rs`,
  `widgets/launch_spec_form.rs`, `widgets/path_omnibox.rs` and all seven
  `preview/*.rs` modules take none (zero "theme" mentions), which is why they
  hard-code colors (section 2).

### 1.2 Field table: generic token vs Conspectus domain

Read counts are reads of `theme.<field>` outside `theme.rs` and tests.

| Group | Field(s) | Default | Reads | Class | Notes |
| --- | --- | --- | --- | --- | --- |
| Accent | `panel_focus_accent` | Cyan | 23 | generic | Popup border (`popup_frame.rs:65`, 4 hand-built popups), focus marker (`ui.rs:334`), key column in help (`help.rs:~640`), chip divider label, in-flight chips, choice pills. The name is wrong for its job: it is the framework's primary accent. |
| Selection | `selection_active`, `selection_inactive` | REVERSED+BOLD, BOLD | 2 + 1 | generic, but under-used | Only `left_panel.rs:1089-1091` and `messages.rs:243` read them; 26 other cursor rows hard-code `Modifier::REVERSED` (section 13). |
| Muted | `placeholder` (Modifier DIM) | DIM | 51 | generic | The most-used token. Documented as unreliable: `DIM` is a no-op in some terminals (theme.rs:68-71, 86-91). |
| Muted | `secondary_text` (Color) | DarkGray | 30 | generic | Created because DIM is unreliable (theme.rs:86-91). |
| Muted | `ghost` (Color, drawn italic) | DarkGray | 4 | generic ("inert/disabled") with a domain name | Used for gone/unstarted entities; `ghostify` lives in `ui/left_panel.rs:894`. |
| Muted | `divider` (Modifier) | DIM | 4 | generic | Rule lines in `ui/detail_header.rs:302,318`. |
| Muted | `disclosure` (Color) | Cyan | 1 | generic | Tree arrow color; read once. |
| Status | `warning`, `error`, `success` | Yellow, Red, Green | 13, 6, 3 | generic | `form.rs`/`pin_edit.rs` ignore `warning` and hard-code `Color::Yellow` (7 sites); the toast ignores `success` (section 16). |
| Reference | `link_id` | Blue | 7 | generic | "identifier/link" hue. |
| Highlight | `match_highlight` | Yellow | via `match_style()` x8 | generic | Search-match style (fg + BOLD+UNDERLINED). |
| Chip | `badge` (Modifier), `badge_width` (usize) | REVERSED+BOLD, 8 | 2, 7 | generic | `badge_width` is a layout token inside a color theme; `search.rs:1160` hard-codes 10 (`CHIP_WIDTH`) and ignores it. |
| Heat scale | `recency_fresh/active/recent/cold` (StyleSpec) | LightGreen+BOLD, Green, Yellow, DIM | 1 each | generic scale, domain bucket thresholds | Thresholds (5 min, 1 h, 1 d) live in `rows::recency_bucket` (`rows/mod.rs:1130`), not in the theme. |
| Harness identity | `harness_colors: BTreeMap<String, Color>`, `harness_unknown` | claude-code Magenta, codex Cyan, opencode Green, aider Red, pi LightBlue; unknown White | 0 direct, `harness_color()` x4; unknown x1 | domain | Lookup walks `crate::discovery::harness::registered_adapters()` (`theme.rs:288`). Defaults name `aider`, which milestone m-24 removes. |
| Mux state | `mux_attached` (Color), `mux_unmuxed` (Modifier) | Green, DIM | 5, 4 | domain | |
| Cwd | `cwd_mark` | Cyan | 3 | domain | The `(cwd)` tag on a group header. |
| PR state | `pr_open/closed/merged/draft` | Green, Red, Magenta, Yellow | 11, 6, 6, 6 | domain (a generic "status palette" in disguise) | Mapping from PR state string to color is re-implemented at 9 sites in 6 files (section 17, item 11). |
| Node kind | `node_workspace/repo/checkout/agent_session/mux_session/runtime_process/branch/fork` | LightBlue, Blue, Cyan, LightGreen, Magenta, DarkGray, Green, LightMagenta | 1 each (via `node_kind_style`) | domain | `NodeKind::Pin` has no field and borrows `node_mux_session` (`icons.rs:223`); `ForgePr` has none (`Color::Reset` sentinel). |
| Edge state | `edge_alt_of`, `edge_conflict` | DarkGray, Yellow | 1 each | domain | Alias the `secondary_text` and `warning` hues by default. |
| Pin chip | `command_badge` (Color, bg) | Black | 2 | domain | Pane-command chip on mux rows. |
| Icons | `icons: IconOverrides` | empty | 3 | domain | Keyed by `crate::model::NodeKind`. |
| Dead | `pin_placeholder` | LightYellow | 0 | dead | Kept only so old config files load (`theme.rs:76-79`). |

Observations:

- Four tokens default to `Color::DarkGray` (`secondary_text`, `ghost`,
  `node_runtime_process`, `edge_alt_of`). `placeholder` and `divider` and
  `mux_unmuxed` all default to `Modifier::DIM`. Three of the four "muted"
  concepts are therefore visually identical in the default theme; they differ
  only in what a user could change.
- The token *vocabulary* is partly named after widgets or domain events:
  `panel_focus_accent` (really "primary accent"), `placeholder` (really
  "muted"), `link_id`, `cwd_mark`, `pin_placeholder`.
- 19 generic / 22 domain by field count; by read count the generic tokens
  carry roughly 165 of 230 reads.

### 1.3 Configuration and override mechanism

- Source: TOML `[tui.theme]`, parsed in `src/config.rs` (`merge_tui_theme`
  `:1348-1432`, `merge_tui_theme_icons` `:1438-1475`, `merge_tui_theme_harness`
  `:1483-1539`), documented in `docs/operations.md:248-320` and ADR 0032.
  User config and project `.conspectus.toml` layer through the normal config
  merge. Test coverage: 13 `tui_theme_*` tests in `src/config_tests.rs:812-1070`.
- Flat key table: each key is one of `Color` / `Modifier` / `StyleSpec` kind,
  routed by `known_keys`. Values are strings. Three keys are special-cased
  *before* the table lookup: `icons` (sub-table), `harness` (sub-table, keys
  validated against `discovery::harness::harness_keys()`), `badge_width`
  (integer, minimum `MIN_BADGE_WIDTH = 2`).
- Diagnostics are soft: unknown key, wrong type, or unparseable value produces
  a `ConfigDiagnostic` and keeps the default. The TUI never aborts on theme
  errors (ADR 0032 section 4).
- Five legacy flat aliases (`harness_claude`, `harness_codex`, `harness_opencode`,
  `harness_aider`, `harness_pi`) write into the `harness_colors` map
  (`theme.rs:523-537`).
- **Maintenance cost:** three hand-synchronised lists per token (struct field,
  `known_keys` row, a `set_*` match arm). The only guard is
  `known_keys_cover_every_settable_field` (`theme_tests.rs:134`), which checks
  one direction (every listed key is settable) and not that every field is
  listed. `pin_placeholder` shows the drift: field exists, key exists, nothing
  reads it.
- **Documentation gaps:** `docs/operations.md` documents neither
  `[tui.theme.icons]`, `[tui.theme.harness]`, nor any `node_*`/`edge_*` key;
  it calls the focus marker `▸` (`docs/operations.md:287`) where the code
  draws `►`.
- **No presets and no light/dark logic.** ADR 0032 section 6 deliberately
  defers variants; CSP-269 ("Ship preset theme variants", milestone m-13) is
  still To Do. The TUI does not detect terminal background; it relies on
  `REVERSED` for selection. The `examples/pantry.rs` "Theme" ingredient
  (`:485-740`) renders every key against `Theme::default()` only and notes
  that dark/light variants are future work. Every test in the repo that builds a
  theme uses `Theme::default()`; `badge.rs:96` is the only test that mutates one
  (`badge_width`). There is no test that renders under a non-default palette.
- **No runtime switching, no `NO_COLOR`/monochrome handling in the TUI.** The
  CLI table renderer resolves `--color`/`NO_COLOR`/`CLICOLOR`
  (`src/cli/mod.rs:273-330`, ADR 0022) with `anstyle`; the TUI has no
  equivalent. ADR 0032 section 5 keeps the two palettes independent.

Is the mechanism generic? The *grammar* (`parse_color`, `parse_modifier`,
`parse_style_spec`, `StyleSpec`, `ThemeKeyKind`) is fully generic and has no
Conspectus imports. The *loader* is generic in shape but lives in
`src/config.rs` and carries three Conspectus special cases plus the
`ConfigDiagnostic` type. The *container* (`Theme` with 41 typed fields and
string-keyed setters) is not generic: a framework consumer cannot add a token
without editing the struct, `known_keys`, and `set_*`.

### 1.4 Conspectus coupling ("leaks") in `theme.rs`

| Leak | Where | Class |
| --- | --- | --- |
| `use crate::tui::icons::IconOverrides` (field `icons`) | `theme.rs:20,148` | (b) adapter: IconOverrides is keyed by `model::NodeKind` |
| `Theme::harness_color` walks `crate::discovery::harness::registered_adapters()` to map label to key | `theme.rs:288-290` | (b) adapter: harness identity palette |
| Domain fields: harness, mux, pr, node, edge, command_badge, cwd_mark, pin_placeholder | `theme.rs:45-146` | (b) adapter, or (a) a generic named-palette map as `harness_colors` already is |
| Defaults named in ADR numbers and "Phase 3" prose | doc comments | cosmetic |
| `DEFAULT_BADGE_WIDTH = 8` justified as "longest registered harness label (`opencode`)" | `theme.rs:22-24` | (a) parameterize; default should be a design constant |
| `Theme` is `pub use`d from `tui/mod.rs:53` and referenced by `config.rs`, `examples/pantry.rs` | | (b) |

External crates: `ratatui::style::{Color, Modifier, Style}` used directly (no
shim). ADR 0032 explicitly rejected `anstyle`, theme crates, and
environment-variable overrides.

### 1.5 Framework verdict (Theme)

**Split into generic core plus Conspectus adapter.**

- Extract as-is: `StyleSpec`, `parse_color`, `parse_modifier`,
  `parse_style_spec`, `ThemeKeyKind`/`ThemeKey` and their tests.
- Extract after refactor: a framework `Theme` (name candidate: `Palette` or
  `Theme`) holding only the 19 generic tokens, renamed by role (`accent`,
  `muted`, `muted_fg`, `inert`, `rule`, `status_{ok,warn,err}`, `reference`,
  `match_`, `selection_{active,inactive}`, `chip`, `chip_width`, `heat[4]`),
  plus semantic style constructors (`theme.style(Token::Accent)`) so call sites
  stop recomposing `Style::default().fg().add_modifier()`.
- Conspectus-side adapter: `ConspectusPalette` (harness, mux, pr, node, edge,
  command_badge, cwd_mark, icons) either as a second struct that holds a
  `&Theme`, or as a generic `named: BTreeMap<String, StyleSpec>` extension map
  in the framework theme. The config loader (`merge_tui_theme*`) splits the
  same way: generic table walk in the framework, domain keys in a callback.
- Replace the three hand-synchronised lists with one table (macro or
  `const` array of `(name, kind, getter, setter)`) so `known_keys` cannot drift.

## 2. Theme bypasses (raw style in production code)

### 2.1 Raw colors (20 lines)

| File:line | Literal | Purpose | Should map to |
| --- | --- | --- | --- |
| `widgets/form.rs:71` | `fg(White).bg(DarkGray)` | active entry field fill (`entry_style`) | a surface/input token; also contradicts the "no fixed bg" rule in the selection comments (`controls.rs:1133`, `search.rs:1299`, `theme.rs:10-14`) |
| `form.rs:73` | `fg(Gray)` | inactive entry | `secondary_text` |
| `form.rs:105-111` | `fg(Cyan).bg(DarkGray)+BOLD`, `fg(Cyan)+BOLD` | scroll-edge `<`/`>` markers in a text window | accent |
| `form.rs:114` | `fg(Black).bg(White)` | text-cursor cell | inverse/cursor token |
| `form.rs:252-254, 282-284` | DarkGray `?`, Green `√`, Yellow `!` | path validity mark, duplicated verbatim in two functions | `secondary_text`, `success`, `warning` + `marks::OK` |
| `form.rs:264` | Cyan | Tab-completion hint | accent |
| `form.rs:342, 489, 513` | Yellow | suffix, warning rows | `warning` |
| `form.rs:386` | Cyan+BOLD | selected harness chip | accent |
| `pin_edit.rs:798, 836` | Yellow | override mark, warning style | `warning` |
| `widgets/badge.rs:44` | `fg(White)` | `command_badge` text color (bg is themed) | chip-foreground token |
| `widgets/toast.rs:60` | `toast_bg(Color::Reset)` | toast background | n/a; border color is hard-coded green inside the upstream crate, so `theme.success` is not honored (`toast.rs:13-21`) |

Roughly 60 percent of raw color is one file, `widgets/form.rs`, which owns the
three multi-field forms (mux launch, new mux, pin edit). It takes no `Theme`
parameter at all.

### 2.2 Raw modifiers (outside `theme.rs` and `snapshot.rs`)

| Modifier | Count | Notes |
| --- | --- | --- |
| BOLD | 74 | mostly structural emphasis (labels, titles, active row). 13 in `ui/left_panel.rs`, 12 in `ui/right_panel.rs`. Never themed; arguably a base typographic token ("emphasis"). |
| REVERSED | 26 | 20 are cursor/selection rows that should read `selection_active` (section 13); 2 are `remove_modifier(REVERSED)` for scrollbars (`left_panel.rs:207`, `help.rs:277`); rest are chip detection (`left_panel.rs:896`). |
| DIM | 21 | `preview/value.rs:34`, `forge.rs:68`, `git.rs:113,223`, `path.rs:82,100,136`, `process.rs:133` (8, all in modules that take no theme); `form.rs:226,377,392,462,467,477,509,521` (8); `controls.rs:796`; `pin_edit.rs:655,838`; `pane_viewer.rs:416,458`. Equivalent token is `theme.placeholder` (51 themed uses). |
| ITALIC | 13 | ghost look (`left_panel.rs:889,899,904`), previews (`left_panel.rs:1204,1348,1509`), help legend (`help.rs:700,807`), form hints (`form.rs:462-490`), `preview/path.rs:114`. The "ghost = italic + color" and "preview = dim italic" combinations are not tokens. |

### 2.3 Local style helpers (near-copies of "dim"/"bold")

`fn dim(...)` is defined 5 times with 3 different signatures:
`preview/value.rs:33` (Span), `preview/forge.rs:67` (Span), `preview/git.rs:112`
(Line), `preview/process.rs:132` (Line), `widgets/pane_viewer.rs:413`
(`dim(theme) -> Style`, the only one that consults the theme, and it combines
`theme.secondary_text` with a raw `Modifier::DIM`, a fifth "muted" recipe).
`bold` is defined twice (`preview/value.rs:29`, `preview/forge.rs:63`). Ten
inline `let dim = Style::default()...` bindings appear elsewhere (`form.rs`,
`pin_edit.rs`, `path.rs`, `help.rs`, ...).

## 3. Glyphs (`src/tui/icons.rs`, 514 LOC; `icons_tests.rs`, 338 LOC)

### 3.1 What exists

| Set | Where | Members | Class |
| --- | --- | --- | --- |
| Node-kind glyphs | `impl NodeKind { theme_key, default_glyph }` `icons.rs:29-66` | `▦ ◆ ◇ ● ▣ ◈ ⌥ ⋎ ⇄`, plus `Pin` borrowing `state::PIN_RUNNING` | domain (inherently; keyed by `model::NodeKind`) |
| State glyphs | `mod state` `:74-114` | `⊸ ∘ ⊘ ? ○ ◐ ⊕ ⊗ ◉ ◎`, `ALL: [&str; 10]` | domain (linked/unlinked, PR states, pin states) except `UNKNOWN` which is generic |
| Marks | `mod marks` `:119-142` | `▼ ▶ ▲ ◌ — √ × • ► ◄`, `SPINNER: [&str; 8]` (Braille) | mixed: generic = EXPANDED, COLLAPSED, WARNING, NO_VALUE, OK, FAIL, BULLET, POINTER, BACK, SPINNER; domain = UNTRACKED |
| Registry | `all_glyphs()` `:147-178` | de-duplicated union of the above | glyph table / docs test input |
| Resolved style | `NodeKindStyle {glyph, color, width}` + `node_kind_style(kind, &Theme)` `:190-211` | | domain |
| Overrides | `IconOverrides` (BTreeMap<NodeKind, String>) `:235-259`, `parse_icon_override(key, raw)` `:272-300` | only node-kind glyphs can be overridden; width must be exactly 1 | domain keys, generic validation |
| Vetting | `VETTED: &[(&str, &str)]` `:320-486` (62 entries), `glyph_risk()` `:491-506`, `EMOJI_CAPABLE` deny-list `:510-514`, `scripts/glyph-widths.py` | | generic |

Glyphs drawn but not in `icons.rs`: arrows `← ↑ → ↓` (footer labels, key
tables), `·` (separator, 139 occurrences), `…` (54), `›` (breadcrumb,
`explorer.rs:995`), box-drawing `─` rules (5 hand-rolled `repeat` sites), `└`
(`form.rs:479`). All are in `VETTED`.

### 3.2 Width handling and fallback

- Width is policed by tests, not by types: `default_glyphs_are_all_one_cell_wide`,
  `state_glyphs_are_one_cell_wide_and_all_different`,
  `vetted_glyphs_have_no_width_or_emoji_risk`, `risky_glyphs_are_rejected`
  (`icons_tests.rs:57,219,262,277`). `every_glyph_the_sources_draw_is_vetted`
  (`:299-338`) scans every non-comment non-test `src/tui` source line for any
  non-ASCII character and requires it in `VETTED`. This is a strong, generic,
  reusable guard.
- `glyph_risk` rejects width != 1, multi-char, ZWJ/variation selectors, the
  emoji block, and a deny-list of text-presentation emoji candidates. `▶`
  (U+25B6) is explicitly exempted "by operator preference" (`icons.rs:499-502`)
  even though U+25B6 is in `EMOJI_CAPABLE`. The Braille spinner is likewise
  kept "by operator preference; some fonts draw it past one cell"
  (`VETTED` rows 1-8).
- `IconOverrides` is the only runtime glyph customization: node-kind glyphs
  only; state glyphs, marks, spinner, and key arrows are not overridable.
- **There is no ASCII/fallback mode** (grep for `ascii`, `NO_COLOR`, `nerd`,
  `COLORTERM` in `src/tui` finds nothing relevant), no per-terminal font
  detection, and no Nerd Font path other than the manual override table.
  ADR 0073 chose geometric Unicode so none is needed.
- `ui.rs:11-13` header comment still describes `◉`/`◯` mux glyphs (stale).

### 3.3 Docs tables kept in step by tests

`docs/tui-columns.md` carries three machine-checked tables
(`consistency_tests/docs.rs`, 5 tests):

1. Glyph table (`## Glyphs`, 30 rows): `every_glyph_the_left_panel_can_draw_is_in_the_glyph_table_and_nothing_else_is`
   compares the first-column backticked tokens with `icons::all_glyphs()`.
2. State-word table (`## State words`, 12 rows): compared with `rows::words::ALL`
   (`rows/mod.rs:246-265`; words `live, in mux, resumable, stopped, removed,
   missing, planned, unknown, open, draft, merged, closed`) and the status
   labels produced by `SlotStatus::label_for` / `NodePresence::label_for`.
3. Headings per `View` and `RowKind` (exhaustive `match`es force a compile error
   when a view or row kind is added).
4. Example frames are re-rendered by `tests/tui_columns_doc.rs`
   (needs the `snapshot` feature and the built binary; `just tui-columns`
   regenerates them).
5. `the_reference_speaks_the_operators_language` bans ADR/CSP numbers, "slot",
   and "gone" in the document.

The *Age* section of the doc (4-cell right-aligned, `Ns/Nm/Nh/Nd`, `—`, color by
bucket) is prose only: `is_age` in `consistency_tests/rendering.rs:31` accepts
`w` as a unit and `…`, so the doc and the check are not identical.

### 3.4 Constants versus literals (the glyph table is not the single source)

Uses of `icons::marks::*` outside `icons.rs` and tests: `EXPANDED` 1,
`COLLAPSED` 1, `WARNING` 1, `UNTRACKED` 1, `NO_VALUE` 4, `OK` 1, `BULLET` 1,
`SPINNER` 1, **`FAIL` 0, `POINTER` 0, `BACK` 0**. Meanwhile literals:

| Glyph | Literal sites outside `icons.rs` | Examples |
| --- | --- | --- |
| `▲` | 13 | `detail.rs:477,755,1170`, `messages.rs:27`, `ui/header.rs:128,239`, `ui/right_panel.rs:347,564,669`, `widgets/help.rs:727`, `worktree_menu.rs:570,595,625` |
| `►` | 4 | `ui.rs:333`, `row_menu.rs:138`, `mux_menu.rs:123`, `worktree_menu.rs:552` (while `marks::POINTER` is unused) |
| `◄` | 2 lines | `ui/right_panel.rs:255,268` (`marks::BACK` unused) |
| `√` | 3 | `messages.rs:26`, `form.rs:253,283` (`marks::OK` used once in `multi_select.rs`) |
| `×` | 2 | `messages.rs:28`, `preview/forge.rs:135` |
| `▼`/`▶` | 1 pair | `ui/right_panel.rs:666` |
| `—` (NO_VALUE) | ~65 | `explorer.rs` 25, `detail.rs` 12, `controls.rs` 5, and others; only 4 use `marks::NO_VALUE` |
| `…` | 54 | no constant exists |
| `·` | 139 (19 as the exact `" · "` literal) | no constant; `ui/header.rs:87-89` defines `CHIP_SEPARATOR`/`SECTION_SEPARATOR` locally |

`LogLevel::glyph()` (`messages.rs:24-30`: `√ ▲ ×`) is a separate generic
"status level" glyph set that duplicates three `marks` entries.

Cursor marker glyphs are covered in section 13.

### 3.5 Framework verdict (glyphs)

**Split.** Move `marks` (generic ones), the `VETTED` table, `glyph_risk`,
`EMOJI_CAPABLE`, the width/emoji tests, the sources-are-vetted scanner, the
`parse_icon_override` width-1 validator (made generic over the key type), and
`scripts/glyph-widths.py` into the framework as a `glyphs` module and test
harness. Keep `NodeKind` glyphs, `state` (linked/stopped/PR/pin), `UNTRACKED`,
`NodeKindStyle`, `node_kind_style`, `IconOverrides` and the `[tui.theme.icons]`
loader on the Conspectus side as an adapter that registers its glyphs with the
framework's vetting table. Before extraction, route all literal duplicates in
3.4 through constants and add `ELLIPSIS`, `SEPARATOR`, `ARROW_*` constants. Decide
whether the vetting table is mandatory for adopters (open question 8).

## 4. Badges and chips (`src/tui/widgets/badge.rs`, 149 LOC, 7 inline tests)

### 4.1 `badge.rs`

- `harness_badge(label: &str, theme: &Theme) -> Span<'static>` (`:30`): fg
  `theme.harness_color(label)` + `theme.badge` (REVERSED|BOLD) over a
  fixed-width label.
- `command_badge(command: &str, theme: &Theme) -> Span<'static>` (`:42`): fg
  hard-coded `Color::White`, bg `theme.command_badge`, BOLD.
- private `badge_span(label, style, width)` (`:52-61`): width =
  `max(theme.badge_width, 2)`, counts **chars** not cells
  (`label.chars().count()`, `chars().take(width-1)` + `…`), output
  `" {label:<width$} "` in a single styled span so padding carries the fill.
  Footprint is always `badge_width + 2` (10 cells by default).
- Callers: `harness_badge` 7 sites outside `badge.rs` (`left_panel.rs:752,1126,
  1292,1304,1395`, `search.rs:1245`, `ui/header.rs:111`), `command_badge` 2
  sites (`left_panel.rs:754,1293`). Variants in play: harness pill
  (per-harness color), program pill (fixed bg), dimmed pill for ghost rows
  (`ghostify`: `left_panel.rs:894-907` detects chips by
  `bg.is_some() || REVERSED`).
- Leaks: `theme.harness_color` (registry walk), concept of "harness" and
  "command" in the function names. Everything else is generic: a fixed-width
  filled pill.

### 4.2 Every other "chip" or pill form (none shares code with `badge.rs`)

| Form | Where | Look | Notes |
| --- | --- | --- | --- |
| Section chip divider | `ui/detail_header.rs:281-322` `chip_divider_line(label, suffix, width, theme, ChipAnchor)` | `" label "` with fg `panel_focus_accent` + `theme.badge` (REVERSED) between `─` rules; dim suffix | the only chip that reuses `theme.badge`; uses `chars().count()` |
| Choice pill | `widgets/search.rs:690-707` `choice_segments` | `[ Sessions ]` in accent + BOLD + REVERSED vs plain word | hard-codes `BOLD | REVERSED` instead of `theme.badge` |
| Harness choice tokens | `widgets/form.rs:364-408` `harness_chips` | ` [claude]` Cyan+BOLD when selected, DIM otherwise | raw colors; budget-aware with ` …` overflow |
| Button text | `controls.rs:1149,1161` | `[ Edit ]`, `[ Cycle ]` appended to a row | plain text |
| Checkbox text | `form.rs:565` `checkbox` returns `[x]`/`[ ]`; `controls.rs:1189` `box_glyph` the same literal; `multi_select.rs:~286` uses `marks::OK`/`marks::BULLET` via the upstream crate | three checkbox vocabularies | |
| Status-bar chips | `ui/header.rs:266-316` (`tmux:off`, `gh:...`, in-flight spinner chips) | warning or dim text, accent+BOLD for in-flight | no fill |
| Count/state chips in rows | `left_panel.rs` (`N hits`, `was in <mux>`, ` no agent `) | dim text | |
| Key chips | `pane_viewer.rs:542` `"(a) attach · (?) help · (q) close"` | hand-written text, not generated from the key table | contrast with `overlay::footer()` |

### 4.3 Verdict

**Extract after refactor** as `Chip`/`Pill`: a `fixed_width_pill(label, style,
width_cells)` that measures in cells (unicode-width) rather than chars, with
variants `Filled` (REVERSED over fg color), `FilledBg(fg, bg)`, `Bracketed`,
`Dim`. `harness_badge`/`command_badge`/`CHIP_WIDTH` stay Conspectus-side as
thin adapters. Merge `chip_divider_line`'s chip, `choice_segments`, and
`harness_chips` into the same primitive. Fix `search.rs:1160` `CHIP_WIDTH = 10`
to read `theme.badge_width + 2`.

## 5. Popup chrome (`src/tui/widgets/popup_frame.rs`, 127 LOC, no direct tests)

### 5.1 Surface

- `centered_rect(area, width, height) -> Rect` (`:43`): 13 callers
  (`multi_select.rs:317`, `mux_menu.rs:145`, `help.rs:876,886`,
  `row_menu.rs:174`, `picker.rs:182`, `controls.rs:1273`, `input.rs:167`,
  `messages.rs:118`, `worktree_menu.rs:661`, `clean_up.rs:130`, `form.rs:638`,
  `search.rs:1415`). It is the one shared piece of rect math; seven of the
  callers are widget-local `centered_modal_rect`/`modal_rect` wrappers that
  only choose width and height.
- `themed_popup(body, title: Line, &Theme) -> tui_popup::Popup` (`:58`):
  `Borders::ALL`, `border_style = fg(theme.panel_focus_accent)`. The module
  comment (`:24-26`) says "border + title both inherit panel_focus_accent";
  the code sets only the border style, so the title is whatever the caller's
  `Line` carries.
- `LinesBody {lines, inner_width, inner_height}` (`:75-95`): **zero callers.**
- `wrapped_lines(text, indent, width) -> Vec<String>` (`:99`): thin wrapper over
  `crate::viewer::render::word_wrap` (a layering inversion: `tui::widgets`
  depends on the transcript viewer). 4 callers (`row_menu.rs:158,165`,
  `controls.rs:795`, `search.rs:1086`).
- `ellipsize(text, width) -> String` (`:110`): width-aware end truncation with
  `…`. 8 callers (`search.rs` x7, `row_menu.rs:155`). Not unit-tested anywhere.
  Unlike `ui/text.rs::truncate_to_width_strict` it returns `"…"` for `width == 0`
  and keeps `…` outside the width budget when `width == 1`.
- The popup plumbing is `tui_popup::Popup` (crate `tui-popup 0.7.6`): it
  `Clear`s, draws a `Block` with `Borders`/`border_set`/`border_style`/`title`
  and renders a `KnownSize` body centered. The body must implement
  `KnownSize` (width, height).

### 5.2 Frame construction patterns (17 frames drawn four ways)

| Pattern | Sites | Look |
| --- | --- | --- |
| `themed_popup` (tui_popup, accent border) | 11: `picker.rs:195`, `controls.rs:812`, `multi_select.rs:222`, `messages.rs:132`, `help.rs:145`, `input.rs:121`, `mux_launch.rs:652`, `new_mux.rs:204`, `pin_edit.rs:743`, `search.rs:742`, `value_modal.rs:178` | plain border, accent color |
| Hand-built `Clear` + `Block::default().borders(ALL).title(" X ").border_style(accent)` | 4: `row_menu.rs:174-179`, `mux_menu.rs:146-150`, `worktree_menu.rs:662-666`, `clean_up.rs:131-135` | identical result to `themed_popup`, but built by hand, each computing its own rect |
| Unthemed fallback `tui_popup::Popup::new(body).title(title)` | 2: `input.rs:124`, `multi_select.rs:225` (taken when no theme is attached; no production caller omits it) | upstream defaults, uncolored border |
| Pane frames `Block::default().borders(ALL).title(...)` with **no** border style | 2: `ui/left_panel.rs:8-10`, `ui/right_panel.rs:6-8` | border is the terminal default; focus is shown only by a `► ` title marker and the reversed selected row. ADR 0076 claims the border is highlighted on the focused pane; the code does not. |

Every frame uses `Borders::ALL` and the default `BorderType::Plain`; no
rounded/thick/double border is drawn anywhere (the `╭╮╰╯╔╗╚╝` entries in
`VETTED` and in the layout test's `corner()` predicate are only for detection).

KnownSize boilerplate: 11 hand-written `impl KnownSize for XBody` blocks whose
`width()`/`height()` return two stored `inner_width`/`inner_height` fields
(`messages.rs:142`, `input.rs:139`, `picker.rs:206`, `help.rs:233`,
`form.rs:649,679`, `search.rs:761`, `controls.rs:1091`, `popup_frame.rs:81`,
`value_modal.rs:193`, `multi_select.rs:242`). `tui_popup` ships
`KnownSizeWrapper<W>` for exactly this; it is used nowhere.

Title conventions are caller-owned: padding is embedded in the string
(`" Help "`, `" Mux "`, `" harness "`, `" max age "` from `controls.rs:543-591`),
added by the widget (`format!(" {} ", title)` in `row_menu.rs:178`), added with
a count (`picker.rs:183`, `" {title} ({n}/{m}) "`), or built from spans with
two spaces and an accent-bold label (`value_modal.rs:171`, `messages.rs:122`).
Of the 14 overlays drawn from `ui::draw`, 12 have plain titles and two (value
modal, messages) have accent+bold labels; the search title embeds interactive
`[ Choice ]` pills (section 4.2).

### 5.3 Sizing rules (13 variants, `centered_rect` is the only shared math)

| Overlay | Width | Height | Margin rule |
| --- | --- | --- | --- |
| Help (`help.rs:873`) | `min(100, w-4).max(40)` | `h-4`, min 10 | -4 |
| Value modal, Help-style (`help.rs:882`) | `min(78, w-4).max(40)` | `28.clamp(10, h-2)` | -4 / -2 |
| Controls (`controls.rs:1268`) | `min(64, w-4).max(40)` | content+2 clamp(8, h) | -4 / 0 |
| Text input (`input.rs:165`) | `min(60, w-4).max(20)` | 3 | -4 |
| Multi-select (`multi_select.rs:310`) | `min(60, w-4).max(20)` | items+2 clamp(3, h-4) | -4 |
| Picker (`picker.rs:179`) | `min(80, w-4).max(30)` | `(h-4).clamp(8, 20.max(..))` | -4 |
| Search (`search.rs:74-86`) | 85% of w, 60..200, `<= w-2` | 80% of h, 12..50, `<= h-2` | -2, percent-based |
| Messages (`messages.rs:116`) | `(w-4).clamp(40, 110)` | `h-2`, min 10 | -4 / -2 |
| Row menu (`row_menu.rs:171`) | `72.min(w)` | content+2 clamp(8, h) | 0 |
| Mux menu (`mux_menu.rs:140`) | `62.min(w)` | content+2 clamp(10, h) | 0 |
| Worktree menu (`worktree_menu.rs:661`) | `56.min(w)` | content+2 clamp(5, h) | 0 |
| Clean up (`clean_up.rs:130`) | `68.min(w)` | content+2 `.min(h)` | 0 |
| Forms via `form::modal_rect` (`form.rs:634`) | `min(cap, w-4).max(44)` caps 76 (mux launch), 76 (new mux), 84 (pin edit) | content+2 clamp(min_h, h) | -4 |

`min(cap, w-4).max(n)` appears 10 times as source text (controls, form,
input, help x2, picker, multi-select, mux_launch, new_mux, pin_edit; messages
uses a `clamp` variant); the three forms (`mux_launch.rs:503`, `new_mux.rs:177`,
`pin_edit.rs:643`) each repeat the formula a second time to derive the inner
width, so the rule must be kept in step at two places per form. Four menus use
`min(w)` (flush to the edges on narrow terminals) while the others leave a
2-cell margin.

### 5.4 Verdict

**Extract after refactor** as `ModalFrame` (name candidate). Keep
`centered_rect`. Replace the 13 sizing rules with one `ModalSize {width:
Cap|Percent, height: Content|Fixed|Percent, margin}` helper. Fold the 4
hand-built frames into `themed_popup`. Delete `LinesBody`; adopt
`KnownSizeWrapper` (or a framework `SizedBody`) for the 11 identical
`KnownSize` impls. Normalize title padding and style in one place. Move
`word_wrap` into the framework so `wrapped_lines` stops importing
`crate::viewer::render`. Add direct unit tests (there are none for
`centered_rect`, `ellipsize`, `wrapped_lines`, `themed_popup`).

## 6. Width, truncation, padding, wrapping

### 6.1 `src/tui/ui/text.rs` (210 LOC, all `pub(super)`, borrows `ui.rs` imports via `use super::*`)

Generic: `truncate_to_width` (`:6`, alias of strict), `truncate_to_width_strict`
(`:10`, end ellipsis, width 0/1 handled, cell-aware), `truncate_to_width_middle`
(`:40`, left-biased middle ellipsis), `truncate_to_width_no_marker` (`:86`,
hard cut; 1 caller), `pad_to_width` (`:103`), `fit_spans_to_width` (`:111`,
truncates a span list to a budget). Domain: `compact_path_label` (`:135`,
special-cases `"Ungrouped"` and the workspace double-space format),
`compact_path_secondary` (`:160`), `compact_mux_label` (`:186`),
`compact_mux_native_id` (`:196`, 36/28/6 character constants).
Call-site counts: strict 16, pad 9, middle 2, no_marker 1, fit_spans 3,
`truncate_to_width` 5, all inside `ui/left_panel.rs`/`ui/right_panel.rs`.
Tests: `ui_tests.rs:686-700` (middle), several row-shape tests; no direct tests
for strict/pad/fit_spans.

### 6.2 Every truncation or fit implementation in the repo

| # | Location | Measures | Marker | Pads | Semantics that differ |
| --- | --- | --- | --- | --- | --- |
| 1 | `ui/text.rs:10` `truncate_to_width_strict` | cells | `…` inside budget | no | width 0 -> "", 1 -> "…" |
| 2 | `ui/text.rs:40` `truncate_to_width_middle` | cells | middle `…` | no | left half preferred |
| 3 | `ui/text.rs:86` `truncate_to_width_no_marker` | cells | none | no | hard cut (session ids) |
| 4 | `widgets/popup_frame.rs:110` `ellipsize` | cells | `…` | no | width 0 returns "…" (overflows) |
| 5 | `widgets/form.rs:579` `fit` | **chars** | `…` | yes | pads; used by `pin_edit.rs:716-721` |
| 6 | `widgets/form.rs:573` `truncate_chars` | chars | none | no | 5 callers in form.rs |
| 7 | `widgets/badge.rs:54` `badge_span` | chars | `…` | yes | inside the styled span |
| 8 | `explorer.rs:1856` `truncate` | chars | `…` | no | 8 callers (`:1185-1534`) |
| 9 | `preview/process.rs:110` (inline) | chars | `…` appended after MAX chars | no | result is MAX+1 chars |
| 10 | `ui/header.rs:339` (inline, find chip) | chars | `…` after 23 chars | no | |
| 11 | `widgets/pane_viewer.rs:521` (inline, find query) | chars | `…` after 16 chars | no | |
| 12 | `widgets/snippet.rs:72` `head`, `crop` | cells | `…` | no | window around a match |
| 13 | `preview_wrap.rs:209` `truncate` | cells | none | no | styled cell lines |
| (CLI, not TUI) | `output/render.rs:1135` `truncate_to_width`, `cli/search.rs:977-1050` `crop_around` | cells | `…` | no | near-copies of #1 and #12 |

Char-counting variants (5-11) miscount wide and combining characters; the cell
variants (1-4, 12-13) agree with each other except for the width-0/1 edges.
The same arithmetic is also open-coded:

- `spans_width(spans)` (`left_panel.rs:1525`, 21 callers) and `spans_cells`
  (`header.rs:584`) are identical; ratatui's `Line::width()`/`Span::width()` do
  the same. `search.rs:1419` `cells(text)` is a third.
- `ui.rs:341` `wrapped_line_count` estimates wrapped rows as
  `div_ceil(width)` per line (ignores word boundaries), and drives scrollbar
  `content_length` and header heights (3 callers); ratatui's own wrapping can
  produce more rows.
- `format!("{:<w$}")` / `{:>w$}` padding appears ~79 times and counts chars.

### 6.3 Word-wrap implementations (6)

| # | Location | Unit | Styled? | Notes |
| --- | --- | --- | --- | --- |
| 1 | `viewer/render.rs:554-645` `word_wrap`, `word_wrap_with_budgets`, `hard_break`, `wrap_styled_line` | cells | plain + styled | used by `popup_frame::wrapped_lines` and 6 sites in `widgets/help.rs`; lives in the transcript viewer module |
| 2 | `widgets/form.rs:593` `wrap_words` | chars | no | whitespace-collapsing; used by `pin_edit.rs:669` |
| 3 | `widgets/form.rs:520` `wrap_footer` | chars | dim lines | splits on `" · "` segments, not words; 3 callers |
| 4 | `preview_wrap.rs:165` `word_wrap` | cells | per-cell style | hanging indent, `smart_rows`, `squeeze_padding`, `hard_wrap` |
| 5 | ratatui `Paragraph::wrap(Wrap {trim})` | cells | yes | 9 sites (`left_panel.rs:29,59`, `preview_pane.rs:61,79`, `worktree_menu.rs:670`, `value_modal.rs:226`, `messages.rs:219`, `right_panel.rs:148`, `detail_header.rs:140`) |
| 6 | `widgets/help.rs:564-660` `legend_entry`, `binding_lines` | cells (via #1) | hanging indent | domain-neutral (keys + description) |

### 6.4 Verdict

**Extract after refactor** as a `text` module: `truncate_end`, `truncate_middle`,
`truncate_hard`, `pad_to_width`, `fit` (truncate + pad), `fit_spans`,
`ellipsize` (merged into `truncate_end`), `word_wrap` (cell-based, one
implementation), `wrap_styled_line`, `spans_width`. Define the width-0/1
contract once. The `compact_*` helpers stay in Conspectus. Replace the
char-count variants (#5-11) and the CLI twins, and decide whether the CLI table
renderer may depend on the framework text module (open question 6).

## 7. Cursor stepping (`src/tui/cursor.rs`, 52 LOC, 2 tests)

- `pub(crate) fn wrap_step(index, len, delta: i32) -> usize` and
  `clamp_step(index, len, delta)`; pure, no deps.
- Callers: `wrap_step` 17 (picker, controls, search, mux_launch, pin_edit,
  launch_spec_form, keymap), `clamp_step` 2 (`app/tree.rs:161`,
  `app/explorer_nav.rs:116`).
- Deviation: `row_menu.rs:97-102`, `mux_menu.rs:86-91`, `worktree_menu.rs:383-387`
  hand-roll saturating `cursor - 1` / `(cursor + 1).min(last)` (clamp) while
  siblings (picker, controls, search) wrap around, with no stated reason.
- Scroll-window math is duplicated: `form.rs:724` `scroll_offset_for_cursor`
  and `controls.rs:1109` `scroll_offset_for_cursor` (same arithmetic, one takes
  `Option<usize>`), `multi_select.rs:295` `compute_scroll`, `picker.rs:~243`
  inline, `search.rs:1423` `compute_scroll`, plus the reducer's
  `adjust_left_scroll`/`adjust_explorer_scroll` (`app.rs:1280,1320`).
- **Verdict: extract as-is** (`ListCursor` helpers, `pub`), then add a single
  `scroll_to_keep_visible(cursor, viewport, total)` and make the three menus use
  `clamp_step`/`wrap_step`.

## 8. Captured-pane layout (`src/tui/preview_wrap.rs`, 289 LOC, 14 tests)

- `pub fn layout_capture(lines: &[Line<'_>], mode: PreviewWrap, pane_width:
  Option<u16>, width: u16) -> Vec<Line<'static>>` (`:54`). One production
  caller (`ui/preview_pane.rs:388`).
- Internals: private `Cell {text, width, style}`; drops trailing blank rows;
  `Plain` (word wrap), `Smart` (`smart_rows` `:89`: strip trailing padding,
  truncate when only decoration overflows, `squeeze_padding` `:123`, hanging
  indent), `None` (`hard_wrap` at the pane's width, then clip); tab stops every 8
  columns (`:246`); box-drawing and dash ranges treated as "decoration"
  (`:38-46`).
- Coupling: `use super::PreviewWrap` (config enum with `as_str`/`parse`/`label`
  for the controls overlay, `tui/mod.rs:297-340`). No Conspectus model imports.
  The tmux angle is only a doc comment and the meaning of `pane_width`.
- **Verdict: extract after refactor** into the framework `text` module as a
  styled-cell layout (`WrapMode` enum moves too; the controls label/parse stay
  in Conspectus). It is the only styled cell-accurate wrapper and could back
  wrap #1 and #2 above.

## 9. Snippet fitting (`src/tui/widgets/snippet.rs`, 198 LOC, 10 tests)

- `pub fn fit(text: &str, highlight: &Range<usize>, budget: usize) -> Fitted`
  (`:23`), `Fitted {text, highlight: Option<Range<usize>>}`.
- Behavior: strips Markdown backticks (`strip_ticks` `:44`, domain
  assumption about transcript text), crops around the match favoring text
  after it (two thirds of spare room), cuts on word boundaries, `…` on each cut
  side, handles a match wider than the budget.
- One production caller (`widgets/search.rs:1268`); the CLI has an independent
  near-copy (`cli/search.rs:1004` `crop_around`).
- **Verdict: extract as-is** after making backtick stripping a parameter;
  merge with the CLI copy if the CLI may depend on the crate.

## 10. Scrollbars and scroll indication

ADR 0076 chose `ratatui::widgets::Scrollbar` (VerticalRight, 1-row vertical
margin, hidden when content fits, default style). Code:

| Call site | File:line | Via shared helper? |
| --- | --- | --- |
| Left panel tree | `ui/left_panel.rs:136-141` | yes |
| Explorer header | `ui/right_panel.rs:145-153` | yes |
| Preview body (plain) | `ui/preview_pane.rs:59-71` | yes |
| Preview body (windowed transcript) | `ui/preview_pane.rs:77-83` | yes |
| Help page (and viewer/pane-viewer help) | `widgets/help.rs:266-287` | **no, a copy** |

Shared helpers: `scrollbar_layout(area, content_length) -> (Rect, Option<Rect>)`
(`left_panel.rs:152`, reserves a 1-column gutter, "fade-on-fit") and
`render_vertical_scrollbar(frame, area, content_length, position)`
(`left_panel.rs:191`, maps `content - viewport + 1` so the thumb reaches the
bottom; applies `Style::default().remove_modifier(REVERSED)` so the selection
highlight does not bleed in). The help copy re-derives the same
`max_scroll + 1` / `remove_modifier(REVERSED)` / `Margin{vertical:1}`.

ADR vs code deltas: the ADR says the bar is rendered over the pane inner area;
the code instead carves the gutter out of the content `Rect` first. The ADR says
no theme keys (still true: `theme.rs` has no scrollbar field).

Not every scrolled surface has a scrollbar. Other indicators in play:

| Surface | Indicator |
| --- | --- |
| Pane viewer (full screen) | `[ n/m ]` text in a footer (`pane_viewer.rs:511`) |
| Form text fields | `<` / `>` edge markers (`form.rs:101-135`) |
| Lists clipped to N items | `… N more` text (`preview/path.rs:122-124`, `git.rs:131`, `process.rs:37`) |
| Controls, picker, multi-select, search lists, messages, value modal, row/mux/worktree menus | none |

The `Paragraph::scroll` offset is a `u16` in some places (value modal, controls)
and a `usize` in others (pane viewer, help).

**Verdict: extract after refactor** as `ScrollView`/`Scrollable` helpers:
`scrollbar_layout` + `render_vertical_scrollbar` move to the framework; the
help copy and any new modal list use them; add `scrollbar` thumb/track tokens
when theming is wanted (ADR 0076 open item); unify the numeric position type
and replace `wrapped_line_count` with an accurate row counter.

## 11. Age and relative-time formatting

| Function | Location | Output | Callers |
| --- | --- | --- | --- |
| `format_recency(now: Option<i64>, then: Option<i64>) -> Option<String>` | `rows/mod.rs:999` | `Ns`, `Nm`, `Nh`, `Nd`; negative deltas clamp to `0s` | ~25 (rows/*, `ui.rs`, `explorer.rs`, `messages.rs`, `header.rs`, `forge.rs`, `value.rs`, `content.rs`) |
| `recency_bucket(now, then) -> Option<RecencyBucket>` | `rows/mod.rs:1130` | Fresh <5m, Active <1h, Recent <1d, Cold | via `age_span` (`left_panel.rs:761`) |
| `format_elapsed(seconds: u64)` | `ui/detail_header.rs:237` | the same s/m/h/d ladder, on a `Duration`; self-described "mirrors" `format_recency` | 1 |
| `age_label(secs: u64)` | `app/content.rs:295` | `just now`, `Ns ago`, `Nm ago`, `Nh ago` (no days) | 2 |
| `relative_epoch(epoch, now)` | `explorer.rs:1849` | `format_recency` + `" ago"`, `—` if unknown | 2 |
| `format_relative_age(then, now)` | `output/render.rs:1168` (CLI table) | `Ns/Nm/Nh/Nd/Nw`, `now` for future | CLI only |
| `format_duration_for_input` | `widgets/controls.rs:638` | inverse: duration to the compact text a user edits | 1 |

" ago" is appended ad hoc at 7 sites: `explorer.rs:1851`, `messages.rs:236`
(`{age:>4} ago`), `ui/header.rs:183` (`updated {label} ago`),
`ui/detail_header.rs:224`, `preview/forge.rs:107`, `preview/value.rs:74`,
`widgets/clean_up.rs:33`. `docs/tui-columns.md` states "Nd (days, with no larger
unit)" while the CLI renders weeks and the doc's own consistency helper accepts
`w`. The age *cell* (`age_span`, `left_panel.rs:761`) right-aligns to
`AGE_CELL_WIDTH = 4` (`:685`), uses `marks::NO_VALUE` for unknown, and colors by
bucket through `theme.recency_*`.

**Verdict: extract after refactor** as `age` (formatter + bucket enum + cell
helper): `format_recency` and `recency_bucket` are pure; they currently live in
`rows/mod.rs` only by accident. Collapse the three TUI copies, make "ago" a
caller option, and decide the day/week cap (open question 10).

## 12. Hints, footers, and key labels

- Source of truth for overlay footers: `keybindings/overlay.rs`
  (`KeyRow`, `Hint::{Shown, Alias}`, `footer(rows) -> String`, `:57-98`),
  joined with `" · "`, one segment per verb with labels joined by `/`.
- Footer *renderers* (ten different ones, five different styles):

| Renderer | Location | Style | Wrap |
| --- | --- | --- | --- |
| `form::wrap_footer` | `form.rs:520` (3 callers: pin_edit, mux_launch, new_mux) | raw `Modifier::DIM` | splits on `" · "` segments |
| search footer (`render_footer`) | `search.rs:1081-1100`, via `popup_frame::wrapped_lines`, capped at 4 rows | `theme.placeholder` | word wrap |
| controls footer | `controls.rs:792-796`, via `wrapped_lines` | raw `Modifier::DIM` | word wrap |
| row menu footer | `row_menu.rs:163-167`, via `wrapped_lines` | unstyled | word wrap |
| `help::push_footer` | `help.rs:553` (also `viewer_help.rs:98`, `pane_viewer.rs:600`) | `theme.placeholder` | word wrap |
| messages footer | `messages.rs:154-166` | `theme.placeholder` | none |
| value modal footer | `value_modal.rs:228-233` | `fg(theme.secondary_text)` | none |
| mux menu / worktree menu footer | `mux_menu.rs:136`, `worktree_menu.rs:580,605,635,649` | unstyled `Line::from` | none |
| pane viewer footer | `pane_viewer.rs:541-550` | `dim(theme)` = `secondary_text` + raw DIM; hand-written `(a) attach · (?) help · (q) close` | drops the scroll reminder when tight |
| status bar | `ui/header.rs:517-582` | `theme.placeholder`, priority-dropping segments | by priority |

  (The find-prompt hint strings at `viewer_bridge.rs:59` and `pane_viewer.rs:174`
  are generated from `VIEWER_FIND_KEYS` and shown inside the find prompt.)

- Key *labels* have three vocabularies: `keybindings::key_label` (`Up`, `PgUp`,
  `Ctrl-D`), `viewer_help::key_notation` (`:72`, converts to `↑ ↓ ← →`, `Space`,
  upper-cases `Ctrl-x`), and the literal labels in each overlay table
  (`↑`, `Space`, `PgUp`, `S-Tab`/`Shift-Tab` accepted by `label_names_key`
  `overlay.rs:114-141`). `help.rs` also types `"j / k / ↓ / ↑"` by hand
  (`:446`). `mislabeled()` (`overlay.rs:102`) checks table labels against
  bindings; hand-typed help strings and the pane-viewer chips are not checked.
- There is **no key-chip widget**. Keys appear as plain text in footers and as
  an accent-colored fixed 16-column key column in help (`help.rs:614-660`,
  `KEY_COLUMN_WIDTH = 16`, key padded to 14).
- **Verdict (layer 2 overlap):** `footer()`/`KeyRow` belong with the
  key-table work; the primitive in this slice is a single `HintBar` renderer
  (style = one token, wrap = one rule) that replaces the ten above, plus one
  `key_label` vocabulary.

## 13. Selection, cursor markers, and focus styling

- Theme tokens exist (`selection_active` REVERSED|BOLD, `selection_inactive`
  BOLD) but are read at exactly 3 places (`left_panel.rs:1089-1091`,
  `messages.rs:243`).
- Everything else hard-codes `Modifier::REVERSED` on the cursor row:
  `controls.rs:1137,1149,1161,1189`; `picker.rs:254`; `search.rs:692` (accent+BOLD+REVERSED
  choice pill), `925`, `1073`, `1222`, `1308`, `1385`, `1407`;
  `right_panel.rs:451,610,611,615,642,680,699`; `input.rs:159` (text cursor
  cell). Menus (`row_menu`, `mux_menu`, `worktree_menu`) use BOLD only.
- Row cursor marker has two vocabularies: ASCII `"> "` (10 sites in 5 files:
  `controls.rs:1132,1147,1159,1186`, `form.rs:60`, `picker.rs:259`,
  `search.rs:942,1067,1238,1314`) and `►` (`row_menu.rs:138`, `mux_menu.rs:123`,
  `worktree_menu.rs:552`; also the pane focus marker at `ui.rs:333`).
  `docs/tui-columns.md:103` lists `►` as "a menu cursor" only.
  `marks::POINTER` is defined and never used.
- Ghost look: `ghost_style` / `ghostify` (`left_panel.rs:886-907`): theme
  `ghost` color + ITALIC, BOLD removed; chips instead get `placeholder | ITALIC`
  so an agent still reads as an agent. Used by every left-panel row builder, not
  by right-pane rows or overlays.

**Verdict: extract after refactor** into `selection` helpers
(`cursor_marker(active) -> &str`, `selected_style(theme, focused)`), then apply
to all 26 REVERSED sites. Choose one marker (open question 3).

## 14. Rules, dividers, separators

- Horizontal rule: `"─".repeat(n)` hand-rolled at `preview_pane.rs:155`,
  `detail_header.rs:302,318`, `messages.rs:213`, `pane_viewer.rs:444`; only the
  detail header applies `theme.divider`. `chip_divider_line` (see 4.2) is the
  labeled form.
- Separators: `" · "` (19 literal sites, 139 total uses of `·`),
  `header.rs:87-89` locals, `" › "` breadcrumb (`explorer.rs:995`).
- Verdict: extract after refactor as `rule(width, label?, anchor)`.

## 15. Consistency tests in this slice

| File | Tests | What it asserts | Generic or domain |
| --- | --- | --- | --- |
| `consistency_tests/styling.rs` (156 LOC) | 4 | gone/unstarted entities wear `theme.ghost` + ITALIC, running ones do not; hollow pin glyph in ghost color; no pin row says "gone"; `p` has an inverse on the same key in every view | helper `label_wears_ghost_style` (find a label's first cell, check fg/modifier) is generic; assertions are domain |
| `consistency_tests/rendering.rs` (338 LOC) | 13 | one column grid across live/gone mux rows and pinned kinds; same chip look (bg or REVERSED) for the agent name in every view; unfocused pane shows no highlighted row (counts REVERSED cells per half); regrouping resets scroll; history empty state copy | helpers `tokens`, `row_grid`, `token_is_a_chip`, `reversed_cells`, `is_age` are generic buffer-inspection utilities; assertions are domain |
| `consistency_tests/text.rs` (376 LOC) | 8 | no rendered screen, composed hint, `--help` page, or `src/tui`/`src/cli` string literal contains an ADR/CSP number or `docs/*` dev path; no "slot" | `string_literals()` (a ~90-line Rust lexer that extracts non-comment, non-test literals) and `literal_offenders(dir, pred)` are generic lint infrastructure; `artifact_in`, `says_slot`, `USER_GUIDES` are Conspectus policy |
| `consistency_tests/docs.rs` (220 LOC) | 5 | glyph table, state-word table, view/row-kind headings, operator-language, doc linked from index | section/table parsing helpers (`section`, `first_cells`, `has_heading`) are generic; the `vocabulary()` set is domain |
| `icons_tests.rs` (338 LOC) | 20 | see 3.2 | vetting tests are generic; node-kind round trips are domain |
| `theme_tests.rs` (150 LOC) | 14 | default values (many pin ADR numbers), parser behavior, key table drift (one direction) | parser tests generic; default-value tests pin domain colors |
| `snippet_tests.rs`, `preview_wrap_tests.rs` | 10, 14 | fitting and wrap behavior | generic |

Gaps: no test renders any non-default theme; no test asserts a source file other
than `theme.rs` is free of `Color::` literals (20 lines currently violate the
intent, section 2); no direct tests for `centered_rect`, `ellipsize`,
`wrapped_lines`, `themed_popup`, `ui/text.rs` strict/pad/fit_spans, or the
duplicated scrollbar code.

`src/tui/snapshot.rs:393-470` converts a ratatui `Buffer` to ANSI text with its
own `Color` -> SGR table (19 arms), emitting only `add_modifier` bits (BOLD, DIM,
ITALIC, UNDERLINED, REVERSED); `sub_modifier` is ignored, so a
`remove_modifier(REVERSED)` scrollbar prints as the cell's added modifiers only.
That harness is another slice's concern but depends on the token set above.

## 16. External crates and wrapping

| Crate | Where | Wrapped or direct |
| --- | --- | --- |
| `ratatui 0.30` (`crossterm`, `macros`) | everywhere; `Color`, `Modifier`, `Style`, `Span`, `Line`, `Paragraph`, `Block`, `Scrollbar`, `Clear`; `span!`, `line!`, `vertical!`, `horizontal!` macros (177 `span!`, 47 `line!`) | direct. Theme tokens are ratatui types, no shim. |
| `tui-popup 0.7` | `popup_frame.rs` and 11 body wrappers | shim (`themed_popup`), but `KnownSizeWrapper` unused |
| `unicode-width` | `ui/text.rs`, `snippet.rs`, `icons.rs`, `popup_frame.rs`, `preview_wrap.rs`, `left_panel.rs` (14 files) | direct |
| `ratatui-cheese 0.7` | `multi_select.rs` (styles bridged by `cheese_styles_from_theme`, `:48-60`), `help.rs` (`Binding` as a data container only) | wrapped; theme bridge is the good pattern |
| `ratatui-comfy-toaster 0.4` | `widgets/toast.rs` | shim; border color hard-coded green upstream, `theme.success` not honored |
| `anstyle 1` | CLI table renderer only (ADR 0022) | not used by the TUI |
| `ansi-to-tui` | `ui.rs:27` for preview text | direct |

## 17. Duplications and inconsistencies (numbered, with counts)

1. **Typed theme tokens bypassed:** 20 raw `Color::` lines (17 in `form.rs`),
   21 raw `Modifier::DIM` where `placeholder` exists, about 20 raw
   `Modifier::REVERSED` cursor rows where `selection_active` exists. Forms and previews take no `Theme` at all.
2. **Four "muted" tokens plus raw DIM:** `placeholder` (51 uses), `secondary_text`
   (30), `ghost` (4), `divider` (4), `mux_unmuxed` (4); same visual default.
3. **Five `dim()` helper definitions** with three signatures (`preview/value.rs:33`,
   `forge.rs:67`, `git.rs:112`, `process.rs:132`, `pane_viewer.rs:413`), two
   `bold()` helpers, and ten inline `let dim = Style::default()...`.
4. **13 truncation/ellipsis implementations** (section 6.2); 6 measure chars, not
   cells; width-0/1 edge behavior differs between `ellipsize` and
   `truncate_to_width_strict`.
5. **6 word-wrap implementations** (section 6.3); `tui::widgets` imports the
   transcript viewer for one of them.
6. **3 identical span-width sums** (`spans_width`, `spans_cells`, `cells`) and a
   `div_ceil` row estimator that disagrees with real wrapping.
7. **Popup frames built 4 ways** (11 `themed_popup`, 4 hand-built, 2 unthemed
   fallbacks, 2 pane frames without border style); 11 identical `KnownSize`
   impls ignoring the upstream `KnownSizeWrapper`; one dead type (`LinesBody`).
8. **13 modal sizing rules**; the `min(cap, w-4).max(n)` text appears 10 times
   (three forms repeat it twice); 4 menus use `min(w)` margin 0, others -4/-2.
9. **Title padding and style:** embedded spaces vs widget-added vs span-built;
   plain (12) vs accent-bold (2: value modal, messages).
10. **Cursor row styling:** `"> "` (10 sites) vs `►` (4 sites); REVERSED
    selection (hard-coded 20 sites) vs BOLD-only menus vs `theme.selection_*` (3
    sites); menus clamp, lists wrap.
11. **PR-state to color mapping re-implemented at 9 sites in 6 files:** `left_panel.rs:910-917,
    1552-1561, 1582-1590`, `detail_header.rs:360-376`, `right_panel.rs:226,496`,
    `explorer.rs:962-968`, `search.rs:1397-1401`, `help.rs:667-672`. The
    `ForgePr` `Color::Reset` sentinel ("callers pick from `pr_*`") is
    special-cased in 5 files.
12. **Glyph constants vs literals:** `marks::FAIL/POINTER/BACK` have 0 users; `▲`
    13 literals, `►` 4, `√` 3, `×` 2, `—` ~65 vs 4 uses of `NO_VALUE`.
13. **Node-kind glyph span builders:** `Style::default().fg(color); "{} ", glyph`
    reimplemented at `left_panel.rs:919,1544,1564,1573,1594`, `right_panel.rs:230,500`,
    `help.rs:679`, `search.rs:1405`, `explorer.rs:1062`.
14. **Age formatting:** 4 TUI formatters + 1 CLI formatter with different unit
    ladders (`age_label` has no days, CLI has weeks); " ago" appended ad hoc at 7 sites.
15. **Footer rendering:** 10 renderers, 5 styles (raw DIM, `placeholder`,
    `secondary_text`, `secondary_text`+DIM, unstyled); the pane viewer's chips are hand-typed, not
    generated.
16. **Key labels:** 3 vocabularies (`Up` vs `↑`, `PgUp`, `S-Tab`/`Shift-Tab`,
    `Ctrl-D`) plus hand-typed help strings (`help.rs:446`).
17. **Chips:** 6 pill forms share no code; `badge_width` is honored by
    `badge.rs` but `search.rs:1160` hard-codes 10 (`CHIP_WIDTH`); chip dividers
    and `badge_span` measure chars, not cells.
18. **Scrollbar:** shared helper at 4 sites, copy at 1; at least 6 scrolled
    surfaces (controls, picker, multi-select, search, messages, value modal)
    have no indicator; scroll offset type differs (`u16` vs `usize`).
19. **Scroll-window math:** 5 copies of "keep cursor visible"
    (`form.rs:724`, `controls.rs:1109`, `multi_select.rs:295`, `search.rs:1423`,
    `picker.rs:243`).
20. **Checkbox vocabulary:** `[x]`/`[ ]` (`form.rs:565`, `controls.rs:1189`) vs
    `√`/`•` (`multi_select.rs`) vs `[x]`/`√`/`☑` accepted by a test
    (`rendering.rs:292`).
21. **Doc/code drift:** `popup_frame.rs:24` says the title is accent-colored (it
    is not); `ui.rs:11` describes `◉/◯` mux glyphs; `popup_frame.rs:3` says nine
    overlays (14 are drawn from `ui::draw`, plus the pane viewer); ADR 0076 says
    the focused-pane border is highlighted and that three panes get scrollbars
    (four call sites plus help; pane frames have no border style);
    `docs/operations.md:287` calls the focus marker `▸`; `[tui.theme.icons]`,
    `[tui.theme.harness]`, `node_*`, `edge_*` undocumented for users.
22. **Dead and stale:** `pin_placeholder` (kept for config compat), `LinesBody`,
    `marks::{FAIL,POINTER,BACK}`, `truncate_to_width` (alias of strict).

## 18. Framework verdict summary

| Candidate base control or module | Source | Verdict | Merges with |
| --- | --- | --- | --- |
| `Theme` / `Palette` (generic tokens, semantic style fns) | `theme.rs` | split: generic core + Conspectus domain palette adapter | the 4 muted tokens; selection tokens; chip tokens |
| `StyleSpec` + color/modifier grammar | `theme.rs:157-195, 596-725` | extract as-is | |
| Theme config loader (generic walk) | `config.rs:1348-1432` | split: generic table-walk to framework, domain keys to adapter | |
| `glyphs` (marks, vetting, risk checks, width tests, scanner) | `icons.rs`, `icons_tests.rs`, `scripts/glyph-widths.py` | split | literal glyph sites (section 3.4) |
| Node-kind/state glyphs, `IconOverrides` | `icons.rs` | stays in Conspectus (adapter registering into the vetting table) | |
| `Chip` / `Pill` | `badge.rs`, `chip_divider_line`, `choice_segments`, `harness_chips` | extract after refactor | |
| `harness_badge`, `command_badge` | `badge.rs` | stays in Conspectus | |
| `ModalFrame` (rect, themed popup, sized body, title, sizing) | `popup_frame.rs` + 4 hand-built frames + 11 `KnownSize` impls | extract after refactor | `form::modal_rect`, every `centered_modal_rect` |
| `text` (truncate/pad/fit/wrap/measure) | `ui/text.rs` (generic half), `ellipsize`, `form::fit/wrap_words`, `viewer::render::word_wrap` | extract after refactor | 13 truncation + 6 wrap implementations |
| `compact_path_*`, `compact_mux_*` | `ui/text.rs` | stays in Conspectus | |
| `ListCursor` (wrap/clamp step, keep-visible) | `cursor.rs` + 5 scroll copies | extract as-is, then merge | menus' hand-rolled clamp |
| `CaptureLayout` / styled-cell wrap | `preview_wrap.rs` | extract after refactor (move `WrapMode`) | `form::wrap_words`, viewer wrap |
| `SnippetFit` | `snippet.rs` | extract as-is (parameterize tick stripping) | `cli/search.rs::crop_around` |
| `Scrollbar` helpers | `left_panel.rs:152-216`, `help.rs:266-287` | extract after refactor | `wrapped_line_count` fix |
| `age` (format, bucket, cell) | `rows/mod.rs:999,1130`, `detail_header.rs:237`, `content.rs:295`, `explorer.rs:1849` | extract after refactor | CLI `format_relative_age` |
| `HintBar` / key label vocabulary | `form.rs:520`, `help.rs:553`, 7 other renderers, `overlay::footer` | extract after refactor (layer 2) | key-table work |
| `selection` helpers (cursor marker, row style, ghost) | 26 REVERSED sites, `ghostify` | extract after refactor | `theme.selection_*` |
| `rule` (horizontal divider) | 5 `"─".repeat` sites | extract after refactor | |
| Consistency-test kit (buffer inspection, literal scanner, doc-table parsing) | `consistency_tests/{rendering,styling,text,docs}.rs` | split: helpers to framework, assertions stay | |

## 19. Open questions for the operator

1. **Muted tokens.** Fold `placeholder`, `secondary_text`, `ghost`, `divider`,
   `mux_unmuxed` into two (`muted` and `inert`)? Making the 21 raw DIM sites use
   the theme changes nothing in the default palette but adds new override
   surface. Confirm.
2. **Form colors.** `form.rs` paints the active field `White on DarkGray` and the
   text cursor `Black on White`: fixed backgrounds that contradict the
   REVERSED-only rule used elsewhere (and read poorly on light terminals).
   Unify on REVERSED/theme tokens (visible change) or keep as a named
   "input surface" token?
3. **Cursor marker and behavior.** `"> "` (10 sites) or `►` (4 sites, and the
   doc)? Should menus wrap like the lists or clamp like they do today?
4. **Titles.** One title form (padding in the frame, accent+bold label, or
   plain)? Today 12 plain and 2 accent-bold.
5. **Pane frame focus.** Highlight the focused pane's border with the accent
   (ADR 0076 says it does; code does not), or keep focus on the title marker
   only?
6. **Cell vs char width.** Standardize all truncation on terminal cells (CJK and
   emoji correct) and make `badge` and `form::fit` follow? Should the CLI table
   renderer share the framework text module?
7. **Presets and light/dark.** Does the first framework cut ship palette presets
   (CSP-269) or stay REVERSED-only with no detection? Is a non-default theme
   required in the test harness?
8. **Glyph policy.** Keep the operator-preference exceptions (`▶`, Braille
   spinner) and the mandatory "every glyph vetted" scanner as framework
   behavior? Add an ASCII fallback mode (none exists today)?
9. **Toast.** The upstream toast ignores `theme.success`; accept, fork, or
   replace for the framework?
10. **Age units.** One ladder for TUI and CLI (`s/m/h/d`, with or without `w`)?
    Is " ago" a caller concern? Keep `just now`?
11. **Key labels.** One vocabulary (`Up`/`↑`, `S-Tab`/`Shift-Tab`) and generated
    pane-viewer chips?
12. **Typed vs dynamic tokens.** Does the framework theme keep typed fields
    (compile-time safety, hand-maintained key table) or become a map of named
    `StyleSpec`s (adopters extend freely, config loader is trivial, harness
    colors already work this way)?
13. **Aider removal (m-24).** The default `harness_colors` and the
    `harness_aider` alias disappear with it; confirm that milestone lands before
    or independently of the extraction so the adapter is written once.
