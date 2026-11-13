# Proposed Control Set And House Rules

Phase 3 of the [TUI framework plan](README.md). It dedupes the
[inventory](inventory.md) into a smaller base set, states the design-language
rules that make the set consistent, and lists the decisions the operator needs
to confirm before any backlog entry is created.

Status: **confirmed by the operator** (control set accepted as proposed; all 18
house rules accepted; separate cleanup stories first; crate named `tuikit` with
the three shims replaced). The decisions are recorded in
[ADR 0150](../../adr/0150-tui-framework-workspace-crate.md),
[ADR 0151](../../adr/0151-tui-framework-control-set-and-house-rules.md), and
[ADR 0152](../../adr/0152-tui-framework-dependency-ownership.md). Scope is
fixed: workspace crate; layers 1 and 2; internal reuse only; consistency-first.

## 1. Result In One Table

| | Today | Proposed |
| --- | --- | --- |
| Menu, dialog, prompt, list, reader, and toast surfaces | 18 + 11 text-entry + 7 scrolling readers + 8 tree views | **16 interactive controls** |
| Chrome pieces | 16 parts of `ui.rs`, `header.rs`, `right_panel.rs`, `left_panel.rs` | **5 chrome controls** |
| Design-token and primitive modules | 13 truncators, 6 wrappers, 13 sizing rules, 4 age formatters, 4 chip forms... | **8 foundation modules** |
| Interaction contracts | key tables, `Overlay`/`Modal`, help generation, clipboard, message log, snapshot and consistency harness | **6 layer-2 modules** |

