# TUI Inventory: Widgets, Composites, And Their Variations

Phase 2 of the [TUI framework plan](README.md). It consolidates seven
per-area evidence files in [`inventory/`](inventory/) into one census. Each
claim here cites a section of those files, which in turn cite `file:line`
at commit `ca668a3`.

**Method and confidence.** The notes come from reading source, not from
running the TUI. Anything that reads like a defect is marked *(code-read)*
and must be reproduced before it is fixed or promised. Counts come from
`grep` over `src/tui` excluding test files.

Notation: **G** = generic enough to be a framework control. **A** = generic
core plus a Conspectus adapter. **D** = stays domain. Slice files:
[01 tokens](inventory/01-design-tokens.md),
[02 left-pane tree](inventory/02-left-pane-tree.md),
[03 text entry and forms](inventory/03-text-entry-and-forms.md),
[04 menus, pickers, dialogs](inventory/04-menus-pickers-dialogs.md),
[05 search, help, viewers](inventory/05-search-help-viewers.md),
[06 interaction conventions](inventory/06-interaction-conventions.md),
[07 chrome and explorer](inventory/07-chrome-and-explorer.md).

## 1. Headline Numbers

| Measure | Count |
| --- | --- |
| Census rows in section 2 (surfaces, shared primitives, and contracts) | 64, drawn from 7 slice files that each list their own overlapping surfaces |
| TUI source | about 86k lines including tests; `widgets/` holds 25 modules |
| `Theme` fields | 41: 19 generic tokens, 22 Conspectus-domain (harness, mux, PR, node, edge, cwd, icons) |
| Raw `Color::` literals bypassing the theme | 20 (17 in `form.rs`) |
| Raw `Modifier::DIM` / `REVERSED` literals | 21 / 26 (about 20 are cursor rows); `theme.selection_active` is read in 3 places |
| Truncate / ellipsize / pad implementations | 13 (6 count `char`s, not cells) |
| Word-wrap implementations | 6 |
| Modal-sizing rules | 13 distinct (5 `centered_modal_rect` functions); 4 popups bypass the shared frame |
| Selection-highlight implementations | at least 6, with 4 different looks |
| Age/elapsed formatters | 4 in the TUI plus 1 in the CLI |
| Text-edit engines / renderers of one editing state | 2 / 4 |
| Scrolling reading surfaces that are one shape | 7 |
| Overlay key handlers copy-pasted in the runtime | 16 (14 near-identical), plus 16 `Action::*Key` wrappers |
| Production sites touched to add one modal (the Picker, newest) | about 9 |
| Consistency tests | 204; about 30 lift directly into a test kit, about 55 are domain instances of generic rules, about 115 are pure domain |
| Dead or superseded code found | about 1,800 LOC in `detail.rs` and `detail_header.rs`, plus smaller items (section 8) |

## 2. Census

### 2.1 Design tokens and primitives (slice 01)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Theme and palette | `theme.rs` 729 | 41 typed fields; 4 "muted" tokens plus 21 raw DIM sites plus 5 local `dim()` helpers; no presets, no light/dark | A: generic role tokens in the framework; domain palette registered by the app |
| Style grammar and config loader | `theme.rs` `StyleSpec`, `config.rs:1348-1432` | 43 keys plus `icons`, `harness`, `badge_width`; three hand-synced key lists | A: generic table walk moves; domain keys stay |
| Glyph slate | `icons.rs` 514 | vetted-glyph table, width tests, scanner; about 25 literals bypass `marks`; `marks::FAIL/POINTER/BACK` unused | A |
| Chip / pill | `badge.rs` 149 and five other chip forms | harness badge, command badge, plain `no agent`, dim `N hits`, `was in` | G (`Chip`) with domain constructors in the app |
| Popup frame and sizing | `popup_frame.rs` 127 | 13 sizing rules; 4 hand-built frames; 11 identical `KnownSize` impls | G (`ModalFrame`) |
| Text measuring, truncating, wrapping | `ui/text.rs`, `form::fit`, `popup_frame::ellipsize`, viewer `word_wrap`, others | 13 truncators, 6 wrappers; `tui::widgets` imports `crate::viewer::render::word_wrap` (layering inversion) | G (one `text` module) |
| Cursor stepping | `cursor.rs` 52 | `wrap_step` in lists, `clamp_step` in the tree, explorer, and menus | G |
| Captured-pane layout | `preview_wrap.rs` 289 | `PreviewWrap` modes | G after decoupling |
| Snippet fitting | `snippet.rs` 198 | one cropper plus a near-copy in `search.rs:1268` plus 7 match painters | G |
| Scrollbar and gutter | `left_panel.rs:152-216`, `help.rs:262-286` | 5 call sites, one a copy | G |
| Age formatting | `rows/mod.rs:999,1130` and 4 more | thresholds hard-coded | G with injectable thresholds |
| Selection and cursor-marker styling | 26 `REVERSED` sites | `> ` at 10 sites, `►` at 4 | G (`selection`) |

