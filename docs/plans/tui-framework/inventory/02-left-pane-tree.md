# Inventory 02: Left-pane tree view and columnar rows

Status: evidence notes for the inventory phase (read-only survey; nothing in
this file changes code). Paths are repo-relative; `file:line` citations are
against the `worktree-tui-framework` checkout at commit `ca668a3`. Behavior
claims marked **(code-read)** were derived from reading the code and were not
confirmed by running the TUI (this survey may not run cargo or tmux).

## 0. Slice map and headline numbers

| Area | Files | LOC (prod / tests) |
| --- | --- | --- |
| Row view-models (domain builders) | `src/tui/rows/{mod,sessions,mux,prs,pinned,history,forks,union,search,content}.rs` | 6,043 prod (6,556 incl. a 513-line test fixture `all_kinds_world` at `rows/pinned.rs:588`) |
| Row tests | `src/tui/rows/*_tests.rs` | 5,783 lines, 136 `#[test]` (rows_tests 17, sessions 56, mux 25, prs 18, pinned 12, history 8); **no** tests for `forks.rs`/`search.rs`/`content.rs` builders in `rows/` (search is covered from `consistency_tests/search_*.rs`) |
| Left-pane renderer | `src/tui/ui/left_panel.rs` | 1,710 |
| Width/ellipsis helpers | `src/tui/ui/text.rs` | 210 |
| Tree state (selection, expansion, retention) | `src/tui/app/tree.rs` | 339 |
| Scroll reconcile, `visible_rows`, per-view slot | `src/tui/app.rs:333-358, 621-700, 1266-1375` (inside the 2k-line `App`) | n/a |
| Cursor math | `src/tui/cursor.rs` | 52 (9 callers) |
| Chip primitive | `src/tui/widgets/badge.rs` | 149 |
| Snippet cropper | `src/tui/widgets/snippet.rs` | 198 |
| Glyph slate | `src/tui/icons.rs` | 514 |
| Living reference | `docs/tui-columns.md` | 724 |
| Tests that touch the left pane | `src/tui/ui_tests.rs` (169 tests, ~50 are left-pane), `consistency_tests/{rendering(13), state(6), styling(4), docs(5), search_flat(15), search_view(13)}.rs`, `tests/tui_columns_doc.rs` (frames), `src/tui/snapshots/*.snap` (19 insta files) | |

Key architectural facts before the details:

1. The left pane is **not a widget**. It is a free function
   `draw_left_panel(app: &mut App, frame, area)` (`left_panel.rs:7`) that reads
   ~20 different `App` accessors, builds one `Line` per visible row, and hands
   the whole `Vec<Line>` to a `Paragraph` with `.scroll((offset, 0))`
   (`left_panel.rs:137`). No ratatui `List`, `Table`, or tree crate is used.
2. The tree **state is not a struct**. Selection, expansion set, scroll offset,
   and the duplicate-id tiebreaker are five separate fields on `App`
   (`app.rs:129,131,241,259`, plus `ViewStateSlot` at `app.rs:338`), mutated by
   `impl App` methods in `app/tree.rs` and reducer arms in `app.rs:1437-1500`.
3. The row model (`RowKind`, `RowId`) is the app's **central entity index**, not
   just a render model: it is also read by header counts (`ui/header.rs:163`),
   preview selection (`ui/preview_pane.rs:125`), quick-select corpus
   (`src/tui/search.rs:78,95,148`), keymap (`keymap.rs`), runtime, and actions
   (`actions.rs:176 subject_of_row`). 28 `RowKind::` references in
   `left_panel.rs`, 53 more across 10 other non-builder files.
4. There is **no declarative column description** in the TUI. Columns are
   hard-coded per row kind: seven `render_*_spans` functions plus inline group-header and fork code (ten layouts counting the Mixed row's two grids). (The CLI has
   one: `src/output/render.rs:234 ColumnSpec` + `fit_to_width` at `:1084`; the
   two systems share nothing.)

## 1. Controls and primitives found (index)

| # | Name (candidate base control) | Where | Verdict |
| --- | --- | --- | --- |
| 1 | Tree/list navigation state (`TreeState<Id>`) | fields on `App` + `app/tree.rs`, `app.rs:1280-1375,1437-1500`, `cursor.rs` | split into generic core + Conspectus adapter |
| 2 | Row model (`Row{id,depth,expandable}` + `RowTree`) | `rows/mod.rs:136-144, 100-105` | extract after refactor (generic over `Id`; `RowKind` stays Conspectus) |
| 3 | Columnar row renderer / column grid | `ui/left_panel.rs` (`render_left_row`, seven `render_*_spans`, group/fork inline code) | split into generic core (new) + Conspectus adapter; today it is not generic at all |
| 4 | Tree-wide column alignment (`GroupAlign`, `PrAlign`, `MixedAlign`) | `left_panel.rs:587-723` | extract after refactor (replace three structs with one measure pass keyed by column) |
| 5 | Row prefix (indent, disclosure, kind glyph, pin marker) | `left_panel.rs:942-977, 1696-1710, 738-745, 1542-1678` | extract after refactor |
| 6 | Group header row (bold label, muted mid-ellipsised secondary, right-aligned count, warning) | `left_panel.rs:464-581, 979-1013` | extract after refactor |
| 7 | Age cell (4 wide, right aligned, recency tint) | `left_panel.rs:761-773`, `rows/mod.rs:999-1158` | extract as-is after making thresholds/theme injectable |
| 8 | Chip / badge (`harness_badge`, `command_badge`) | `widgets/badge.rs` | extract after refactor (generic `Chip{label,color}`; harness palette stays in Conspectus) |
| 9 | State glyph cell + spinner swap | `left_panel.rs:1680-1694, 1259-1273, 809-819, 1582-1595` | split (generic `StatusGlyph{glyph,hue}`; mappings are domain) |
| 10 | Ghost / pending / selected row styling | `left_panel.rs:884-907, 1072-1098` | extract as-is (rename to generic row states) |
| 11 | Snippet with match highlight | `left_panel.rs:1499-1523`, `widgets/snippet.rs` | extract as-is (merge with the near-copy in `widgets/search.rs:1268`) |
| 12 | Width helpers (truncate, middle-truncate, pad, fit spans) | `ui/text.rs:6-131` | extract as-is; merge with 6+ near-copies (section 12) |
| 13 | Vertical scrollbar gutter | `left_panel.rs:152-216` | extract as-is; merge with `widgets/help.rs:262-286` |
| 14 | Tab-strip pane title + focus marker | `left_panel.rs:222-244`, `ui.rs:331` | extract as-is |
| 15 | Pane chrome slots (header line, footer banner, empty-state text) | `left_panel.rs:14-63, 246-418` | split (slot mechanism generic; copy is domain) |
| 16 | Row builders per view | `rows/*.rs` | stays in Conspectus |
| 17 | Cursor math (`wrap_step`, `clamp_step`) | `cursor.rs` | extract as-is |
| 18 | Left-pane consistency harness (token/column scraping, ghost-style probe) | `consistency_tests/{rendering,state,styling}.rs` | extract the technique (helpers), not the cases |

## 2. The row and tree model

### 2.1 Data types (`src/tui/rows/mod.rs`)