Everything else (row builders, harness colors, transcript parsing, preview
sources, graph-querying menus, the global keymap's actions) stays in
Conspectus as adapters over these controls.

## 2. Architecture Of The Framework

```
framework crate (working dir: crates/<name>/)
├─ foundations   theme · glyphs · text · scroll · chip · age · frame · selection
├─ controls      text entry · choice & command · reading · data display · chrome · feedback
├─ conventions   keys · overlay · help · focus · clipboard · feedback · testkit
└─ (feature) richtext   markdown · tables · syntax    ← pulls tui-markdown, syntect, comfy-table

conspectus (the app)
└─ adapters      row builders · palette & theme registrations · Msg mapping · domain copy
```

Dependency direction is one-way: conspectus depends on the framework; the
framework depends on `ratatui`, `crossterm`, `unicode-width`, `tui-input`, and
the optional rich-text crates, and on nothing from conspectus. Each control is
generic over the host's message type `M` (commit value) and, where it needs
host data, a context type `Cx`; it takes `&Theme` and never reaches for global
state.

## 3. Foundations

| Module | Contents | Absorbs |
| --- | --- | --- |
| `theme` | generic role tokens (surface, accent, muted, inert, selection, warning, error, success, link, match highlight, badge); an app-registered named-style map for domain roles; the `StyleSpec` grammar; the config table walk | 19 generic of 41 `Theme` fields; the 4 muted tokens, 21 raw DIM sites, 5 local `dim()` helpers; 20 raw `Color::` literals; the generic half of the `[tui.theme]` loader |
| `glyphs` | vetted-glyph table, `marks` (pointer, disclosure, ok, fail, back, ellipsis, no-value), cell-width checks, the "every glyph is vetted" scanner | `icons.rs` generic half; about 25 duplicated glyph literals; `scripts/glyph-widths.py` |
| `text` | cell-width measure, end and middle truncation, pad, fit-spans, word and char wrap, snippet fit, match painting, `find_ci` | 13 truncators, 6 wrappers, 2 snippet croppers, 7 match painters |
| `scroll` | wrap and clamp stepping, follow-cursor offset, scroll window, scrollbar gutter, wrapped-row count | `cursor.rs`; 5 scroll-window copies; 5 scrollbar sites; unclamped offsets |
| `chip` | fixed-width `Chip`, toggle chip, segmented title, position chip, status chip | `badge.rs`; 5 other chip forms |
| `age` | relative-time format and recency tint with injectable thresholds | 4 TUI formatters plus the CLI's |
| `frame` | `ModalFrame` (sizing policy, themed popup, title style), `PaneFrame` with focus marker, `SplitPane`, `rule` | `popup_frame.rs` and 4 hand-built frames; 13 sizing rules; 11 `KnownSize` impls; 5 `"─".repeat` sites |
| `selection` | list cursor marker, selected, active/inactive, ghost, and pending row styles | 26 `REVERSED` sites; 4 highlight looks |

## 4. Controls

Waves refer to the milestone sketch in section 9.

### Text entry

| Control | Absorbs | Stays in Conspectus | Wave |
| --- | --- | --- | --- |
| `LineEdit` + `TextField` | `TextInputState` (`tui-input`), viewer `FindPrompt`, the form text window, search and picker query lines | nothing | 3 |
| `TextPrompt<M>` | rename (session, mux, pin), worktree branch prompt, Controls max-age and contains editors | title strings, `parse_max_age`, which `Msg` a confirm maps to | 3 |
| `PathField` | `path_omnibox.rs` plus `form::path_row/path_mark` and the hint | the known-candidate lists | 3 |
| `Form<M>` | mux launch, new mux, pin edit; field kinds Text, Path, Choice, Toggle, CheckGroup, OneOf, ReadOnly, plus an `Override` decorator for Ctrl-R | field lists, request builders, `LaunchDefaults` preview, layer words | 3 |

### Choice and command

| Control | Absorbs | Stays in Conspectus | Wave |
| --- | --- | --- | --- |
| `ActionMenu<M>` | row menu, mux menu, worktree list, pin-unpin menu | `row_menu_for`, entry policy | 4 |
| `Dialog<M>` | worktree confirm x3, close-down choice, armed Forget (where a body helps), Clean-up empty notice | all wording and the `Msg::Commit*` mapping | 4 |
| `Checklist` | multi-select (the `ratatui-cheese` shim goes), Clean up | candidate labels, record mapping | 4 |
| `Palette<R>` | Picker, quick select (`/`), content search shell, and the backlog's command palette (CSP-392) as a third instance | ranking, entity index, outcome words, debounce providers | 4 |
| `SettingsPanel<M>` | Controls overlay: sections, 5 row kinds, sub-editors | the 24 rows, `ControlsContext`, filters | 4 |

### Reading

| Control | Absorbs | Stays in Conspectus | Wave |
| --- | --- | --- | --- |
| `Pager` (+ `PagerContent`, modes Screen / Modal / Embedded, and `Find`) | transcript viewer chrome, pane scrollback viewer, value modal, help page body, Messages detail, preview-pane scroll region | transcript model and parsers, gutter chips, `a` attach, preview sources and loaders | 5 |
| `HelpPage` | main help, viewer help, pane help (3 generators become 1) | the 48 hand-written prose rows and legends, as annotations | 2 |
| `LogList` | Messages list-plus-detail | `LogTarget`, `CommandRecord` | 4 |
| `richtext` (feature) | ANSI captured-pane layout, markdown, tables, syntax | which content uses it | 5 |

### Data display

| Control | Absorbs | Stays in Conspectus | Wave |
| --- | --- | --- | --- |
| `TreeView<R>` + `TreeState<Id>` + `ColumnSchema` | left-pane navigation state (five loose `App` fields), the columnar renderer (7 `render_*_spans` plus inline code), `GroupAlign`/`PrAlign`/`MixedAlign`, group header, row prefix, status glyph, age cell, row states, scrollbar | all row builders, `RowKind`, `RowId`, vocabulary tables, empty-state copy | 6 |
| `PropertyGrid` (`PropertyRow`, `LinkRow`, `DisclosureRow`) | right-pane node fields, related rows, the Other header, copy and inspect | `FieldValue`, link semantics, provider chips | 6 |
| `Explorer` (`ZonedExplorer`, `Breadcrumb`, `StackedMasterDetail`) | cursor over zones with stable row keys, drill stack, armed back, stacked layout | the node builders (about 48% of `explorer.rs`), `impl App` wiring | 6, staged last |

### Chrome and feedback

| Control | Absorbs | Stays in Conspectus | Wave |
| --- | --- | --- | --- |
| `AppFrame` | `ui::draw` shell: header row, body, footer row, overlay layer, toast layer | header content, which view fills the body | 7 |
| `TabBar` | left title strip, with elision | view list | 7 |
| `StatusBar` (`PriorityLine`, `KeyHintBar`) | status and hint line with priority-drop fitting; segments generated from key tables | segment producers | 7 |
| `EmptyState` | 12 hand-written empty-state messages | the copy | 7 |
| `Toasts` | `toast.rs` plus severity and theme-honoring border | call sites | 2 |

## 5. Interaction Conventions (Layer 2)

| Module | Provides | Absorbs |
| --- | --- | --- |
| `keys` | `KeyMatcher`, `KeyRow<A>`, `Hint`, `resolve`, `footer`, `key_label`, `mislabeled`; standard tables for menu, dialog, pager, text input, checklist, and form; a `KeyRole` classifier replacing `Debug`-name matching; a scoped global keymap (late) whose rows carry help text that actually renders | `keybindings/overlay.rs` generic half; the global keymap's shape; `MODAL_CONVENTION` |
| `overlay` | `Overlay<M, Cx>` with `handle` and `render`, `Outcome<M>`, `ModalStack` (host-owned, draws every entry bottom to top) | `modal.rs`, the 16 runtime handlers and `Action::*Key` wrappers, the 14-call draw ladder |
| `help` | key-table-to-help generator and page renderer | 3 generator copies |
| `focus` | declared focus rules: pane focus cue, modal focus lock, Tab cycle | `remap_for_focus` conventions (the `Focus` enum stays in the app) |
| `clipboard` | OSC 52 write to a host-supplied `Write` | `clipboard.rs` |
| `feedback` | one `Feedback { severity, text, ttl }` type feeding three channels (status line, toast, message log); bounded repeat-collapsing log | `tui/messages.rs` generic half; toast; status-line colouring |
| `testkit` | buffer inspection, key DSL, ANSI-preserving one-frame driver, overlay and key conformance harness with `assert_conventions(&Registry)`, source-scan lint helpers, doc-table sync | `snapshot.rs` (DSL and emitter), about 30 consistency tests lifted as helpers, hygiene-guard mechanisms. The existing `examples/pantry.rs` preview moves with the controls; no new gallery or coverage rule (internal-reuse scope) |

## 6. What Stays In Conspectus

Row builders and `RowKind`/`RowId`; harness colors and the `harness` theme
table; state-word and glyph meanings; mux, PR, pin, history, and worktree
semantics; the transcript model and parsers; preview sources and loaders; the
entity index and ranking; `Msg`, `Action`, `App`, the effect and undo systems
(layer 3); the header composition; process, socket, and write paths (the
framework spawns nothing). Conspectus-side adapters implement the framework's
traits for these.

## 7. Removed Before Or During Extraction

- `detail.rs` and `detail_header.rs` (the superseded sectioned detail model,
  about 1,800 LOC plus tests), after retargeting its two live reads.
- `LaunchSpecFormState` and ADR 0097's premise, when mux launch moves to `Form`.
- The hidden Union and Forks views, or at least their ignored groupings and the
  untested Forks builder.
- Dead surface: the omnibox's `selected`/`MAX_CANDIDATES`/`source`/`exists`,
  `tagged_input_row`, `warning_row`, `LinesBody`, `GroupSummary.attached` and
  `.unmuxed`, `RowTree.view`, the unused `marks::*`.
- Stale claims in `modal.rs` and ADR 0076.

## 8. House Rules (The Design Language)

These are the rules each extraction story applies. Each row names the
divergence in the inventory, the recommended rule, and what the operator would
see. Rows 1–12 are visible changes; rows 13–18 are internal or low-visibility.
**Confirm, amend, or strike any row.**

| # | Topic | Today | Recommended rule | Visible effect |
| --- | --- | --- | --- | --- |
| 1 | List cursor and highlight | 4 looks: `►`+BOLD, `> `+REVERSED, accent-only, `selection_active` | Full-row highlight from the theme's `selection_active` (`selection_inactive` when unfocused) plus the `marks::POINTER` marker in a 2-cell gutter, in every list | menus, Controls, Picker, Search, and forms change look; one look everywhere; the marker keeps selection visible under `NO_COLOR` |
| 2 | List ends | menus clamp; lists wrap; tree and explorer clamp | Short choice lists (menus, pickers, checklists, palettes) wrap; scrollable content (tree, properties, pagers, logs) clamps | menus start wrapping |
| 3 | Confirmation | inline arm, y/n sub-screens, m/d choice, none | Two sanctioned idioms: an inline arm on a menu entry when there is nothing more to say (`Enter` again), and a `Dialog` (`y`/`Enter` confirms, `n`/`Esc` cancels) when a body helps; reversible verbs (Unpin, Rebuild) need neither | worktree prompts keep their screens with uniform keys |
| 4 | Close keys | `Esc`, `q`, `Ctrl-C` mean different things per overlay | `Esc` steps back one level; `Ctrl-C` and `q` dismiss the overlay (`q` only where nothing is typed) | worktree menu's `Ctrl-C` now closes instead of going back |
| 5 | Close feedback | 6 overlays say "cancelled", 8 stay silent | Silent: cancelling is not an event | 6 status messages disappear |
| 6 | Enter and Tab in forms | new mux advances; mux launch and pin edit submit from any row; mux launch has a Tab-only socket stop | `Enter` submits from any row; `Tab`/`Down` next and `S-Tab`/`Up` previous over one ring | new mux no longer advances on Enter; mux launch's socket checkbox joins the ring |
| 7 | Validation errors | 3 displays plus 4 silent rejections | A single error line in the modal's footer slot, set by a failed confirm and cleared by the next edit; no silent rejection | max-age, branch, new mux, and empty pin rename now explain themselves |
| 8 | Footers and key hints | 5 of 9 families render one; hand-written in the status bar and two viewers | Every modal footer and the status-bar hints are generated from key tables; one grammar: short imperative verb, no "to" | wording unifies; hand-tuned status strings are regenerated |
| 9 | Focus cue | title marker only; modal borders use the accent | Tint the focused pane's border and title marker with `panel_focus_accent` (what ADR 0076 already claims) | the focused pane gains a coloured border |
| 10 | Unfocused cursor | tree shows an inactive pair; the explorer shows nothing | Every list shows the active or inactive pair | the right pane gains a visible unfocused cursor |
| 11 | Feedback severity | toasts always green; the status line paints everything in the warning colour | `Feedback` has info, success, warning, and error; each channel styles from the theme; errors stay longer | failures stop looking like successes |
| 12 | Paging and scroll | fixed 8-line pages in help, value modal, and messages; terminal-height-minus-2 in the tree; unclamped offsets | Viewport-sized `PgUp`/`PgDn`, `Home`/`End`/`g`/`G` on every list taller than a screen, half-page `Ctrl-D`/`Ctrl-U` in pagers, offsets clamped | paging distances change; blank-tail scroll bugs go away |
| 13 | Theme use | forms ignore it; 21 raw DIM and 26 raw REVERSED | Every control takes `&Theme` and no raw `Color::`; the five muted tokens collapse to `muted` and `inert`, old keys kept as config aliases | forms honour `[tui.theme]`; default look nearly unchanged |
| 14 | Truncation | 6 truncators count `char`s | All truncation and padding measure terminal cells; `…` ends, middle ellipsis for paths and ids | wide characters stop misaligning |
| 15 | Text editing | no bracketed paste; Alt-word motion inert; not width-aware | Cell-width-correct editing, bracketed paste on (multi-line paste cannot submit), Alt/Ctrl word motion working | multi-line paste behaves |
| 16 | Path fields | files suggested for a `cwd`; `~` not expanded on commit; per-frame I/O | A `PathKind` (dir or file); `~` expanded on commit; completion cached behind a `PathSource` | a file no longer validates as a `cwd` |
| 17 | Labels | `mux name`/`name`, `launch argv`/`argv`, `branch`/`worktree_branch` | One vocabulary: the shorter form | labels shorten |
| 18 | Pager details | Esc/q differ; find lands differently; position shown by chip, scrollbar, or nothing | `Esc` steps back (help, then find, then leave), `q` leaves; confirming a find always scrolls to the match; a `[ n/m ]` chip in Screen mode, a scrollbar in Modal and Embedded | pane viewer and transcript viewer behave alike |

## 9. Structural Decisions

| Decision | Recommendation | Why |
| --- | --- | --- |
| Crate name and path | a neutral working name, `crates/<name>/`, `publish = false` | internal-only (confirmed); the name must not carry the app's identity so extraction is a directory lift. **Operator to name it.** |
| Theme model | typed generic tokens plus an app-registered named-style map | compile-time safety for the framework's own controls; domain roles (harness colors, PR states) are registered, not added to the framework |
| Tree cell content | land `TreeView` with pre-rendered lines from the app first, then migrate each view to a `ColumnSchema` one at a time | smallest first extraction; the schema is the end state that removes the three alignment structs and about 40 literal gutters |
| Overlay context | host-owned `ModalStack`, a host-chosen `Cx` passed to `handle` and `render` | removes the 11-field context clone in Controls and Search |
| Stack drawing | draw every entry bottom to top | what ADR 0085 promised; Help over Controls then shows Controls underneath |
| Global keymap | scoped keymap is a late, separate story; the first cut ships overlay tables and generated hints | the current global table is wired into `Action`, `Msg`, and `View` |
| Header line | app composition over the framework's `PriorityLine` primitive | its content is domain |
| Explorer | extract last, behind a design story | about 13% of the file is domain-free and it is `impl App`; high risk, lower payoff than the others |
| Third-party crates | keep `tui-input`; keep `ansi-to-tui`, `tui-markdown`, `syntect`, `comfy-table` behind a `richtext` feature; replace the `ratatui-cheese` checklist shim, the `ratatui-comfy-toaster` toast, and `tui-popup` with in-tree code | each shim is where a theme bypass or a workaround lives (green-only toast border, mirror state and hand-written `Clone`, double rect computation); removing three dependencies |
| Test kit | extract helpers and the conformance harness; keep domain assertions in Conspectus | about 30 of 204 consistency tests lift directly, about 55 become helper-shaped |
| Multi-line input | out of the first cut; gap-analysis item | `tui-textarea` 0.7.0 declares `ratatui ^0.29` and will not resolve against 0.30; the maintained fork is `ratatui-textarea` |

## 10. Proposed Milestone Shape

A first sketch for sizing. The created stories (57, in milestone m-25) are
listed with IDs and dependencies at the end of [README.md](README.md); they
differ from this sketch in three ways: the cleanups and defect fixes are seven
separate stories, `Palette` is two stories (the control, then search on it),
and the column schema is three. Each extraction story carries a **TUI
changes** section listing regressions and consistency wins, and checks them
with `conspectus tui --snapshot`.

0. **Decisions and prep**: ADRs (boundary, control set and house rules,
   dependency ownership); delete the legacy detail model; retire or fix Union
   and Forks; triage the 14 suspected defects (reproduce, then fix in separate
   stories ahead of the extractions that would otherwise paper over them).
1. **Workspace and guardrails**: convert the repo to a workspace (CI,
   justfile, flake banner, `CLAUDE.md` invariants gain `--workspace`); the
   independence guard (dependency allowlist, forbidden-import scan, no process
   spawning, no domain vocabulary); extend the five source-scanning tests to
   the new crate with a "scanned at least N files" tripwire; framework docs
   skeleton.
2. **Foundations**: `theme`, `glyphs`, `text`, `scroll`, `chip`, `age`,
   `frame` + `selection`, and `Toasts` with `feedback`.
3. **Conventions and text entry**: `keys`, `overlay`, `help`, `clipboard`,
   `testkit`; then `LineEdit`, `TextPrompt`, `PathField`, `Form` and the three
   form collapses.
4. **Choice and command**: `ActionMenu`, `Dialog`, `Checklist`, `Palette`,
   `SettingsPanel`, `LogList`.
5. **Reading**: `Pager` (pane viewer first as the smallest), the transcript
   viewer on `Pager`, `richtext` and the preview region.
6. **Data display**: `TreeState` + `TreeView` (pre-rendered), `ColumnSchema`
   per view, `PropertyGrid`, `Explorer`.
7. **Chrome**: `AppFrame`, `PaneFrame` focus cue, `TabBar`, `StatusBar`
   generated hints, `EmptyState`, the scoped keymap.
8. **Separation readiness**: complete framework docs, the separation rehearsal
   (build and test the crate alone from a scratch export), framework-scoped
   decision records, Conspectus docs and agent instructions updated.
9. **Close-out**: optional stretch evaluation of layer 3 (the app runtime),
   then the backlog impact assessment over every open story.

## 11. Known Overlaps With Open Backlog

The closing assessment story reviews all 124 open stories. Overlaps already
visible:

| Story | Overlap |
| --- | --- |
| CSP-342 | lifts `src/viewer/` into a workspace member crate; the workspace conversion and the `Pager` split change its shape |
| CSP-392 / 390 / 393 / 394 | command palette and minibuffer; the palette becomes a `Palette<R>` instance, the minibuffer a `TextPrompt` host |
| CSP-435 | `tui-tree-widget` for the explorer; the explorer is a flat list with one collapsible header, so it did not fit and this likely closes |
| CSP-430 | `tui-textarea`; the crate no longer resolves against ratatui 0.30 |
| CSP-432 | `throbber-widgets-tui` spinners; a framework `Spinner` may replace it |
| CSP-455 / 434 | omnibox candidate list and browse mode; they target `PathField` |
| CSP-269 / 270 | theme presets and density modes; they sit on the framework `theme` |
| CSP-319 / 190 / 180 / 183 | responsive layout, empty/loading/error matrix, buffer-snapshot coverage; they share the narrow-width contract and `testkit` |
| CSP-188 / 191 | label compression, density modes; they touch the `text` and `TreeView` seams |
| CSP-702 | remember expanded groups across restarts; it touches `TreeState` |
| m-24 (CSP-699–701) | aider removal changes the default `harness_colors` and the `harness_aider` alias; landing it first means the theme adapter is written once |

## 12. Decisions (All Confirmed)

1. The control set in sections 3–5, and what stays out (section 6): accepted
   as proposed, including the Explorer as the last, design-first extraction.
2. The 18 house rules in section 8: all accepted.
3. Pre-extraction cleanups (section 7): separate stories first (delete the
   legacy detail model, retire Union and Forks, reproduce-then-fix the
   suspected defects).
4. Crate name `tuikit` (working name, `publish = false`) and replacement of
   the `ratatui-cheese` checklist shim, `ratatui-comfy-toaster`, and
   `tui-popup` with in-tree code.