### 2.2 Text entry and forms (slice 03)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Single-line editor | `input.rs` 204 (wraps `tui-input`), `viewer/find.rs` `FindPrompt` | 2 engines (the second hand-rolled), 3 windowing algorithms, only the viewer's is cell-width aware | G (`LineEdit`) |
| Text prompt dialog | `input.rs`, `RenameOverlayState`, branch prompt, Controls max-age and contains | 5 users; empty input is a silent no-op in 3 of them and a toast in 1 | A (`TextPrompt<M>` with `on_confirm`) |
| Inline text field / query line | `form::window`, search and picker query lines | search and picker query lines draw no caret | G (`TextField`) |
| Path field with Tab completion | `path_omnibox.rs` 333 | see section 3 | A (`PathField`) |
| Form row kit | `form.rs` 741 | text, path, harness chips, option checkboxes, one-of-N, read-only note and derived rows | G with theme roles |
| Mux launch form | `mux_launch.rs` 717 over `launch_spec_form.rs` 457 | 11 stops; Enter submits from any row; Tab-only socket stop | A (instance of `Form`) |
| New mux form | `new_mux.rs` 215 | Enter on name advances, Enter on cwd commits | instance of `Form` |
| Pin edit form | `pin_edit.rs` 850 | per-pin-kind rows; Ctrl-R drops an override; pinned footer | A (`Form` plus `OverrideRow`) |

### 2.3 Menus, pickers, dialogs, toasts (slice 04)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Row action menu | `row_menu.rs` 188 | inline "press Enter again" confirm on one entry; cannot scroll | A (`ActionMenu<M>`) |
| Mux action menu | `mux_menu.rs` 199 | two fixed verbs, its own heading and width | folds into `ActionMenu` as data |
| Worktree menu | `worktree_menu.rs` 677 | list, text prompt, 3 y/n confirms, a two-verb choice; about 215 lines query the graph | A |
| Picker | `picker.rs` 277 | type-to-filter; wraps at the ends | G (`FilterPicker<T>`), merge with quick select |
| Multi-select and Clean up | `multi_select.rs` 322 (shim over `ratatui-cheese`), `clean_up.rs` 144 | the shim keeps a per-frame mirror state and a hand-written `Clone` | G (`Checklist`) |
| Controls overlay | `controls.rs` 1344 | 24 rows, sections, 5 row kinds, 3 sub-editor types; about 25–30% is generic | A (`SettingsPanel<M>`) |
| Value modal | `value_modal.rs` 239 | read-only text with a copy payload | the `Pager` modal mode |
| Messages overlay | `messages.rs` 251 + `tui/messages.rs` 281 | log list plus a detail pane | A (`LogList`); the log model is generic with a domain target |
| Toast | `toast.rs` 175 (shim over `ratatui-comfy-toaster`) | border hard-coded green; ignores `theme.success`; no severity | G (`Toasts`) |
| Confirm idioms | row menu, worktree sub-screens | 4 idioms: inline arm, y/n, m/d choice, none for Unpin and Rebuild | G (`Dialog`) |

### 2.4 Reading and search surfaces (slice 05)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Transcript viewer | `src/viewer/*` (about 5.9k), `viewer_bridge.rs` | gutter chips, toggles, hidden-match count | A (`Pager` plus transcript adapter) |
| Pane scrollback viewer | `pane_viewer.rs` 616 | shares 14 of 15–18 messages with the transcript viewer; clips long lines; `a` attaches | A (about 60 lines of adapter) |
| Keymap help, viewer help, pane help | `help.rs` 891, `viewer_help.rs` 128, inside `pane_viewer.rs` | only the viewer and pane pages are generated from key tables; the main help has 48 hand-written rows | A (`HelpPage` plus generator; prose stays) |
| Preview pane | `preview_pane.rs` 470, `preview/*` about 2.0k | `Top`, `Bottom`, `Match` anchoring; ANSI and wrap | A |
| Quick select and content search | `widgets/search.rs` 1523 | 3 separate hit-column grids; no reuse of the left-pane row renderer; debounce state machine | A (`SearchPalette`) |
| Rich text (markdown and tables) | `viewer/render.rs`, `table.rs` | wraps `tui-markdown`, `comfy-table`, `syntect` | G behind a cargo feature |