| Type | Lines | Shape | Generic? |
| --- | --- | --- | --- |
| `RowTree { rows: Vec<Row>, view: ViewLabel }` | 100-105 | flat depth-first list; no parent pointers; no child index | generic once `Id` is a type parameter. `view` is **never read outside tests** (set by every builder; `ViewLabel` + `From<View>` at 108-133 are vestigial) |
| `Row { id: RowId, depth: u8, expandable: bool, kind: RowKind }` | 136-144 | tree structure is implicit in `depth`; `expandable` is a builder-decided flag | generic core is `{id, depth, expandable}`; `kind` is the payload |
| `RowId` (14 variants) | 42-94 | `Group(NodeId)`, `AgentSession`, `MuxSession`, `Pr`, `Fork`, `History{record_id}`, `Pin{pin_id}`, `Nested{pin_id,node}`, `Hit{session,turn}`, `Pane{mux,pane}`, `PaneHit{mux,pane,line}`, `Bucket(String)`, `Remote(String)`, `Synthetic(&'static str)` | the **requirements** are generic (stable, hashable, `Ord` because expansion is a `BTreeSet<RowId>`); the variants are domain. Identity is content-addressed so selection survives rebuilds |
| `RowKind` (9 variants) | 153-169 | `Group, AgentSession, MuxSession, Pr, Fork, Mixed, Hit, Pane, PaneHit` | domain |
| `GroupRow {display_path, primary_node, is_launch_context}` | 374-386 | **stringly typed columns**: `display_path` packs label and secondary with a two-space separator, decoded by `ui/text.rs:135 compact_path_label` / `:160 compact_path_secondary` (workspace `"name  a+b  (provider)"`, PR checkout `"owner/repo  ~/path"`, no-checkout `"~/path  no checkout"`, checkout marker `"~/path (linked · locked)"`). The special string `"Ungrouped"` is compared literally at `text.rs:136,161` | domain contract encoded as text; must become explicit cells |
| `AgentSessionRow` (15 fields) | 389-445 | harness label, key, recency + epoch, `MuxIndicator`, preview, title/alias, `title_disambiguates`, `mux_host`, `pin_id`, `content_hit`, `project_display`, `cwd_display`, `short_id` | domain |
| `MuxSessionRow` (17 fields) | 525-566 | name parts, attached count, 3 epochs, agent labels, program, program harness, preview, `pin_id`, `content_hit` | domain |
| `PrRow` | 570-590 | number, `label` (`#N` or `owner/repo#N`, rewritten after the fact by the builder, `prs.rs:251-262`), state, draft, branch, tracked, title, recency, url | domain |
| `ForkRow`, `HitRow`, `PaneRow`, `PaneHitRow` | 594-601, 225-241, 174-194, 199-210 | | domain |
| `MixedRow` (13 fields) | 280-305 | the Pinned/History row: `node_kind`, `label`, `status` (word), `tone` (Normal/Dim), `age` + `age_epoch`, `state: Option<EntityState>`, `chip: Option<MixedChip>`, `content`, `primary_node`, `pr_state`, `marker: PinMarker`, `grid: MixedGrid` | the closest thing to a **generic row**: kind glyph + label + status + age + chip + content. It already carries presentation intent (`tone`, `marker`) |
| `MixedGrid {Columns, Mux}` | 363-370 | a flag that makes a `MixedRow` draw with the **Mux view's** grid so ghost muxes line up with live ones | a workaround for lacking a shared grid (section 12, D6) |
| `words::*` | 246-265 | 12 status words, `ALL` array, checked against `docs/tui-columns.md` | domain vocabulary, but the "vocabulary table + doc test" mechanism is generic |
| `PinMarker {None, Running, Stopped}` | 337-345 | the 2-cell marker column | generic pattern (leading marker with two states), domain naming |
| `EntityState {Linked, Unlinked, Stopped, Unknown}` | 350-359 | doc says "sessions and muxes", but it is only produced for ghost muxes (`mux.rs:715-717`) | domain |

Pure helpers in the same file: `shorten_home` (610), `format_recency` (999),
`recency_bucket` + `RecencyBucket::style(&Theme)` (1130, 1145), `retain_pinned_rows`
(947: keep a row when it or a descendant matches, a generic "prune with
ancestors" over the depth encoding), `format_workspace_display` (639),
`repo_display_path` (665), and the mux/pin program-resolution family (774-884,
purely domain).

### 2.2 Build pipeline

`build_tree_for_view(TreeInputs)` (`rows/mod.rs:1209`) dispatches on `View` to
eight builders (`1225-1320`), then optionally `content::annotate` (a post-pass
that writes `content_hit` into session rows and single-agent mux rows,
`rows/content.rs:20`) and `retain_pinned_rows`. Builders are deterministic
functions of `(&GraphSnapshot, projection inputs, home, now)` with no ratatui
types; this is the cleanest seam in the slice. Two purity leaks:
`tree_home_dir()`/`tree_current_unix_epoch()` read env/clock inside the
dispatcher (`1322-1343`, with a `FROZEN_NOW` `OnceLock` for snapshot mode), and
`pinned.rs:537` re-reads `$HOME` in `children_of` instead of using the
`home` input (**code-read**).

Cost shape per rebuild: every builder does a full pass over `snapshot.nodes`
and `candidate_links` (several linear `link_by_id`/`find` scans, e.g.
`sessions.rs:488`, `mux.rs:612`); acceptable at current scale.

### 2.3 State: selection, expansion, scroll, visible-index math

| Concern | Where | Behavior |
| --- | --- | --- |
| Selection | `App.selection: Option<RowId>` (`app.rs:131`); `set_selection` (`app.rs:790`) | identity-based. `set_selection` silently ignores ids not in the tree, resets the tiebreaker, clears `status_message`, **calls `recompute_detail()`**, so selection change is coupled to the right pane |
| Retain across refresh | `set_data` (`tree.rs:6-56`), `set_tree` (`tree.rs:89-114`) | previous id kept if present; else clamp to the previous visible index; else first row. `set_data` additionally honors a first-load `initial_selection_hint` (launch-context group). The retention block is **duplicated** between the two functions (`tree.rs:27-54` vs `96-112`) |
| Duplicate ids | `position_closest_to` (`tree.rs:121`), `current_visible_index` (`tree.rs:191`), `last_visible_index` (`app.rs:259`) | when one `RowId` appears twice, step from the occurrence nearest the cached index. Comment cites the Mux view; the PRs view can still produce duplicates (a session whose cwd sits under two PR checkouts, e.g. a worktree nested under the repo root, `prs.rs:333 agent_attached_to_roots`) **(code-read)** |
| Visible rows | `App::visible_rows()` (`app.rs:1357`) | O(n) depth scan: a row is hidden while an ancestor (tracked as `hidden_below: Option<u8>`) is collapsed. Recomputed on every call: draw (`left_panel.rs:55`), the viewport message (`app.rs:1467`), `visible_rows_owned()` (`tree.rs:251`, **clones every `RowId`, which carry `String`s**) on every `j`/`k`, `set_tree` x2, `select_row_expanding`. No cached visible-index vector |
| Expansion | `App.expanded: BTreeSet<RowId>` (`app.rs:129`); saved/restored per view in `ViewStateSlot` (`app.rs:338-358, 621, 651`) | in memory only (not persisted across runs). `toggle/expand/collapse_selected` (`tree.rs:214-249`) each re-scan `tree.rows` linearly for "is selection expandable" (3 copies). `expand_to_and_select` (`app/overlays.rs:218`) opens ancestors for jump-to. Stale ids are never pruned |
| Initial expansion | `initial_expanded_rows` (`tree.rs:286-333`), `seed_first_visit` (`app.rs:682`), `show_top_of_tree` (`app.rs:674`) | all `RowId::Bucket` groups; the launch-context group plus its ancestors and descendants; else the first `Group`. Search view auto-opens every new expandable row, but at most 10 sessions (`app/content.rs:1360`, `AUTO_EXPAND_SESSIONS = 10`) |
| Lazy children | `rows/search.rs:405 row.expandable = !children.is_empty() \|\| detail_pending`, `app/content.rs:468 queue_details` | an expandable row whose children are loaded after it is opened. No framework-level notion; it is a side channel through `App` |
| Scroll offset | `App.left_scroll: u16` (`app.rs:241`); `adjust_left_scroll` (`app.rs:1280`); reducer arm `Msg::LeftViewportChanged` (`app.rs:1455`); getter `left_scroll()` (`1300`) | one line per row. Minimal-motion rule: only moves when the selected line leaves `[offset, offset+vh)` (no centering, no scroll-off margin). Draw measures `inner.height` and dispatches the message each frame. **Offset is never clamped to `total - viewport`** **(code-read)**, so collapsing or filtering can leave a blank tail with earlier rows scrolled off. Not covered by a test (`app_tests.rs:516-547` only test the three selection cases). Saved per view; reset on regroup (`app.rs:674-680`) |
| Page step | `translate(event, viewport_height)` (`keymap.rs:239`) from `runtime.rs:146 terminal.size()?.height.saturating_sub(2)` | PageUp/PageDown move the **selection** by terminal height minus 2, not by the measured tree viewport. The tree's real `inner.height` is at least 2 rows smaller (borders) and is halved in the narrow stacked layout (`ui.rs:312-317`); it is also reduced by the Search header line and Pinned banner **(code-read)** |
| Wrap | `clamp_step` for the tree; `wrap_step` only for view cycling (`keymap.rs:142`) | tree does not wrap |
| Mouse | none anywhere in `src/tui` (no `MouseEvent`/`EnableMouseCapture`) | keyboard only |

Left-focus keys (all from the declarative table `keybindings.rs`, remapped by
focus in `keymap.rs:158-210`): `Up/k`, `Down/j` (`NavUp/NavDown`), `PageUp/PageDown`,
`Home/g`, `End/G`, `Right/l` (`ExpandRow`), `Left/h` (`CollapseRow`), `Enter`
(`DefaultAction`: toggles expansion on groups, otherwise attach/view/scrollback
via `selected_default_action`, `runtime.rs:2253`), `Tab` (focus). `h`/`l` on a
leaf or on an already-closed row are no-ops (no "jump to parent" or "jump to
first child"); there is no expand-all/collapse-all and no type-ahead inside the
tree. With right-pane focus `h/l` are dropped and `j/k/PgUp/PgDn/g/G` retarget
the explorer (`keymap.rs:158-203`).

### 2.4 What is domain-free (summary)

Domain-free today or with trivial genericization: `Row{id,depth,expandable}`,
the depth-scan visibility algorithm, selection retention, nearest-duplicate
logic, expansion set, `adjust_left_scroll` (the explorer has a near-copy,
`adjust_explorer_scroll` at `app.rs:1320`, differing only in tracking a first
and last row of a wrapped span), `clamp_step`/`wrap_step`, `retain_pinned_rows`
(prune-with-ancestors), `format_recency`/`recency_bucket`, `shorten_home`.
Everything that mentions `NodeId`, `GraphSnapshot`, harness, mux, pin, PR is
domain (builders and `RowKind`).

## 3. Rendering pipeline (`draw_left_panel`, `left_panel.rs:7-142`)

Order of operations each frame:

1. `Block::default().borders(Borders::ALL).title(left_panel_title(app))`
   (`:8-10`). Plain borders; **focus is shown only by the `►` in the title**
   and by the row highlight, never by border style.
2. Optional bottom banner (`:16-38`): the Pinned view's `pin migrate` advice,
   `Paragraph` + `Wrap{trim:true}` in `theme.warning`, 1-3 lines, only if
   `inner.height > 4`.
3. Optional top header line (`:42-53`): the Search view's "which search" line,
   only if `inner.height > 3`.
4. Empty state (`:55-63`): a `Paragraph` in `theme.placeholder` (DIM) with copy
   chosen by `empty_left_panel_text` (`:317-418`, ~100 lines, 12 different
   messages keyed on view, filter, search status, snapshot presence).
5. Alignment pre-pass (`:65-105`): `compute_group_summaries` over the **whole**
   tree (`:435`), `GroupAlign` label/body/count widths over **visible** group
   rows, `PrAlign::of_tree`, `MixedAlign::of_tree`, `native_marker_reserved`
   (each is a full pass over the rows every frame).
6. `row_width = scrollbar_layout(inner, visible.len()).0.width` (`:109`): rows
   are laid out against the width left after the scrollbar gutter.
7. One `Line` per **visible** row (`:113-121`), not per viewport row: the whole
   visible set is rendered every frame and the `Paragraph` clips it
   (**code-read**; virtualization is a framework opportunity).
8. `app.update(Msg::LeftViewportChanged{viewport_height})` mid-draw, read
   `app.left_scroll()`, `Paragraph::new(lines).scroll((scroll, 0))` (`:130-138`).
9. Scrollbar (`:139-141`): `scrollbar_layout` reserves a 1-column gutter only
   when `content_length > area.height` ("fade on fit", `:152-169`);
   `render_vertical_scrollbar` (`:191-216`) uses ratatui `Scrollbar`
   (VerticalRight, default track/thumb/arrow symbols) with
   `.remove_modifier(REVERSED)` so the selection highlight does not bleed in,
   a `content - viewport + 1` length so the thumb reaches the bottom, and a 1-row
   vertical margin.

Selection highlight (`render_left_row`, `:1086-1099`): `Line::style` with
`theme.selection_active` (default `REVERSED | BOLD`) when `app.focus()==Left`,
else `theme.selection_inactive` (default `BOLD`), also patched onto every span.
It fills the full content width (gutter excluded).

## 4. The column grid, precisely

### 4.1 Common row prefix (`render_left_row`, `:942-977`)

| Order | Cell | Width | Source | Notes |
| --- | --- | --- | --- | --- |
| 1 | Indent | 2 x depth | `row_indent` (`:1708`) | the "flat sessions has no indent" special case (`:943-952`, repeated in `group_row_body_width` `:521-527`) is a **no-op**: flat rows are depth 0, so `row_indent(0)` is empty either way; the second copy tests `row.kind` for `AgentSession` after destructuring it as `Group` and is always false |
| 2 | Disclosure | 2 | `disclosure_span` (`:1696`) | `▶ `/`▼ ` in `theme.disclosure` (cyan) when `row.expandable`, else two spaces |
| 3 | Kind glyph | 2 (glyph + space) | `row_kind_glyph_span` (`:1662`) | colour per `theme.node_*`; PR glyph hue by PR state; **absent** for `Group` with no node (Ungrouped, PR remote/state groups, Pinned/History/Search buckets), `Hit`, `PaneHit` |
| 4 | Pin marker | 2 | `pin_marker_span` (`:738`) | reserved on every row once any row has one. Two mechanisms: native views (Sessions, Mux, PRs, Search) look the marker up **at render time** from `SnapshotHandle::pinned_nodes()` (`:960-970`, `native_marker_reserved` `:614`); `Mixed` Columns rows carry `marker` in the row (`:972-977`, `MixedAlign.marker` `:691-696`) |

Total fixed prefix: 4 cells, 6 with the marker. Because glyph-less headers omit
cell 3, their labels start 2 cells left of sibling headers with a glyph (visible
in `docs/tui-columns.md` frames: `▶   Ungrouped` vs `▶ ◆   repo-a`); the count
column still aligns because bodies are padded to `align.body_width`.

### 4.2 Gutters and separators

Cells are separated by a literal two-space gutter written inline at ~40 sites
(20 x `Span::raw("  ")`, 16 more as `"  {..}"` format prefixes, a few trailing).
There is no `GUTTER` constant and no row-level "join cells with N spaces" helper.
The PR label-to-tracking separator is one space (`:1611`), the harness-chip
neighbors in a mux row are one space (`:1302`), everything else two.

### 4.3 Per row kind: columns, width policy, truncation, drop rules

Policy vocabulary: **Fixed(n)**, **Cap(n)** = padded to the widest value in a
scope, capped at n, **Fill** = takes the remaining budget and is cut with `…`,
**Drop** = omitted when the remaining room is below a threshold.

| Row kind (renderer) | Cells after the prefix, in order | Width policies | Narrow-width behavior |
| --- | --- | --- | --- |
| **Group** (`:979-1013`, `append_group_body_spans` `:468`, `append_group_summary_spans` `:563`) | label (bold) / secondary path (dim) / `(cwd)` / padding to body column / `(N)` / `▲` | label: Cap(visible-group max), never truncated; secondary: Fill with **middle** ellipsis (`truncate_to_width_middle`) in `width - head - label - 2 - (count_width+5)`; `(cwd)`: Fixed text, unbudgeted; count: right-aligned to widest `(N)`; `▲`: Fixed 1 | reserves `count_width + 5` for the summary block first, so count and warning survive; the secondary absorbs all slack; `(cwd)` is not budgeted and may be clipped by the pane edge |
| **AgentSession** (`render_session_spans` `:1104`, `append_session_preview` `:1185`) | key / badge / age / state / label / `was in` chip / project (flat only) / `N hits`+snippet or preview | key: Fixed 8 (`truncate_to_width_no_marker`, no ellipsis) + 2 pad; badge: Fixed `badge_width+2` (10); age: Fixed 4; state: Fixed 1; label: Cap 32, ellipsis; was-in: Cap `was in ` + 20; project: Fixed 16 (padded); preview/snippet: Fill, Drop if `width <= used + 8` | **no width awareness on the left cells**: only the trailing preview is budgeted; everything else is hard-clipped by the `Paragraph` with no `…` (**code-read**) |
| **MuxSession** (`render_mux_session_spans` `:1212`, `append_mux_agent_labels` `:1279`) | name / chips (<=2 or 1, `+N`) / age / state / preview or `N hits`+snippet | name: Fixed by tier (`mux_label_column_width` `:1316`: `width<=38 -> clamp(width-14,10,18)`, `39-72 -> 20`, `73-104 -> 24`, else 30), head-tail ellipsis for ids over 36 (`compact_mux_native_id`); chips: 2 when `width >= 76` else 1, each `badge_width+2`, `no agent` fallback; age Fixed 4; state Fixed 1; preview Fill, Drop if `width <= used + 8` | the only row kind with progressive disclosure: returns early at `width <= used + 4` after the name and after the chips, and ends with `fit_spans_to_width` (ellipsis on the cut span) |
| **Pr** (`render_pr_spans` `:1602`) | label / tracking / age / state / branch / title | label: Cap(tree-wide max of PR labels, unbounded); tracking Fixed 1; age Fixed 4; state Fixed 1; branch: Cap 24; title: Fill; each content part Drop if `room <= 2` | fixed cells are not truncated; branch and title shrink with `…` |
| **Mixed / Columns** (`render_mixed_spans` else-branch `:828-877`) | label / status word / age / chip / content | label: Cap(tree-wide max, 28) padded to `align.label - prefix` (so nested rows share the status column); status: Cap(tree-wide max, 9); age Fixed 4; chip: one tree-wide width, Drop if `budget - used <= cell + 2`, blank pad when the row has none; content: Fill, Drop if `room <= 2` | label/status/age never shrink; chip then content give way |
| **Mixed / Mux grid** (`:792-827`) | name (link colour) / chip / age / state glyph / content | name: Fixed by the same tier function; chip: one only (no `+N`); content: Fill, Drop if `room <= 4` | no early exits; final width is clipped by the `Paragraph` |
| **Hit** (`render_hit_spans` `:1385`) | [badge + name Fixed 12] / `turn N` (Fixed 4) / role (Fixed 6) / kind (Fixed 11) / age (`{:>4}`, not truncated) / snippet | snippet Fill, Drop if `budget <= used + 8` | fixed cells never shrink |
| **Pane** (`render_pane_spans` `:1431`) | mux (Fixed 16) / `w.p` (Fixed 5) / command (Fixed 8) / age (`{:>4}`) / `N hits` + snippet | same thresholds as above | |
| **PaneHit** (`:1464`) | [mux+place Fixed 16] / `line N` (Fixed 5) / age / snippet | | |
| **Fork** (`:1032-1044`) | bold label / `parent:x` / scope / `(N)` | none | none; no tests |

### 4.4 Alignment scopes (where a "column" is measured)

| Column family | Measured over | Computed at | Mechanism |
| --- | --- | --- | --- |
| Group label / body / count | **visible** group rows | draw, `:79-101` | `GroupAlign` (a fourth and fifth field carry the next two) |
| `GroupSummary` counts | whole tree (collapsed groups still advertise) | `:66` | `compute_group_summaries`, O(groups x descendants). `attached` and `unmuxed` fields are written (`:451-456`) and **never read** |
| PR label | whole tree | `:103` | `PrAlign::of_tree` |
| Mixed label / status / chip / marker | whole tree | `:104` | `MixedAlign::of_tree` (builds each chip span only to measure it, `:715-719`) |
| Native pin marker reservation | whole tree | `:105` | `native_marker_reserved` |
| Mux name, session key, badge, age, state, label caps | none (constants / tiers) | n/a | literals |

`docs/tui-columns.md` says the group label is "padded to the widest in the tree"
(row 1 of the Group header table); the code pads to the widest **visible** group
row, so folding a repo can shift the label column of other groups
(`ui_tests.rs:603` is named `group_row_summary_chips_align_across_visible_groups`; `:629` checks the secondary column across groups). Only the Pinned
test `folding_a_group_does_not_shift_the_columns_of_the_rows_left` (`ui_tests.rs:5213`)
pins the tree-wide rule, and it covers Mixed rows only.

### 4.5 How cell content is produced

| Cell | Producer | Notes |
| --- | --- | --- |
| Age | `age_span(age, epoch, now, theme)` (`:761`) | text from the builder (`format_recency` at build time), tint from `recency_bucket` at draw time with `theme.recency_{fresh,active,recent,cold}`; `—` (`marks::NO_VALUE`) when unknown; right-aligned in `AGE_CELL_WIDTH = 4`. Used by Session, Mux, Pr, Mixed. Hit/Pane/PaneHit print the age with a raw `{:>4}` in secondary text: not tinted, not truncated |
| Chip | `harness_badge`/`command_badge` (`widgets/badge.rs:34,46`) via `mixed_chip_span` (`:749`) | pads/cuts to `theme.badge_width` inside one styled span (so the padding carries the fill); style `theme.badge` (REVERSED+BOLD) over `theme.harness_color(label)`; program chips are white-on-`theme.command_badge`. Mux rows with no agent print a plain dim ` no agent ` span that is **not** a badge (`:1294`, `:804`) |
| State glyph | per row kind: `mux_indicator_span` (`:1680`), inline `match client_attached` (`:1262-1272`), inline `match mixed.state` (`:811-818`), `pr_state_span` (`:1582`) | all draw from `icons::state::*`; the spinner replaces the glyph (or the status word) while `handoff_spinner.is_some()` |
| Count | `append_group_summary_spans` | `"({N})"` right-padded to the widest |
| Warning | same | `▲` in `theme.warning` + BOLD |
| Snippet/hit | `append_content_hit` (`:1357`), `append_snippet` (`:1499`) | `N hits` chip then `snippet::fit` crop around the match; match in `theme.match_style()` (bold, underlined, `match_highlight`) |
| Ellipsis | `ui/text.rs` | see section 12 |

### 4.6 Row styling states (all applied after the cells are built)

| State | Trigger | Effect | Where |
| --- | --- | --- | --- |
| Selected + focused | `app.selection()==row.id && focus==Left` | whole line `REVERSED \| BOLD` | `:1086-1099` |
| Selected, pane unfocused | | whole line `BOLD` | same |
| Ghost | `RowKind::Mixed` with `tone==Dim`, or any non-Mixed row whose node has `snapshot.presence(node).is_some()` (a render-time domain lookup, `:1077-1085`) | fg `theme.ghost` (DarkGray), italic, bold removed; **chips keep their background and are dimmed instead** (detected by `bg.is_some() \|\| REVERSED`, `:894-907`) | `ghostify`, `ghost_style` (`:884-907`) |
| Pending hand-off | `app.handoff()` and `app.row_is_frozen(row)` (calls `actions::subject_of_row`) | old values kept; spinner glyph (`header.rs:253`) replaces the state cell/word; every span gets `theme.placeholder` (DIM) | `:938-941, 1072-1076` |

### 4.7 Magic numbers (every width/threshold literal in the renderer)

Named: `AGE_CELL_WIDTH=4` (`:685`), `MARKER_CELL_WIDTH=2` (`:726`),
`PR_BRANCH_MAX_WIDTH=24` (`:638`), `MIXED_LABEL_MAX_WIDTH=28` (`:664`),
`MIXED_STATUS_MAX_WIDTH=9` (`:666`), `FLAT_LABEL_WIDTH=12` (`:1381`),
`PANE_LABEL_WIDTH=16` (`:1426`). Function-local consts:
`SESSION_ID_COLUMN_WIDTH=8`, `SESSION_DISPLAY_LABEL_WIDTH=32` (`:1111-1112`).
Bare literals: project 16 (twice, `:1178-1179`), `was in` name 20 (`:1164`),
pane command 8 (twice, `:1442-1443`), location 5 (`:1447`), `turn` 4 (`:1406`),
role 6 (`:1407`), kind 11 (`:1408`), `line` 5 (`:1479`), search query 28/27 (`:271-272`),
mux tiers 38/72/104/10/18/20/24/30 (`:1316-1322`), two-badge threshold 76
(`:1299`), `compact_mux_native_id` 36/28/6 (`text.rs:196-209`), banner
height 4 and search header 3 (`:17,43`), mixed prefix `depth*2+4` (`:731`).
"Room remains" thresholds come in three values with no shared rule:
`> 2` (PR parts `:1626`, Mixed Columns content `:870`), `> 4` (ghost-mux content
`:821`, mux early exits `:1239,1246`), `> 8` (every preview/snippet, `:1199,1343,
1373,1412,1483`).

## 5. Per-view inventory

Views: `View` has 8 variants (`tui/mod.rs:220`); the tab strip shows 6
(`widgets/controls.rs:72 VIEW_OPTIONS`). Union and Forks are reachable only with
`--view`. Grouping/sort/filter values come from `Grouping::values_for`
(`tui/mod.rs:576-621`). Sort (`Hierarchy`/`Recency`) is global; `MuxRecency`
(activity/created/last-attached) is Mux-only.

| View | Row kinds | Groupings (default first) | Column schema | Distinct behavior / deviation from the shared grid |
| --- | --- | --- | --- | --- |
| **Sessions** (`rows/sessions.rs`) | `Group` (workspace `▦`, repo `◆`, checkout `◇`, Ungrouped), `AgentSession` (lineage children nested) | graph, workspace, repo, checkout, scan-root (alias of repo), none (flat) | prefix, key, badge, age, state, label, was-in, [project when flat], preview | worktree level collapses when a repo has one session-bearing worktree (`sessions.rs:715-745`); only view that sets `is_launch_context` besides PRs; flat mode adds the project column (16); labels use bucket-scoped title disambiguation (`sessions.rs:871`); `ShortIds` machinery (`:1069-1133`) |
| **Mux** (`rows/mux.rs`) | `MuxSession` (expandable when >1 agent), nested `AgentSession`, `Group` (workspace/repo/Ungrouped), `Mixed` ghost muxes | session (flat), host (identical to session), repo, workspace | prefix, name (tiered), chips, age, state, preview | ghost (stopped/unknown) muxes use `MixedGrid::Mux` to emulate the grid inside `render_mixed_spans`; group headers carry no counts unless agents nested; stopped muxes sort below running (`mux.rs:513`); repo/workspace header ordering is case-folded name then path (`:469-484`) |
| **PRs** (`rows/prs.rs`) | `Group` (checkout `◇`, "no checkout" repo `◆`, repo, Remote, State), `Pr`, nested `AgentSession` | checkout, repo, state | prefix, label, tracking, age, state, branch, title | tree-wide `PrAlign.label`; label shrinks to `#N` when a group has one forge repo (post-hoc rewrite `prs.rs:251-262`); header text assembled as `"slug  path"` and decoded back by `compact_path_label`; `ForgePr` glyph tinted by state |
| **Pinned** (`rows/pinned.rs`) | `Group` (Bucket), `Mixed` Columns (pin rows + nested mux/session rows) | project, kind, none | prefix(with marker), label, status word, age, chip, content | the only view with **status words**; nested rows reuse the same grid with marker omitted; sessions under a mux pin, muxes+sessions under a worktree pin (`pinned.rs:533-579`); `Other` bucket sorts last by key tuple `(u8,String)` |
| **History** (`rows/history.rs`) | `Group` (Bucket), `Mixed` Columns | project, kind, none | same as Pinned | every row is a ghost (`tone=Dim`); marker only when pinned records are listed; no nesting |
| **Search** (`rows/search.rs`) | `Group` (Bucket / "Mux panes"), `AgentSession` (taken from the flat Sessions builder), `Pane`, `Hit`, `PaneHit` | none, project, harness; plus a second axis `Sessions`/`Messages(flat)` | session cols with hit chip + snippet; hit/pane/pane-hit schemas | a first-line header, rank-ordered, lazy hit children (`detail_pending`), auto-expansion; reuses `build_sessions_tree` then re-sorts (`search.rs:83-142`); flat listing splices pane rows by capture time (`:226-237`) |
| **Union** (`rows/union.rs`) | `AgentSession`, `MuxSession` at depth 0 | `UnionGrouping::{Kind,Repo}` accepted but **ignored** | session / mux cols | no pin marker; ages are `—` because `now=None` (`union.rs:56`); only builder with inline tests |
| **Forks** (`rows/forks.rs`) | `Fork`, nested `AgentSession` | `ForksGrouping::{Provider,Parent}` accepted but **ignored** | label, parent, scope, `(N)` | unbudgeted, unaligned; `now=None`; **no tests** |

## 6. Generic vs domain: what a generic columnar tree control is

Parts that form a generic "columnar tree/list" (all exist today, entangled):

- structure: `{id, depth, expandable}` rows; visibility by collapsed ancestors;
  expansion set; selection by id with retention across rebuild, nearest-duplicate
  tiebreak and clamp fallback; ensure-visible scroll offset (minimal motion);
  page/home/end/step; measured-viewport handoff; scrollbar gutter that appears
  only on overflow; focused vs unfocused selection style.
- cells: a prefix (indent, disclosure, optional kind glyph, optional marker), and
  typed cells (text with Cap/Fixed/Fill, chip, tinted age, status glyph, right-
  aligned count, muted secondary with middle ellipsis, snippet with match
  highlight).
- row states: selected/focused, ghost, pending (spinner swap + dim).
- chrome: tab-strip title with focus marker, optional header line, optional footer
  banner, empty-state text, scrollbar.

Domain (stays in Conspectus): all builders, `RowKind`/`RowId` variants, harness
colours and labels, the `tree_label` disambiguation rules, mux/PR/pin/history
semantics, status vocabulary, the glyph meanings, the content-search hit cells,
empty-state copy, `retain_pinned_rows` predicate, tab list.

### 6.1 How a generic control could take cell content

Three seams, ordered by how close they are to the current code:

| Option | Shape | Pros | Cons |
| --- | --- | --- | --- |
| **C. Pre-rendered lines** | app supplies `Fn(&R, RowCtx) -> Line<'static>`; framework owns state, scroll, selection style, scrollbar, helpers | closest to today (`render_left_row` already returns a `Line`); smallest first extraction; keeps all layout in Conspectus | the "opinionated columnar look" stays in app code; alignment pre-passes stay app-side; no consistency win across the ten layouts |
| **B. Declarative columns + cell producer** | app declares `Schema{columns:[Column{key, width: Fixed\|Cap(n)\|Fill, align, drop_below, priority}]}` per row type and supplies `cells(&R, &RowCtx) -> Vec<Cell{key, spans}>` | removes the three `*Align` structs and 40 inline gutters; makes narrow-width behavior uniform; the framework can measure tree-wide widths itself and add tests ("columns line up") for free | needs a measure pass over all rows (today `PrAlign`/`MixedAlign` already do), so cells for non-visible rows must be cheap or cached; migration touches every renderer |
| **A. Trait on the row** | `trait TreeRow { type Id; fn id(); fn depth(); fn expandable(); fn cells(&self, ctx) -> ... }` | idiomatic; lets the framework store `Vec<R>` | `RowKind` is one enum owned by the app with 9 variants and six consumers outside rendering, so the trait would sit on the enum; the framework still needs B's schema to avoid ad-hoc layout |

Recommendation for the control-set checkpoint (evidence-based, operator decides):
end state B built on a `TreeState<Id>` + `TreeView<R>` pair, with C as the
stepping stone (extract state/scroll/scrollbar/helpers first, keep
`render_left_row` as the app-side producer, then migrate renderers to schemas one
row kind at a time). Whichever option is picked, the `RowCtx` handed to the
producer needs: `expanded`, `selected`, `focused`, `frozen` (spinner), `ghost`,
`marker`, `theme`, and available width; today these come from seven separate
`App` touchpoints (`theme`, `handoff`, `row_is_frozen`, `grouping`,
`snapshot_handle` twice, `is_expanded`, `focus`), two of which
(`snapshot_handle().pinned_nodes()` and `.presence()`) are domain lookups inside the
renderer and belong in the adapter.

## 7. Conspectus coupling ("leaks"), classified

(a) parameterizable via generics/closure/trait; (b) belongs in a Conspectus-side
adapter; (c) inherently domain.

| Coupling | Where | Class |
| --- | --- | --- |
| `App` as the only state source (`&mut App` in `draw_left_panel`, `&App` in `render_left_row`) | `left_panel.rs:7,925` | (a) pass `&TreeState` + `RowCtx` |
| `Msg::LeftViewportChanged` dispatched mid-draw | `left_panel.rs:130`, `app.rs:1455` | (a) `TreeState::set_viewport(h)` called by the widget (this also fixes the page-step mismatch) |
| `RowKind`/`RowId` enums | everywhere | (c) generic `Id`, app-owned payload |
| `Theme` fields used by the pane: `secondary_text`(20 uses), `placeholder`(20), `pr_{open,merged,draft,closed}`, `mux_attached`, `mux_unmuxed`, `panel_focus_accent`, `link_id`, `ghost`, `warning`, `selection_{active,inactive}`, `cwd_mark`, `match_highlight`, `disclosure`, `command_badge`, `badge`, `badge_width`, `recency_*`, `node_*`, `harness_colors` | `left_panel.rs` | (a) for generic-named tokens (`secondary_text`, `placeholder`, `ghost`, `warning`, `disclosure`, `selection_*`, `recency_*`, `badge*`, `match_highlight`, `panel_focus_accent`); (b) for domain-named tokens used as generic roles (`link_id` = mux name colour, `mux_attached`/`mux_unmuxed` = "positive/dim" state, `cwd_mark`, `pr_*`, `node_*`, `harness_colors`) which should be renamed to roles in the framework theme |
| Glyph slate and state glyphs (`icons::state`, `marks`, `node_kind_style`, operator `[tui.theme.icons]` overrides) | `icons.rs`, used at `left_panel.rs:744,812,1544,1573,1588` | (a) glyph lookup by role; (c) the specific meanings. The renderer assumes every glyph is 1 cell wide (`"{} "` at `:1544`) while `NodeKindStyle.width` exists |
| `crate::tui::rows::*` imports | `left_panel.rs` | (c) |
| `crate::discovery::harness::display_label_for` | `rows/mod.rs:985` | (c) |
| `crate::aliases::session_display_name` | `rows/mod.rs:463` | (c) |
| `crate::viewer::search::*`, `crate::content_search::PaneRef` inside row types | `rows/mod.rs:175`, `rows/search.rs` | (c); pane rows carry an app type |
| `widgets::controls::VIEW_OPTIONS` for the tab strip | `left_panel.rs:227` | (a) labels + active index as arguments |
| `crate::invocation::invoked_name()` and `filter::render_filter_chips` in empty-state copy | `left_panel.rs:390,384` | (b) |
| `SessionsGrouping::None` special-casing | `left_panel.rs:521,943` | (b); also dead (section 4.1) |
| Domain lookups in the renderer: `pinned_nodes()`, `snapshot().presence(node)`, `row_is_frozen` -> `actions::subject_of_row` | `left_panel.rs:627,963-970,1080-1083` | (b) precompute into row/ctx |
| `format_recency`/`recency_bucket` thresholds (60s/1h/24h; 5m/1h/1d) hard-coded | `rows/mod.rs:999,1130` | (a) injectable thresholds |
| Literal domain strings in the renderer: `"was in "`, `"(mux stopped)"`, `" no agent "`, `"parent:"`, `"turn "`, `"line "`, `"N hits"`, `"(cwd)"`, `"Ungrouped"`, `"< older  > newer"`, 12 empty-state messages | various | (b)/(c) |
| `unicode-width` used directly; ratatui `Paragraph`, `Block`, `Scrollbar`, `Line`, `Span`, `span!` macro used directly (no shims) | `ui.rs:27-36` | direct use of ratatui is fine for a ratatui-based framework |

External crates in this slice: `ratatui 0.30` (core widgets only, `macros`
feature for `span!`), `unicode-width 0.2`. No `tui-tree-widget`, `ratatui-cheese`,
or `rat-*` involvement in the left pane. `tui-tree-widget` was considered for the
explorer and deferred (CSP-435); rat-widget was a no-go (CSP-433); neither
touches this pane.

## 8. Look and feel specifics

- Frame: `Borders::ALL`, default (plain) border type and colour; the title is the
  only carrier of focus: `" " + "► "|"  " (panel_focus_accent) + labels`.
- Tab strip title (`left_panel.rs:222-244`): view labels joined with
  `" · "` in `secondary_text`; active label `panel_focus_accent` + BOLD; six views
  from `VIEW_OPTIONS`. Right pane reuses `focus_marker_span` (`right_panel.rs:216`).
- Disclosure `▶`/`▼` (cyan), kind glyphs from the ADR 0073 slate, pin marker `◉`
  (running) / `◎` (stopped, ghost colour), ghost = DarkGray italic, secondary text
  = DarkGray (a concrete colour because DIM is unreliable), `placeholder` = DIM,
  chips REVERSED+BOLD with 10-cell footprint, age tint 4 buckets.
- Scrollbar: 1-column right gutter, default ratatui symbols (`▲ █ ║ ▼`), shown only
  on overflow, 1-row margin top and bottom, no gutter reserved otherwise.
- Selection: full-width reverse video when focused, bold only when not. Because
  chips are already REVERSED, patching REVERSED onto them is a no-op, so on the
  selected row a chip keeps its normal look while the rest inverts (**code-read**,
  not visually confirmed).
- Hints/footers: none inside the pane except the Pinned banner and Search header;
  keys are advertised in the global status bar.
- Glyph rule (documented in `docs/tui-columns.md:78`): only glyphs that DejaVu Sans
  Mono draws at one cell; `icons_tests.rs` measures them.

## 9. Keys (left focus) and deviations from sibling lists

Documented in section 2.3. Deviations from the other cursor lists in the app:
the tree (and the explorer, `app/explorer_nav.rs:116`) clamps at the ends via
`clamp_step`, while the Picker (`widgets/picker.rs:149,153`) and the quick-select
list (`widgets/search.rs:623`) wrap via `wrap_step`; the tree alone has
expand/collapse keys and drives paging from a runtime-computed height rather than
a measured one. Selection highlight is implemented independently in at
least 6 places (`REVERSED` at `widgets/controls.rs:1137`, `widgets/picker.rs:254`,
`widgets/search.rs:692,925,1308`, `ui/right_panel.rs:451,610,680,699`, the tree
at `left_panel.rs:1086`).

## 10. Tests

| Kind | Where | What it pins |
| --- | --- | --- |
| Pure view-model tests | `rows/*_tests.rs` (136) | builder output: grouping, ordering, filters, ids, collapse of single-worktree level, launch-context marking, title disambiguation, pin markers |
| Renderer unit tests | `ui_tests.rs` (~50 of 169): group alignment `:603-760`, badge `:1641`, glyph spans `:1680-1900`, session/mux/PR row cells `:1895-2190, 4874-5020`, ghost/hand-off `:3815-3945, 5585-5740`, scroll `:2963, 3281-3570`, Pinned snapshots `:5098-5250` | cell content, truncation (`:1998,2031,2079`), scrollbar presence/thumb, alignment |
| Consistency (cross-view) | `consistency_tests/rendering.rs` (13), `state.rs` (6), `styling.rs` (4), `docs.rs` (5) | one grid across live and ghost muxes, aligned Pinned columns, one chip style, one pin pair, one age per entity, ghost style, glyph/word/view/row-kind coverage of `docs/tui-columns.md`, regroup scrolls to top, "never says gone" |
| Doc frames | `tests/tui_columns_doc.rs` (renders `conspectus tui --snapshot` for every `<!-- frame -->` in `docs/tui-columns.md`; `just tui-columns` regenerates) | left pane at fixed width/time |
| insta snapshots | `src/tui/snapshots/*.snap` (19: Pinned at 70/110/160 wide, History, PR states, `was in` chip) | whole-frame text |
| Layout/parity/freshness | `consistency_tests/{layout,parity,freshness}.rs` | **not left-pane tests**: `layout.rs` asserts modal frames stay in bounds at 5 widths x 2 heights, selection moves no text, no clipped words; `parity.rs` asserts Enter/`v`/`p` mean the same on the same entity across `ALL_VIEWS` (it iterates `app.tree().rows`, so it depends on `RowId`/`pin_nodes`); `freshness.rs` asserts every refreshing `Effect` forces a live rescan. They belong to layer 2/3 inventories, not here |

Techniques worth keeping in the framework harness: token-column scraping
(`row_grid`, `tokens`, `rendering.rs:11-57`), "probe a cell's style" helpers
(`styling.rs:12`, `rendering.rs:125`), and the exhaustive-match-or-fail-to-compile
pattern for tying a doc to enums (`docs.rs:129,144`). `is_age` is copied in
`rendering.rs:31` and `state.rs:27`. Gaps: no test for `forks.rs`, for the
unclamped scroll offset, for PageUp/PageDown step vs viewport, or for narrow
widths of Session, PR, Hit, Pane rows (only Pinned has 70/110/160 snapshots).

## 11. Framework verdicts

| Candidate base control | Verdict | Merge with / notes |
| --- | --- | --- |
| `TreeState<Id>` (selection, expansion, scroll, viewport, nav, retention) | split into generic core + Conspectus adapter | absorbs `adjust_left_scroll`, `adjust_explorer_scroll`, the 3 expand/collapse scans, and the retention block duplicated in `set_data`/`set_tree`; the adapter keeps `recompute_detail`, `status_message`, `ViewStateSlot` persistence |
| `TreeView<R>` widget (pane chrome slots + body + scrollbar) | split | slots: title (tab strip), header line, footer banner, empty text |
| `ColumnSchema` + cell types | new, derived from the existing renderers (extract after refactor) | would absorb `GroupAlign`/`PrAlign`/`MixedAlign`, the `MixedGrid::Mux` workaround, all literal widths and gutters |
| `RowPrefix` (indent/disclosure/kind glyph/marker) | extract after refactor | |
| `GroupHeader` row pattern | extract after refactor | needs structured cells instead of the two-space-encoded `display_path` |
| `AgeCell` | extract as-is | thresholds/theme injected; Hit/Pane/PaneHit adopt it |
| `Chip` | extract after refactor | `widgets/badge.rs` + the plain ` no agent ` span + the dim `N hits`/`was in` chips |
| `StatusGlyph` / vocabulary table | split | the doc-vs-code vocabulary test mechanism is the reusable part |
| `RowState` styling (selected, ghost, pending) | extract as-is | |
| `SnippetLine` (crop + highlight) | extract as-is | merge `append_snippet` with `widgets/search.rs:1268-1282` |
| `text` helpers | extract as-is | collapse into one module (section 12 F) |
| `ScrollbarGutter` | extract as-is | merge `widgets/help.rs:262-286`; used today by left, explorer, transcript preview, pane preview |
| `TabStripTitle` | extract as-is | |
| `cursor` math | extract as-is | |
| Row builders, `RowKind`, `RowId` variants, empty-state copy, hit/pane schemas | stays in Conspectus | |
| Hidden Union/Forks views | stay in Conspectus; candidates for retirement before extraction (ignored groupings, no tests for Forks) | see open questions |

## 12. Duplications and inconsistencies

Counts are exact for the files named; (code-read) items were not confirmed at
runtime.

**Duplications**

| ID | What | Count and locations |
| --- | --- | --- |
| D1 | Group-header `Row` construction (`Row{id,depth,expandable:true,kind:Group(GroupRow{display_path,primary_node,is_launch_context:false})}`) | 12 sites: `sessions.rs:774,788,818,850`; `mux.rs:360,442,556`; `pinned.rs:96`; `history.rs:150`; `search.rs:168,258`; `prs.rs:325`. 26 `Row {` constructions in `rows/` overall |
| D2 | Bucket emission skeleton (group items into labelled buckets, header then members) | 4: `pinned.rs:73-106`, `history.rs:127-160`, `search.rs:153-178`, `search.rs:239-270`; two key strategies (`BTreeMap<(u8,String)>` vs linear-find `Vec`) |
| D3 | `OTHER_BUCKET = "Other"` | 3 constants: `pinned.rs:31`, `history.rs:34`, `search.rs:38`; `kind_bucket_label` x2 (`pinned.rs:111`, `history.rs:165`) |
| D4 | Short-id computation (node ids -> FNV -> `unique_prefix_len` -> prefix map) | 4 inline copies: `mux.rs:87-100`, `prs.rs:122-133`, `forks.rs:54-65`, `union.rs:65-74`, plus `ShortIds` + a private `unique_prefix_len` in `sessions.rs:1069-1133` that duplicates `output/render.rs:202` |
| D5 | `AgentSessionRow` hand-filled (15 fields) | 3: `rows/mod.rs:1075`, `mux.rs:874` (`agent_row` near-twin), `sessions.rs:937`. `MuxSessionRow` (17 fields) x3: `mux.rs:150`, `mux.rs:747`, `union.rs:108`. `collect_agents` x3 (`prs.rs:370`, `forks.rs:139`, inline in `union.rs:151`) |
| D6 | Mux column grid | twice: `render_mux_session_spans` (`left_panel.rs:1212-1277`) and the `MixedGrid::Mux` branch of `render_mixed_spans` (`:792-827`); state-glyph mapping to `state::{LINKED,UNLINKED,UNKNOWN,STOPPED}` three times (`:1680`, `:1262-1272`, `:811-818`) |
| D7 | PR (state, draft) -> word/glyph/hue | 6 sites in 4 files: `prs.rs:78 StateBucket::of`, `pinned.rs:174-189`, `history.rs:263-277`, `left_panel.rs:1552 forge_pr_glyph_span`, `:1582 pr_state_span`, `:910 mixed_glyph_span` + `:835-847` status hue |
| D8 | Snippet before/match/after span triple | 2: `left_panel.rs:1499-1523`, `widgets/search.rs:1268-1282` |
| D9 | Ellipsis/pad helper family | TUI: `ui/text.rs` has `truncate_to_width` (a pure alias of `truncate_to_width_strict`, `:6-8`), `_strict`, `_middle`, `_no_marker`, `pad_to_width`, `fit_spans_to_width`; elsewhere `widgets/popup_frame.rs:110 ellipsize` and its `fit`, `widgets/form.rs:573 truncate_chars`, `explorer.rs:1856 truncate` (char-count based), `widgets/snippet.rs head`, inline cut in `widgets/badge.rs:62` and `left_panel.rs:271`, `ui/header.rs:416 truncate_chip_list`, `preview_wrap.rs:209`; CLI: `output/render.rs:1135 truncate_to_width` is semantically identical to `truncate_to_width_strict`. `popup_frame::ellipsize(…, 0)` returns `…` while `truncate_to_width_strict(…, 0)` returns empty |
| D10 | Scrollbar | `render_vertical_scrollbar` has 3 external callers (`right_panel.rs:153`, `preview_pane.rs:65,83`) and `widgets/help.rs:273-285` re-implements it inline |
| D11 | Scroll-into-view | `adjust_left_scroll` (`app.rs:1280`) and `adjust_explorer_scroll` (`app.rs:1320`) |
| D12 | Selection highlight (REVERSED row) | >= 6 widgets (section 9) |
| D13 | Pin marker | two mechanisms: render-time lookup for native rows vs carried on `MixedRow.marker`; `AgentSessionRow.pin_id`/`MuxSessionRow.pin_id` exist but the renderer does not use them for the marker |
| D14 | Two-space gutter literal | ~40 inline sites; no constant |
| D15 | Tree selection retention | `tree.rs:27-54` vs `96-112` (same logic, one extra branch); expand/collapse/toggle each re-scan rows (`tree.rs:214-249`) |
| D16 | Column engines | TUI: ad hoc; CLI: `ColumnSpec` registry + `fit_to_width` greedy shrink (`output/render.rs:234,1084`) |
| D17 | `is_age` test helper | `consistency_tests/rendering.rs:31`, `state.rs:27` |

**Inconsistencies (behavior or contract)**

| ID | Finding | Evidence |
| --- | --- | --- |
| I1 | Three rules for "is this PR a draft": exact `open`+draft (`prs.rs:81`), draft when state is empty or open (`pinned.rs:176`, `history.rs:266`, `left_panel.rs:1584`), and draft regardless of state for the kind glyph (`left_panel.rs:1554`). A closed draft PR can therefore show a draft-yellow `⇄` next to a red `⊗` | listed sites |
| I2 | Narrow-width policy differs per row kind: Mux progressively drops and ends with `fit_spans_to_width`; Session, Pr, Hit, Pane clip hard at the pane edge with no `…` on the fixed cells; Mixed drops chip then content | section 4.3 |
| I3 | "Room remains" thresholds are `>2`, `>4`, and `>8` with no shared rule | section 4.7 |
| I4 | Age cell rendering: `age_span` (tinted, truncated to 4) for 4 row kinds; raw `{:>4}` (untinted, untruncated) for Hit, Pane, PaneHit | `left_panel.rs:761` vs `1409,1449,1480` |
| I5 | Group label alignment is per visible rows while PR and Mixed alignment is per whole tree; doc says tree | section 4.4 |
| I6 | Glyph-less buckets start their label 2 cells left of glyph-bearing siblings | `docs/tui-columns.md` frames |
| I7 | PageUp/PageDown step is `terminal_height - 2`, not the tree viewport (**code-read**) | `runtime.rs:146`, `keymap.rs:239` |
| I8 | Scroll offset not clamped to content length; no test (**code-read**) | `app.rs:1280-1294` |
| I9 | Dead row data: `GroupSummary.attached`/`.unmuxed` written, never read; `RowTree.view` (and `ViewLabel`) never read outside tests; `PaneRow.deduplicated` and `PrRow.url` set by builders and read by no non-builder code; `MuxIndicator::Ambiguous{candidate_count}` is read by no code outside `rows/` | `left_panel.rs:451-456`, `rows/mod.rs:100-118,185,518,589`, `rows/search.rs:454`, `rows/prs.rs:217` |
| I10 | Stale comments: `MuxIndicator` doc says the chip renders `◉`/`◯` (`rows/mod.rs:507-509`), `ui.rs` header says the same; current glyphs are `⊸`/`∘`. `docs/tui-columns.md` still lists aider as a harness colour while the aider removal workstream is open | |
| I11 | Two grouping enums (`UnionGrouping`, `ForksGrouping`) are selectable but ignored by their builders | `tui/mod.rs:592-604` vs `union.rs`/`forks.rs` |
| I12 | `pinned.rs:537` reads `$HOME` directly instead of the builder's `home` input | |
| I13 | `AgentSessionRow.short_id` is computed by five different mechanisms but the pane shows the harness key instead; it is used by quick-select (`tui/search.rs:78,144`) | |

## 13. Open questions for the operator

1. **Which draft rule wins (I1)?** A closed or merged draft PR currently disagrees with
   itself between the kind glyph and the state glyph. Pick one rule for the framework's
   `StatusGlyph` pair.
2. **Narrow widths (I2, I3)**: should every row kind share one drop-priority policy
   (cells carry a priority; the lowest drops first; truncated cells always end in `…`),
   at the cost of changing Session/PR/Hit/Pane rows at small widths?
3. **Group label alignment scope (I5)**: tree-wide (as the doc says) or visible-only
   (as the code does)?
4. **Glyph-less bucket headers (I6)**: pad them so labels share a start column, or keep
   them 2 cells left?
5. **Page step and scroll clamp (I7, I8)**: treat as bugs to fix in the extraction story
   (measured viewport; clamp offset to content), or preserve?
6. **Tree key set**: add "`h` on a collapsed/leaf row jumps to the parent", expand-all,
   collapse-all, type-ahead, mouse? None exist today; the framework would have to
   decide whether to ship them.
7. **Hidden Union and Forks views**: carry them (with ignored groupings and no tests) or
   retire them before the extraction so the framework is not shaped by them?
8. **Cell strings**: `GroupRow.display_path` encodes columns as text. OK to change the
   row model to structured cells (touches every group builder and `compact_path_*`)?
9. **Theme role names**: rename domain-named tokens (`link_id`, `mux_attached`, `mux_unmuxed`,
   `cwd_mark`) to roles in the framework theme, keeping the old config keys as aliases?
10. **Where do the CLI column substrate (`output/render.rs`) and the TUI column schema meet**:
    share one `ColumnSpec`/width-fitting core, or keep the CLI separate?
11. **Selected-row chips**: is the chip staying un-inverted on the selected row intentional
    (needs a visual check; this survey could not run the TUI)?
12. **Persistence**: should expansion and scroll survive restarts (currently in-memory only)?