### 2.5 Data display: tree, properties, explorer (slices 02 and 07)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Tree navigation state | five loose fields on `App`, `app/tree.rs` 339 | selection retained by id, nearest-duplicate tiebreak, expansion set, scroll | A (`TreeState<Id>`) |
| Columnar tree renderer | `left_panel.rs` 1710 | not a widget; 7 `render_*_spans` functions plus inline code, about 10 layouts; 3 alignment structs | A (`TreeView<R>` plus `ColumnSchema`) |
| Per-view row builders | `rows/*` 6,043 | 8 views (6 tabs plus hidden Union and Forks) | D |
| Group header, row prefix, status glyph, age cell, row states | `left_panel.rs` | 12 group-header constructions, 4 bucket-emission skeletons, `Other` defined 3 times | G |
| Property rows | `right_panel.rs:440` | label column, placeholder, annotation, copy and inspect | G (`PropertyGrid`) |
| Link rows | `right_panel.rs:506-702` | validated, alt, conflict, unresolved tones | A |
| Zoned explorer state machine | `explorer.rs` 2156, `explorer_nav.rs` 393 | flat cursor over zones with stable row keys, drill stack, armed back; about 48% domain builders, about 13% domain-free; `impl App` using 7 `App` fields | A, staged late |
| Stacked master-detail layout | `right_panel.rs:5-172` | content-sized list zone, divider, remainder | G |
| Breadcrumb | `explorer.rs:914-1063` | elides by hop | A |
| Legacy sectioned detail | `detail.rs` 1409, `detail_header.rs` 376 | superseded by the explorer | delete |

### 2.6 Chrome (slice 07)

| Surface | Where | Variants in play | Verdict |
| --- | --- | --- | --- |
| Screen shell and overlay layer | `ui.rs` 395, `runtime.rs:843` | `draw` dispatches viewport messages mid-render; only the top modal draws | A |
| Split pane, pane frame, focus marker | `ui.rs:312-335` | split logic copied 3 times; focus shown by title marker only, border never changes | G |
| Tab bar | `left_panel.rs:222-244` | probably clips at 100–107 columns *(code-read)* | G with elision |
| Header line | `header.rs` 681 | prefix plus opt-in chips | D composition over a G priority-line primitive |
| Status bar and hints | `header.rs:188-227,482-582` | hand-written hints in 3 places that can drift from key tables | G, generated from key tables |
| Section rule | `detail_header.rs:281` | chip-divider rule | G |
| Empty state | `left_panel.rs:317`, `right_panel.rs:868` | 12 hand-written messages | G slot, D copy |

### 2.7 Interaction conventions (slice 06)

| Contract | Where | State | Verdict |
| --- | --- | --- | --- |
| Key tables: `KeyRow<A>`, `resolve`, `footer`, `mislabeled`, `key_label` | `keybindings/overlay.rs` 834 | 20 tables, about 210 rows; the generic half imports only the matcher | G, strongest layer-2 contract |
| Modal key convention | `MODAL_CONVENTION` | matches `Debug` names of actions | G with a key-role classifier |
| `Overlay` trait, outcome, modal stack | `modal.rs` 202, `app/overlays.rs` 937, `runtime/overlay_keys.rs` 634 | trait has `handle` but no `render`; 14 impls; only the top modal draws; Controls and Search clone an 11-field context snapshot | A: generic `Overlay<M, Cx>`, `Outcome<M>`, `ModalStack` |
| Help generation | three copies of the generator | | G |
| Global keymap and focus remap | `keybindings.rs` 677, `keymap.rs` 337 | one `KeyMode`; 58 `help_text` strings never rendered | D; the framework designs a scoped keymap fresh |
| Clipboard (OSC 52) | `clipboard.rs` 137 | zero `crate::` imports; writes to process stdout | G with a `Write` sink |
| Message log | `tui/messages.rs` 281 | bounded, repeat-collapse, unseen counter | A |
| Effects, lifecycle, undo | `effect.rs` 387, `undo.rs` 98 | exhaustive classifier plus an iterating test | D (layer 3); only the pattern is generic |
| `--snapshot` mode and key DSL | `snapshot.rs` 571 | cannot drive 10 of the 16 overlay key actions; insta goldens drop styles | A test kit |
| Consistency suite and hygiene guards | `consistency_tests/` 6.4k lines, `tests/*_hygiene.rs` | see section 1 | A test kit |
| Pantry gallery | `examples/pantry.rs` 1,030 | covers 5 of 16 modals | G pattern; fixtures stay |

## 3. Spotlight: The Left-Pane Tree

The flagship opinionated composite (slice 02). Today it is a function that
builds one `Line` per visible row and hands the lot to a `Paragraph`; there is
no list, table, or tree widget, and no declarative column description.

| View | Row kinds | Groupings | What differs from the shared grid |
| --- | --- | --- | --- |
| Sessions | group, session (lineage children nested) | graph, workspace, repo, checkout, none | worktree level collapses when a repo has one session-bearing worktree; flat mode adds a project column; bucket-scoped title disambiguation |
| Mux | mux session (expandable), nested agent, group, ghost mux | session, host, repo, workspace | ghost muxes emulate the grid with `MixedGrid::Mux`; progressive cell dropping with a final ellipsis |
| PRs | group, PR, nested agent | checkout, repo, state | tree-wide label width that shrinks to `#N`; state-to-glyph logic in 6 places across 4 files |
| Pinned | group, mixed pin/mux/session rows | project, kind, none | the only view with status words; chip dropped first at narrow widths |
| History | group, mixed rows | project, kind, none | every row is a ghost |
| Search | group, session, pane, hit, pane hit | none, project, harness, plus a sessions/messages axis | lazy hit children; splices pane rows by capture time |
| Union (hidden) | session, mux | accepts two groupings and ignores both | no pin marker, ages show `—` |
| Forks (hidden) | fork, nested agent | accepts two groupings and ignores both | no tests |

Narrow-width handling is not one policy: Mux rows drop cells and end with an
ellipsis, Session/PR/Hit/Pane rows hard-clip at the pane edge, and Pinned rows
drop the chip first. The "room remains" cutoff takes three values (>2, >4, >8),
and about 25 width literals plus about 40 hard-coded two-space gutters are
spread through `left_panel.rs`.

Cell content can reach a generic control three ways: pre-rendered lines from
the app, a declarative column schema plus a cell producer, or a trait on the
row. The slice recommends the schema as the end state, reached through
pre-rendered lines first (details in
[02 §6.1](inventory/02-left-pane-tree.md)).

## 4. Spotlight: The Path Omnibox

The tab-completed path widget (slice 03, `path_omnibox.rs`, 333 lines). It
imports nothing from the app except its key table, but it is not
behaviorally neutral.

| Aspect | Today |
| --- | --- |
| Callers | new mux `cwd`; mux launch `cwd`/`repo`; pin edit path rows (Mux `cwd`, Checkout `location`, Repo `path`) |
| Customization | known candidates with priorities; a title; the initial value |
| Completion | Tab completes the single best remainder; known candidates rank above filesystem prefix matches |
| Validity mark | `?` unknown, `√` exists, `!` advisory; validates with `exists`, not `is_dir`, so a file passes for a `cwd` |
| Rendering | two renderers: an inline `Tab: x` hint reserving 32 cells (new mux, mux launch) versus the mark only plus a detail line (pin edit) |
| I/O | one `stat` per path row per frame and a `read_dir` per frame while focused; no injection point for the filesystem or `$HOME` |
| Home expansion | `~/x` validates but is committed unexpanded to a CLI that does not expand it; three separate `~` implementations exist |
| Dead surface | `selected`, `MAX_CANDIDATES`, `PathCandidate.source`, `PathSuggestion.exists`, `expanded_value()` (0 production callers) |
| Backlog | CSP-455 asks for a ranked candidate list; CSP-434 for a browse mode |

## 5. Cross-Cutting Duplication

| Concept | Implementations | Where |
| --- | --- | --- |
| Truncate / ellipsize / pad | 13 | `ui/text.rs`, `form::fit`, `popup_frame`, viewer, CLI, others |
| Word wrap | 6 | five hand-written plus ratatui |
| Scroll-to-cursor | 6 stateless copies | menus, pickers, forms |
| Modal rect | 5 functions, 13 rules | each overlay |
| Selection highlight | at least 6 widgets, 4 looks | `►`+BOLD, `>`+REVERSED, accent-only, `selection_active` |
| Footer / hint renderers | 10 renderers, 5 style recipes | overlays plus status bar |
| Text-edit engine / windowing | 2 / 3 | `tui-input`, `FindPrompt` |
| Match painters / snippet croppers | 7 / 2 | search, viewer, left pane |
| Pager | 7 implementations of one shape | transcript, pane, help (3), value modal, messages detail |
| ANSI handling | 3 paths | preview, viewer, pane |
| PR state to glyph and colour | 6 places in 4 files | `rows/prs.rs`, `left_panel.rs`, others; draft rule has 3 variants |
| Age formatting | 5 | TUI 4, CLI 1 |
| Group-header construction | 12 | `rows/*` |
| Outcome enums for "commit or close" | 5 | overlays |
| Per-modal runtime handlers | 16 | `runtime/overlay_keys.rs` |
| Modal registries kept by hand | 4 | tests and helpers |
| `strip_ansi` copies | 4 | tests |

## 6. Design-Language Divergences That Block Deduplication

Each row is a place where two controls that should look and act alike do not.
The control-set checkpoint ([control-set.md](control-set.md)) proposes one
house rule per row; that is where the operator rules on them.

| Axis | Divergence |
| --- | --- |
| List cursor and highlight | `►`+BOLD (menus), `> `+REVERSED (Controls, Picker, Search, forms), `selection_active` (Messages, left pane), invisible in the unfocused explorer |
| List ends | menus clamp; lists wrap; the tree and explorer clamp |
| Confirm | inline arm (Forget), y/n sub-screens (worktree), m/d choice, nothing (Unpin, Rebuild) |
| Close keys | `Esc`/`q`/`Ctrl-C` mean step-back, leave, or close-all depending on the overlay; Ctrl-G jumps to top in every pager |
| Close feedback | 6 overlays say "cancelled" on Esc, 8 stay silent |
| Enter in forms | new mux advances; mux launch and pin edit submit from any row |
| Tab stops | mux launch has separate row and stop rings; the others have one |
| Validation errors | 3 displays plus 4 silent failures (branch prompt, max age, new mux, empty pin rename) |
| Footers | pinned in pin edit, scrolling in the others; 5 of 9 control families render one; grammar varies (`close` / `to close`) |
| Path hint | 32-cell inline hint versus mark only |
| Page size | fixed 8 lines in help, value modal, and messages; viewport-sized in viewers; tree pages by terminal height minus 2 |
| Focus cue | title marker only; modal borders always use the accent |
| Toast | no severity; border always green; the status line paints every message in the warning colour |
| Truncation unit | cells in some places, `char`s in 6 |
| Forms and theme | forms ignore `[tui.theme]`; fixed White-on-DarkGray fills |
| Labels | `mux name`/`name`, `launch argv`/`argv`, `branch`/`worktree_branch` for the same fields |
| Hand-written hints | the status bar, `remap_for_focus`, and the help text each describe the right-pane keys; four status hints in `runtime.rs` escape the hygiene guard |

## 7. Suspected Defects *(all code-read; reproduce before fixing)*

| ID | Claim | Source |
| --- | --- | --- |
| D1 | Enter-to-copy and `o` index `core_fields` while rows index the expanded field list, so extra fields do nothing with `F` on | 07 §3.17 |
| D2 | `G` and `PgDn` in the value modal and Messages detail set an unclamped scroll offset; the body can go blank | 04 D16 |
| D3 | Three hand-rolled menus call `clamp(8/10/5, area.height)`, which panics on terminals shorter than the minimum | 04 D17 |
| D4 | The row menu cannot scroll; the committed `history_row_menu` snapshot already loses its footer at 24 rows | 04 |
| D5 | PageUp/PageDown in the tree step by terminal height minus 2, not the measured viewport; the scroll offset is never clamped to content length | 02 §12 |
| D6 | Group-label alignment uses visible rows, but `docs/tui-columns.md` says the whole tree; glyph-less bucket headers start 2 cells left | 02 §12 |
| D7 | A closed draft PR can show a yellow `⇄` beside a red `⊗` | 02 §12 |
| D8 | `~/x` passes path validation and is committed unexpanded | 03 D19 |
| D9 | Multi-line paste submits a form (no bracketed paste); Alt-B/Alt-F word motion is probably inert (`tui-input` binds META, crossterm reports ALT) | 03 §10 |
| D10 | Viewer help pages by the transcript's viewport (skips at least 3 lines per page); main help over-scroll is never clamped back | 05 §9 |
| D11 | Invalid input is rejected with no message (max-age, empty branch, new mux) | 04 D22 |
| D12 | `tui --fixture` interactive mode cannot dismiss a menu opened with `.` | 06 §10 |
| D13 | The tab strip probably clips at 100–107 columns; the header may read `0/N sessions` outside Sessions and Mux | 07 §3 |
| D14 | The chip keeps its normal look on a selected row because REVERSED on REVERSED is a no-op | 02 §8 |

## 8. Dead Or Superseded Code

- `detail.rs`, `detail_header.rs` (about 1,800 LOC plus 1,180 LOC of tests):
  production reads only `.kind` and `ambiguous_muxes_for_group`; the `e` key on
  the left pane toggles a flag with no visible effect.
- `LaunchSpecFormState` / ADR 0097: its second consumer (`PinCreateState`) no
  longer exists.
- `GroupSummary.attached` and `.unmuxed`, `RowTree.view`, `form::tagged_input_row`,
  `form::warning_row`, `LinesBody`, the path omnibox fields listed in section 4,
  and `InputOutcome::Confirm` arms in three forms.
- Union and Forks views accept groupings their builders ignore; Forks has no
  tests.
- `pending_pin_remove` (named in `docs/tui-architecture-review.md`) no longer
  exists; `explorer_back_armed` is an unrelated press-twice gate.
- Stale text: `modal.rs`'s claim that the whole stack draws and that every
  variant implements `Overlay`; ADR 0076's claim that the focused border is
  highlighted; `tui-detail-mockup.md`.

## 9. Third-Party Crates

| Crate | Used for | Assessment |
| --- | --- | --- |
| `ratatui` 0.30 | everything | the framework's base; direct use is fine |
| `tui-input` | the single edit engine | keep behind `LineEdit` |
| `ratatui-cheese` | multi-select shim; inert `help::Binding` | the checklist shim needs a mirror state; building in-tree is a real option; drop for help |
| `ratatui-comfy-toaster` | toasts | border colour hard-coded; own or fork to honour the theme and add severity |
| `tui-popup` | popup framing | each modal computes its rect twice to satisfy `KnownSize`; wrap privately or drop |
| `tui-markdown`, `syntect`, `comfy-table`, `ansi-to-tui` | rich text | own behind a cargo feature |
| `tui-pantry` (dev) | gallery | pattern extracts; fixtures stay |
| `rat-widget` | evaluated in CSP-433 | no-go stands (mandatory focus/event/scroll trio) |

Moving any of these into a new crate is a dependency-ownership change. The
project rules require an ADR (see the control-set document).

## 10. How Conspectus Leaks Into The Widgets

| Leak | Count and shape | Fix class |
| --- | --- | --- |
| `Msg` as the commit type | every overlay | (a) generic `M` plus `on_confirm` closure |
| `App` / context snapshots | Controls and Search clone an 11-field context | (a) host-chosen `Cx` and a stack held outside the model |
| Domain-named theme tokens used as generic roles | `link_id` (mux name colour), `mux_attached`, `mux_unmuxed`, `cwd_mark`, `pr_*`, `node_*` | (b) role names in the framework; aliases in the app |
| `RowKind` / `RowId` | 28 uses in `left_panel.rs`, 53 more in 10 other files | (a) generic `Id`; the app owns the payload |
| `crate::viewer` imported by `tui` | `word_wrap`, `find_ci`, `snippet_around`, `FindPrompt`, `paint` | (a) move to the framework; the viewer then depends on it |
| Viewer's own `Theme` borrowing domain tokens | `harness_color`, `cwd_mark`, `pr_merged` | (b) generic tokens |
| Draw dispatching messages | `Msg::LeftViewportChanged` mid-draw | (a) `set_viewport` called by the widget |
| Domain lookups inside renderers | `pinned_nodes()`, `presence()`, `row_is_frozen` | (b) precompute in the adapter |
| Key tables importing app action enums | `PaneViewerMsg`, `ViewerFindMsg`, `ViewerMsg` at `overlay.rs:634-636` | (a) tables generic over the action type |

## 11. Where The Questions Go

The slice files raise about 85 open questions. They collapse into the house
rules and structural choices listed in [control-set.md](control-set.md), which
is where they are answered. The ones that are really defect triage (section
7) are filed as separate stories ahead of the extraction work.
