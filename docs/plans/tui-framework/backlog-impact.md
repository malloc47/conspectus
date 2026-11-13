# Backlog Impact Of The TUI Framework Extraction

Story CSP-766, the close-out of milestone m-25, reviewed on 2026-10-31 at
commit `5fbd6154`, with the framework in its final shape (ADRs 0150 to 0200;
the runtime evaluation's outcome, ADR 0200, is folded in). It lists every
story open when the review started: 128, of which 127 were To Do
and 1 In Progress, from every milestone and from none. It
classifies each once and records what was done about it through the
`backlog` CLI. The planning-time list of overlaps is
[control-set.md](control-set.md) section 11; the gaps are
[gap-analysis.md](gap-analysis.md) and the framework's
[roadmap](../../../crates/tuikit/docs/roadmap.md).

## Counts

| Classification | Stories |
| --- | --- |
| Unaffected | 74 |
| Needs a path or terminology update | 38 |
| Needs re-scoping | 13 |
| Newly blocked or unblocked | 1 |
| Superseded or obsolete | 2 |
| Total | 128 |

One story was created: CSP-803 (the TUI ignores `NO_COLOR` and `--color
never`, and nothing downgrades colors to the terminal's depth). Two were
closed: CSP-435 and CSP-455.01. No story was deleted.

## How The Review Was Done

Every open story's full text (description, criteria, notes, references,
documentation) was dumped and scanned for the names the milestone removed or
moved and for TUI words, to order the reading; every story was then read.
Each removed or moved name was checked against the tree, not taken from the
plan:

- **Gone:** `src/tui/widgets/{form,multi_select,row_menu,mux_menu,popup_frame,toast,input}.rs`,
  `src/tui/cursor.rs`, `src/tui/detail.rs`, `src/tui/ui/detail_header.rs`,
  `src/tui/ui/text.rs` (its domain half is `ui/compact.rs`),
  `path_omnibox.rs`, `LaunchSpecFormState`, `TextInputState`, `FindPrompt`,
  `ViewerState`, `compact_path_label`, `chip_divider_line`,
  `GroupAlign`/`PrAlign`/`MixedAlign`, `OverlayOutcome`, the Union and Forks
  views (`RowKind::Fork`), and the `ratatui-cheese`, `ratatui-comfy-toaster`,
  and `tui-popup` dependencies.
- **Still there, reshaped:** `src/tui/widgets/picker.rs` and `clean_up.rs`
  are thin adapters over `tuikit::palette` and `tuikit::checklist`;
  `widgets/controls.rs` keeps `ControlsContext` (built per call, no longer a
  clone) over `tuikit::settings_panel`; `Action::ModalKey` is the one overlay
  key wrapper left (CSP-802 would fold the rest of `Action` into `Msg`);
  `render_to_buffer` and `buffer_to_string` are still in `src/tui/ui.rs`.

The classes:

- **Unaffected:** what the story names still exists and still reads true, or
  the story is outside the TUI. A short reason is given.
- **Needs a path or terminology update:** the story's goal stands, but its
  text names a moved or deleted module, a type that changed, or an approach
  that now has a framework home. A dated "TUI framework update" paragraph was
  appended to its notes, naming the new modules, ADRs, and seams; the
  author's text was left as it was.
- **Needs re-scoping:** part of the scope was delivered, became obsolete, or
  now goes another way. A dated scope note says what is left; titles,
  descriptions, and dependencies changed where the old ones would mislead.
- **Newly blocked or unblocked:** a dependency appeared or went away.
- **Superseded or obsolete:** the milestone's work clearly delivers or
  rules out the story. Closed (Done) with a final summary naming what
  replaced it. A story whose replacement is still open stays open with a note.

## Known Overlaps And Their Outcomes

Every overlap the story's scope names, and section 11's list:

| Story | Outcome |
| --- | --- |
| CSP-342 (lift `src/viewer` into a crate) | Re-scoped; description rewritten with the original kept. The workspace exists; the generic pager moved to the framework and `src/viewer` keeps only the transcript, so the crate carries `tuikit` as a dependency and no reader of its own; a standalone binary writes its own small loop because the runtime stays in Conspectus (ADR 0200). Still unscheduled: no consumer. |
| CSP-390 (palette and minibuffer ADR) | Re-scoped: matching, registry shape, static registration, the remapping seam, and placement are settled by ADRs 0158, 0174, and 0192; the note lists what the ADR still decides and corrects its citation of ADR 0033. Kept open. |
| CSP-391 (action registry) | Re-scoped: the scoped keymap (CSP-760) is most of the registry; what is left is the palette's row source and its tests. Kept open. |
| CSP-392 (command palette) | Path update: a `tuikit::palette` instance over the keymap's bindings, through the seam on `palette.md`. Kept open. |
| CSP-393 (minibuffer) | Path update: a one-row `TextPrompt` host, using the prompt's minibuffer, completion, and live-validation seams. Kept open. |
| CSP-394 (migrate prompts to the minibuffer) | Re-scoped: rename is a `TextPrompt`, `/` is quick select, `v` opens a transcript; the dependency on CSP-193 was removed. Kept open. |
| CSP-430 (`tui-textarea`) | Re-scoped and retitled to `ratatui-textarea` (the old crate does not resolve against ratatui 0.30); a multi-line area is a framework control beside `LineEdit`, needing the framework's dependency record. Still deferred until a surface needs it. |
| CSP-432 (spinners) | Re-scoped and retitled: the framework's spinner (`chip::spinner_frame`, `StatusChip::busy`) already covers the in-flight chips, so no new crate is needed, and what is left is animating the static loading placeholders. |
| CSP-434 (`ratatui-explorer` picker) | Re-scoped: `ratatui-explorer` reads the file system itself, while the framework reads none and `PathField` reads through a host `PathSource`; a browse mode reads the same source (roadmap Tier 3). |
| CSP-455 (cwd omnibox) | Path update: the omnibox is `tuikit::path_field::PathField`; the ranked list it keeps open is the `candidates()` seam; the pin create form is gone. Kept open. |
| CSP-455.01 (row and edit focus in pin create) | Closed as superseded by ADR 0168 (house rule 6): one ring, `Enter` submits from any field; its form is gone. |
| CSP-435 (`tui-tree-widget`) | Closed as won't do: the explorer is a flat list of zones with a drill stack (`tuikit::explorer`, ADR 0188) and the tree is `tuikit::tree` (ADR 0182). |
| CSP-269 (theme presets) | Path update: a preset is a table applied before the user's own, on `tuikit::theme`; that settles its precedence question. Kept open (Tier 1). |
| CSP-270 (density modes) | Re-scoped: density is a column-schema choice plus the roadmap's `density` token; the section-pane part is obsolete. Kept open. |
| CSP-191 (density modes) | Path update: CSP-270 carries its scope; left open (CSP-270 has not delivered it), to close with CSP-270. |
| CSP-319 (responsive layout) | Re-scoped: stacking (`SplitPane`, ADR 0189) and narrow drops (ADR 0183, 0190) have homes, and union and forks are gone; what is left is widen-on-focus (the resizable split), hide-and-tab, and one breakpoint document. |
| CSP-190 (contextual status bar) | Already Done before the review (a dependency of CSP-319); nothing to do. |
| CSP-180 (empty, loading, error frames) | Path update: `EmptyState` (ADR 0190), `StatusChip`, and style-keeping goldens. Kept open. |
| CSP-183 (snapshot coverage) | Path update: `tuikit::testkit` and the conformance suite already cover the overlays; the view-level matrix is what is left. Kept open. |
| CSP-188 (label compression) | Path update: headers carry `label` and `detail` cells cut by the schema's give-way rule and `tuikit::text`; `ui/text.rs` is gone. Kept open. |
| CSP-616.09 (index-backed content search) | Path update: content search is a `tuikit::palette` with a slow provider; "which source answered" fits its status row. |
| CSP-687 (reorder pin slots) | Path update: menu entries as data (ADR 0171), keymap accelerators with `available_when` (ADR 0192), declared lifecycle. |
| CSP-702 (remember expanded groups) | Path update: the set is `TreeState<RowId>::expanded()` in `ViewStateSlot.tree`; dropping stale ids is the host's job. |
| m-24: CSP-699 (aider ADR) | Path update: `harness_aider` is an `ALIASES` row, and dropping it yields the theme's unknown-key warning; keep `aider` in the independence guard's word list. The plan wanted m-24 to land before the theme work; it did not, so the theme adapter keeps aider's two rows until it does (operator review, CSP-721). |
| m-24: CSP-700 (delete aider code) | Path update: its file list names `widgets/badge.rs`, where aider no longer is; the theme rows and `badge_tests.rs` are where it is. |
| m-24: CSP-701 (aider docs) | Unaffected: the docs it names still hold the text, and the framework's docs cannot name aider. |

## Gap Analysis Tier 1 And Tier 2

Each entry of [gap-analysis.md](gap-analysis.md) section 3, mapped. The
framework [roadmap](../../../crates/tuikit/docs/roadmap.md) marks which
Tier 1 items the milestone finished.

| Tier | Entry | Maps to | Notes |
| --- | --- | --- | --- |
| 1 | Command palette as a `Palette` instance | CSP-392, with CSP-390 and CSP-391 | Open. The palette and the keymap are ready; the seam is on `palette.md`. |
| 1 | Tri-state action availability | Done in the framework (show and dim: `keys::with_availability`, `available_when`); CSP-776 for the host's overlays | Hiding is a filter over the same copy. |
| 1 | A spinner | Done in the framework (`chip::spinner_frame`, `StatusChip::busy`, CSP-725); CSP-432 retargeted to the loading placeholders | No `throbber-widgets-tui`. |
| 1 | Theme presets | CSP-269 | Open; the theme needs no change for it. |
| 1 | `NO_COLOR` and color-depth downgrade | New: CSP-803 | `TuiOptions::color` is resolved and never read, so the TUI draws colors under `NO_COLOR` and `--color never` today. |
| 1 | Bracketed paste | Done (CSP-715, CSP-733; ADR 0165) | |
| 1 | Select all and none in `Checklist` | Done (CSP-741; ADR 0173) | |
| 1 | Style-aware snapshot goldens | Done (CSP-732; `testkit::golden`, ADR 0164) | |
| 2 | Multi-line text area via `ratatui-textarea` | CSP-430 (retargeted) | Deferred until a surface needs it. |
| 2 | Validated input | Not planned | Refusals with reasons already exist (house rule 7: a prompt's `on_confirm`, a form field's check); a check on each edit is the prompt's "live validation" seam. No open story needs it. |
| 2 | A general completion source | CSP-393 (the minibuffer's `Tab` completion is the second kind of completing field), with CSP-455 for paths | Generalizes `PathField`'s `PathSource`. |
| 2 | A shared column schema with the CLI tables | CSP-179 (re-scoped) | The framework's plain-text tables seam on `columns.md`. |
| 2 | A progress bar | Not planned | Only content search reports progress (`palette::Progress`, in words); revisit with a second long operation, most likely CSP-686's combined rebuild. |
| 2 | A resizable split | CSP-319 (re-scoped) | A share for `SplitPane`'s first pane. |
| 2 | Light and dark adaptation | Not planned | Picks between presets, so it waits for CSP-269; the theme page's variant seam and mode 2031 detection. Noted on CSP-269. |
| 2 | Mouse hit testing | CSP-344 (re-scoped as the first mouse story), then CSP-355 | Mouse capture is not enabled anywhere today. |
| 2 | User key remapping | Not planned | No story or request needs it; the seam is the keymap's stable ids and its build-time overlap check (`keys.md`), and CSP-390's note records how it would plug in. |
| 2 | A density token | CSP-270 (with CSP-191) | |

The roadmap's Tier 2 also lists a declared focus cycle (from the original
control set, not the gap analysis): not planned; no story needs it.

## Open Follow-Ups Left By The Milestone

With CSP-766 closed, every m-25 story is Done. These stories, filed while the
milestone ran or by this review, are still open (none has a milestone):

| Task | Story | Priority | Filed by | Note |
| --- | --- | --- | --- | --- |
| CSP-776 | Dim overlay actions that cannot act now | Low | CSP-728 | The host's half of the Tier 1 availability item. |
| CSP-802 | Fold the keymap's `Action` layer into `Msg` so every loop routes a key one way | Low | CSP-765 | Operator review asks whether to keep it, close it, or move it to Draft until a runtime extraction is scheduled. |
| CSP-770 | Stop fake-binary tests from racing on a busy executable | Low | a test flake seen during the milestone | Unrelated to the framework; costs a rerun of the full suite. |
| CSP-803 | Draw the TUI without color when color is off, and within the terminal's color depth | Medium | CSP-766 | The Tier 1 gap with no story. |

Also for the operator: CSP-193 and CSP-098 may already be delivered by
earlier work (quick select, filter chips, and the Search view; the tables'
`activity` column), and CSP-191 should close when CSP-270 lands. Each has a
note; none was closed, because the milestone's work did not deliver it.
[operator-review.md](operator-review.md) holds this review's judgment calls
beside every other story's.

## Every Open Story

Status is the status and milestone when the review started. CSP-803, which
this review created, is not a row: it is in the gap map and the follow-ups.

| Task | Title | Status | Classification | Reason and evidence | Action taken |
| --- | --- | --- | --- | --- | --- |
| CSP-093 | Add a human-readable graph projection | To Do (m-11) | Unaffected | A text graph projection for the CLI; not TUI. | none |
| CSP-095 | Add filter flags for the graph and session commands | To Do (m-11) | Unaffected | CLI filter flags; not TUI. | none |
| CSP-097 | Improve discovery diagnostics for missing providers | To Do (m-11) | Unaffected | Discovery diagnostics in the graph and CLI; not TUI. | none |
| CSP-098 | Surface activity/recency in the session tables | To Do (m-11) | Unaffected | A CLI table column; not TUI. The relative-age helper it asks for exists (`format_relative_age` on `tuikit::age`), and the agent and mux tables already have an `activity` column, so it may be done (flagged for the operator). | none |
| CSP-099 | Implement the bootstrap-roots flow described in the design | To Do (m-11) | Unaffected | A bootstrap command; not TUI. | none |
| CSP-100 | Cache layer for forge metadata, tmux, and harness scans | To Do (m-11) | Unaffected | A cache layer; not TUI. | none |
| CSP-101 | Batch `gh pr list` across repos sharing a host | To Do (m-11) | Unaffected | Forge call batching; not TUI. | none |
| CSP-102 | Add a graph-diff command | To Do (m-11) | Unaffected | A graph diff command; not TUI. | none |
| CSP-104 | Complete `Cargo.toml` metadata for crates.io | To Do (m-11) | Newly blocked or unblocked | Newly blocked: `cargo publish --dry-run` cannot pass while the package depends on the unpublished path crate `tuikit` (`publish = false`, ADR 0150). | note appended; now depends on CSP-541 |
| CSP-105 | Pin and verify MSRV | To Do (m-11) | Needs a path or terminology update | `rust-version` belongs in `[workspace.package]` beside `edition` and `license` so both members inherit it; the export carries it (ADR 0199). | dated note appended |
| CSP-106 | Define the release process | To Do (m-11) | Unaffected | Release process; inherits the crates.io question through CSP-104. | none |
| CSP-108 | Settle `ForgePr` identity and branch-association keys | To Do (m-11) | Unaffected | Forge PR identity design; not TUI. | none |
| CSP-109 | Settle declared-link conflict and override semantics | To Do (m-11) | Unaffected | Declared-link conflict semantics; not TUI. | none |
| CSP-110 | Document the graph invariants and snapshot canonicalization contract | To Do (m-11) | Unaffected | Graph invariants doc; not TUI. | none |
| CSP-113 | Add harness adapters for jujutsu and sapling sessions if and when a user uses them with a supported harness | To Do (m-11) | Unaffected | Future harness adapters; not TUI. | none |
| CSP-114 | Add a first-run walkthrough | To Do (m-11) | Unaffected | A first-run walkthrough of the CLI; not TUI. | none |
| CSP-116 | Add library-integration examples beyond the api doctest | To Do (m-11) | Unaffected | Examples for `conspectus::api`; building a TUI on the framework is `crates/tuikit/docs/guide.md`'s subject. | none |
| CSP-124 | Read agent-deck profile state from `state.db` | To Do (m-11) | Unaffected | agent-deck state reader; not TUI. | none |
| CSP-132 | Add a dmux orchestrator adapter (audit-gated) | To Do (m-11) | Unaffected | dmux adapter (audit-gated); not TUI. | none |
| CSP-133 | Add a workmux orchestrator adapter (audit-gated, narrowed scope) | To Do (m-11) | Unaffected | workmux adapter (audit-gated); not TUI. | none |
| CSP-134 | Add an agent-of-empires orchestrator adapter (audit-gated) | To Do (m-11) | Unaffected | agent-of-empires adapter (audit-gated); not TUI. | none |
| CSP-171 | Add PR, fork, and transcript/history detail enrichments | To Do (m-13) | Unaffected | Parent with no text of its own; its open subtask CSP-171.02 is re-scoped. | none |
| CSP-171.02 | Fork right-panel enrichment (lineage, context, children) | To Do (m-13) | Needs re-scoping | Fork rows are gone (`RowKind::Fork`, the Forks view: CSP-711; no view lists a fork: ADR 0198); a fork shows in the right pane after a drill, so the enrichment targets the explorer's Fork node. | scope note appended |
| CSP-172 | Document and verify the v1 TUI workflow | To Do (m-13) | Needs a path or terminology update | Keys are listed by the generated `?` page (keymap, ADR 0192) and the overlays' generated footers; operations.md gained "What changed in the TUI" (CSP-764). | dated note appended |
| CSP-175 | Inline mux-picker for ambiguous `LinkedToMux` candidates | To Do (m-13) | Needs a path or terminology update | The menu home is a Mux menu entry (`ActionMenu`, ADR 0171) and a pick-one list is `widgets/picker.rs` (a `Palette`, ADR 0174), whose keys are not the old search overlay's j/k. | dated note appended |
| CSP-179 | Align `conspectus table sessions` columns with the TUI sessions row tree once view-models converge | To Do (m-13) | Needs re-scoping | The TUI's rows now come from `ColumnSchema` (ADR 0183 to 0185), so the shared surface is the schema (the framework's plain-text tables seam), not a view model. | scope note appended |
| CSP-180 | Fill out the TUI empty/loading/error frame matrix | To Do (m-13) | Needs a path or terminology update | Empty and waiting slots go through `EmptyState` (ADR 0190), chips are `StatusChip`, and style-keeping goldens are `testkit::golden` (ADR 0164); its buffer helpers remain in `src/tui/ui.rs`. | dated note appended |
| CSP-183 | Expand TUI buffer-snapshot test coverage | To Do (m-13) | Needs a path or terminology update | `tuikit::testkit` (ADR 0164) adds the frame driver, style-keeping goldens, and the conformance suite, which already holds the search palettes and the help page; the view-level matrix remains. | dated note appended |
| CSP-185 | Throttle and freshen mux pane-capture previews | To Do (m-13) | Needs a path or terminology update | The Preview's divider is `tuikit::frame::SectionRule` with no summary today; a freshness label is its summary worded by `tuikit::age`; the old `captured` field went with CSP-710. | dated note appended |
| CSP-188 | Compress project, path, and mux display labels | To Do (m-13) | Needs a path or terminology update | Headers carry `label` and `detail` cells (ADR 0184) cut by the column schema's give-way rule (ADR 0183) and `tuikit::text` (ADR 0155); `src/tui/ui/text.rs` is gone, mux helpers are in `ui/compact.rs`. | dated note appended |
| CSP-191 | Add sessions-tree density modes | To Do (m-13) | Needs a path or terminology update | CSP-270 carries this scope; rows come from a `ColumnSchema` (ADR 0183) and the roadmap keeps a `density` theme token. Left open until CSP-270 lands (it does not yet deliver it). | dated note appended |
| CSP-192 | Crop and annotate tmux previews for recognition | To Do (m-13) | Needs a path or terminology update | Same as CSP-185: the Preview's `SectionRule` carries no summary today, so the "slice landed" claim needs re-checking. | dated note appended |
| CSP-193 | Add visible search/filter workflow for large session worlds | To Do (m-13) | Needs a path or terminology update | `/` is quick select on `tuikit::palette` (ADR 0174, 0175, 0196, 0198), content search the second palette, and filter chips show active filters; it may already be delivered (flagged). | dated note appended |
| CSP-196 | Auto-broaden TUI scan roots to the cwd's "code dir" ancestor when neither CLI nor config specifies one | To Do (m-13) | Unaffected | Scan-root resolution; not TUI drawing. | none |
| CSP-211 | Aider transcript parser for the native viewer (deferred) | To Do (m-11) | Unaffected | Aider transcript parser; CSP-701 closes it (its AC #4); the parsers in `src/viewer/parser/` did not move. | none |
| CSP-245 | ADR: provider, dependency, privacy, dispatch | To Do (m-11) | Unaffected | AI naming provider ADR; not TUI. | none |
| CSP-246 | Transcript context extractor | To Do (m-11) | Unaffected | Context comes from `src/viewer/parser/`, which the framework work left in place. | none |
| CSP-247 | CLI + TUI suggest surface | To Do (m-11) | Needs a path or terminology update | Its candidate list is the host picker (`widgets/picker.rs`, a `Palette`, ADR 0174) and an edit is the rename `TextPrompt` (ADR 0166); `s` is now content search. | dated note appended |
| CSP-248 | Optional auto-suggest hook | To Do (m-11) | Unaffected | A config-gated suggestion hook; display follows CSP-247. | none |
| CSP-267 | Descend into scan roots when looking for atelier workspaces (companion to `CSP-196`) | To Do (m-13) | Unaffected | Atelier discovery depth; not TUI. | none |
| CSP-268 | Detect session live status (running / waiting / idle / error) and surface it as a row glyph and per-status header chip | To Do (m-13) | Needs a path or terminology update | No `status_*` colors exist or are reserved; new colors are host roles in `ROLES` (ADR 0153), chips are `StatusChip`, and a row glyph is a schema cell needing a vetted glyph (ADR 0154). | dated note appended |
| CSP-269 | Ship preset theme variants on top of ADR 0032 | To Do (m-13) | Needs a path or terminology update | The theme is `tuikit::theme` with host `ROLES` and `ALIASES` (ADR 0153); a preset is a table applied before the user's own (theme.md seam), which settles its precedence question. | dated note appended |
| CSP-270 | Add sessions-tree density modes — folds `CSP-191` into the theme-aware renderer landed by the styling overhaul | To Do (m-13) | Needs re-scoping | Density is now a column-schema choice plus the roadmap's `density` token; its "section-pane treatment" is obsolete (sectioned detail deleted, CSP-710). | scope note appended |
| CSP-293 | Add an embedding import command | To Do (m-14) | Unaffected | Embedding import CLI; not TUI. | none |
| CSP-294 | Retire snapshot bridge APIs from consumer modules | To Do (m-15) | Needs a path or terminology update | `tui::detail::build_node_detail` went with the sectioned detail model (CSP-710); drop it from the candidate list. | dated note appended |
| CSP-295 | Determine how to detect subagent sessions from opencode state | To Do (m-11) | Unaffected | Subagent detection from opencode state; not TUI. | none |
| CSP-296 | Thread subagent metadata into the graph model | To Do (m-11) | Unaffected | Subagent metadata in the model; not TUI. | none |
| CSP-297 | Filter and nest subagent sessions in the TUI | To Do (m-11) | Unaffected | Row builders stay in Conspectus (`src/tui/rows/sessions.rs`); a nested session is a deeper tree row; nothing it names moved. | none |
| CSP-298 | Suppress subagent sessions from mux attachment resolution | To Do (m-11) | Unaffected | Resolver preference for parents; not TUI. | none |
| CSP-318 | First-class evidence inspector and link-promotion flow | To Do (m-13) | Needs a path or terminology update | Unresolved rows are `property_grid::LinkRow`s with no inspect intent (CSP-788 left `o` to this story); the inspector is a `Pager` content on the modal stack, promotion a `Dialog`. | dated note appended |
| CSP-318.01 | Surface resolver explanations in the TUI detail explorer | To Do (m-13) | Needs a path or terminology update | The value modal is a `Pager` content (ADR 0178) and the Preview sits under a `SectionRule` in `StackedMasterDetail` (ADR 0188); relationship rows are `LinkRow`s from the host's explorer source. | dated note appended |
| CSP-319 | TUI responsive-layout design and breakpoints | To Do (m-13) | Needs re-scoping | Stacking is `SplitPane` with `[tui] narrow_layout_threshold` (ADR 0189) and narrow drops follow one rule (ADR 0183, 0190); union and forks views are gone. Left: widen-on-focus (the resizable split seam), hide-and-tab, one breakpoint doc. | scope note appended |
| CSP-322 | Left-pane follow sync (opt-in view switching) | To Do (m-13) | Needs a path or terminology update | The drill stack is `tuikit::explorer` (ADR 0188) with the mirror host-side in `app/explorer_nav.rs`; the left pane's `Backspace` no longer pops the drill; Controls is a `SettingsPanel` (ADR 0176). | dated note appended |
| CSP-323 | Distinguish symmetric relations in the detail explorer | To Do (m-13) | Needs a path or terminology update | The cursor walks the zones the host's `ExplorerSource::zones` returns (ADR 0188); a third zone is one more `Zone`, and the name `Related` is taken by the existing rule. | dated note appended |
| CSP-329 | Emit explicit resolver decision rationale | To Do (m-16) | Unaffected | Resolver rationale in the HTML inspector and DOT; not the TUI. | none |
| CSP-332 | Config-driven viewer override | To Do (m-11) | Unaffected | External viewer configuration; the external launch path is still `src/tui/viewer.rs`, and the framework spawns nothing. | none |
| CSP-342 | (later) Extraction prep: lift `src/viewer/` into a workspace member crate | To Do (m-11) | Needs re-scoping | The workspace exists and the generic pager moved to the framework (CSP-748, ADR 0180); `src/viewer` keeps only the transcript and depends on `tuikit`, and a standalone binary needs its own loop (ADR 0200). | description rewritten (original kept) |
| CSP-344 | Mouse bindings inside the viewer | To Do (m-11) | Needs re-scoping | Mouse capture is enabled nowhere and `handle_viewer_overlay_key` is gone; the viewer is `Pager` over `Transcript` (ADR 0180). Now the first mouse story: terminal guard, routing, wheel, and click hit testing. | scope note appended |
| CSP-345 | In-viewer navigation into forks / child sessions | To Do (m-11) | Needs a path or terminology update | `ViewerState` is gone: the viewer is `Pager` over `Transcript<M>` (ADR 0180); a child transcript is a second viewer on the modal stack. | dated note appended |
| CSP-346 | Lazy / chunk-by-chunk transcript loading around compaction boundaries | To Do (m-11) | Needs a path or terminology update | `ViewerState` is gone; chunk state lives in the `Transcript<M>` content, and Markdown renders through `tuikit::richtext` (ADR 0181). | dated note appended |
| CSP-347 | ADR: usage-limit detection and scheduled continuation policy | To Do (m-11) | Unaffected | Continuation policy ADR; not TUI. | none |
| CSP-348 | Model blocked-session metadata | To Do (m-11) | Unaffected | Blocked-session metadata in the model; not TUI. | none |
| CSP-349 | Detect usage-limit tails in supported transcript parsers | To Do (m-11) | Unaffected | Parser detection of usage-limit tails; not TUI. | none |
| CSP-350 | Surface blocked-until state in CLI and TUI | To Do (m-11) | Unaffected | A TUI badge or field would be a schema cell or a property row; its words still read true. | none |
| CSP-351 | Implement explicit continue scheduling | To Do (m-11) | Unaffected | Continue scheduling; any TUI action follows the keymap and menus like every other. | none |
| CSP-352 | Document blocked-session and continue workflows | To Do (m-11) | Unaffected | Docs for the continue workflow; not TUI. | none |
| CSP-354 | Per-message selection + clipboard copy | To Do (m-11) | Needs a path or terminology update | `ViewerState` is gone; selection goes on `Transcript<M>`, copy through the OSC 52 sink (ADR 0163), and `yy` is a chord the key model lacks (ADR 0160). | dated note appended |
| CSP-355 | Per-tool expand on click | To Do (m-11) | Needs a path or terminology update | `ViewerState` is gone; per-turn overrides go on `Transcript<M>`, and `t` is one of `TRANSCRIPT_KEYS`. | dated note appended |
| CSP-357 | Investigate Claude Code Workflows process and session topology | To Do (m-11) | Unaffected | Claude Code workflow topology; not TUI. | none |
| CSP-382 | Tmux non-default socket discovery enumeration | To Do (m-11) | Unaffected | tmux socket enumeration; not TUI. | none |
| CSP-383 | Lifecycle hooks beyond `launch.argv` | To Do (m-11) | Unaffected | Pin launch hooks; not TUI. | none |
| CSP-384 | Importers from tmuxinator / tmuxp / smug configs | To Do (m-11) | Unaffected | Config importers; not TUI. | none |
| CSP-386 | Absolute tmux socket paths (`tmux -S <path>`) | To Do (m-11) | Unaffected | Absolute tmux socket paths; not TUI. | none |
| CSP-390 | ADR: command palette surface and minibuffer scope | To Do (m-17) | Needs re-scoping | Its open questions are mostly settled: matching (ADR 0174), registry shape and static ids (ADR 0192), remapping seam (keys.md), placement (ADR 0158); it also cites the wrong ADR (0033). Left: the key, the scopes listed, menu-only actions, the commit route, the minibuffer. | scope note appended |
| CSP-391 | Action registry: enumerate the command surface | To Do (m-17) | Needs re-scoping | The scoped keymap (CSP-760, ADR 0192) is most of the registry: stable ids, verbs, help sections, keys, `available_when`, build-time checks. Left: the palette's row source and its tests. | scope note appended |
| CSP-392 | Command palette overlay (first deliverable) | To Do (m-17) | Needs a path or terminology update | Now a `tuikit::palette` instance over the keymap's bindings (palette.md seam); `widgets/input.rs` is gone; category headers become a cell. | dated note appended |
| CSP-393 | Minibuffer prompt (second deliverable, deferred) | To Do (m-17) | Needs a path or terminology update | A `TextPrompt<M>` host in a one-row bar (text_prompt.md seams: minibuffer host, completion and history, live validation); rename already runs on `TextPrompt`. | dated note appended |
| CSP-394 | Migrate rename, search, and view-switch prompts into the minibuffer | To Do (m-17) | Needs re-scoping | Rename is a `TextPrompt` (ADR 0166), `/` is quick select (ADR 0175), and `v` opens a transcript; what is left is routing rename and a view-switch prompt through the minibuffer. | scope note appended; dependency on CSP-193 removed |
| CSP-412 | Surface node-kind identity in non-TUI outputs (cross-surface consistency) | To Do (m-17) | Unaffected | Node-kind identity in JSON, DOT, and HTML output; not TUI. | none |
| CSP-413 | Docs and snapshot coverage | To Do (m-17) | Needs a path or terminology update | Node-kind colors are `node_*` host roles (ADR 0153), not `Theme` fields; the TUI's NO_COLOR variant needs CSP-803. | dated note appended |
| CSP-430 | Adopt `tui-textarea` when a multi-line input field lands on the backlog | To Do (m-11) | Needs re-scoping | Retargeted: `tui-textarea` 0.7 does not resolve against ratatui 0.30; `ratatui-textarea` is the maintained engine, and a multi-line area is a framework control beside `LineEdit` (text_edit.md seam). | retitled; scope note appended |
| CSP-432 | Adopt `throbber-widgets-tui` for in-flight spinners | To Do (m-11) | Needs re-scoping | Retargeted: the framework spinner (`chip::spinner_frame`, `StatusChip::busy`, CSP-725) already animates the in-flight chips; what is left is the static loading placeholders, with no new dependency. | retitled; scope note appended |
| CSP-434 | `ratatui-explorer` cwd picker for pin `create` / `adopt` | To Do (m-11) | Needs re-scoping | `PathField` completes behind a host `PathSource` and the framework reads no file system (ADR 0167), so `ratatui-explorer` does not fit; the browse mode reads the same source (roadmap Tier 3). | scope note appended |
| CSP-435 | `tui-tree-widget` extraction for the explorer | To Do (m-11) | Superseded or obsolete | Superseded: the explorer is a flat list of zones with a drill stack (`tuikit::explorer`, CSP-756, ADR 0188) and the tree is `tuikit::tree` (CSP-750, ADR 0182), so `tui-tree-widget` did not fit. | closed (Done) with a final summary |
| CSP-451 | Route agent-deck mux renames through agent-deck | To Do (m-11) | Unaffected | agent-deck rename routing is the mutation path (`rename::plan_*`); the rename prompt being a `TextPrompt` does not change it. | none |
| CSP-455 | Hybrid cwd omnibox for pin create/adopt | To Do (m-11) | Needs a path or terminology update | `path_omnibox.rs` became `tuikit::path_field::PathField` (CSP-735, ADR 0167), completion extends the typed text only, and the ranked list is the `candidates()` seam; the pin create form is gone. | dated note appended |
| CSP-455.01 | Explicit row/edit focus for pin create navigation | To Do (m-11) | Superseded or obsolete | Superseded: one ring with `Enter` submitting from any field is house rule 6 (ADR 0151, ADR 0168, CSP-736), and the pin create form it targets is gone (CSP-682). | closed (Done) with a final summary |
| CSP-487 | Add an orchestrator mutation-capability seam (deferred) | To Do (m-11) | Unaffected | Orchestrator mutation seam; not TUI. | none |
| CSP-492 | Retire or permanently bless the `[tui].sessions_grouping` legacy alias | To Do (m-11) | Unaffected | `[tui].sessions_grouping` alias in `src/config.rs`; not the theme aliases. | none |
| CSP-493 | Write the consolidated mux-attribution architecture note | To Do (m-11) | Unaffected | Mux attribution architecture note; not TUI. | none |
| CSP-494 | Adopt a two-tier decision-record convention | To Do (m-11) | Needs a path or terminology update | 51 of the 200 ADRs are the framework's (0150 to 0200), and the framework keeps its own decision log (`crates/tuikit/docs/decisions/`), which a two-tier convention must place. | dated note appended |
| CSP-505.03 | Cut the per-cycle resolve/publish cost (deferred; revisit only on the triggers below) | To Do (m-18) | Unaffected | Daemon publish cost; not TUI. | none |
| CSP-533 | Pre-publication review, then flip the repository public | To Do (m-20) | Unaffected | Parent of the publication stories; release work names no TUI code or doc that moved. | none |
| CSP-533.02 | Flip the repository public | To Do (m-20) | Unaffected | Repository visibility; the workspace changes nothing it names. | none |
| CSP-541 | Define the release process and cut `v0.1.0` | To Do (m-20) | Needs re-scoping | The crates.io choice now includes `tuikit`: the package depends on it by path and it is `publish = false` (ADR 0150), so `cargo publish` fails until that is decided. | scope note appended; CSP-104 now depends on it |
| CSP-542 | Add installation paths beyond `--path` | To Do (m-20) | Unaffected | `cargo install --locked --git` and a flake package build the whole workspace; nothing it names moved. | none |
| CSP-543 | Settle platform posture and CI shape | To Do (m-20) | Needs a path or terminology update | CI changed shape: every step takes `--workspace`, the framework is linted and tested with its features off, and a Separation rehearsal job runs (ADR 0199). | dated note appended |
| CSP-544 | Clean up dependency and tooling leftovers | To Do (m-20) | Needs a path or terminology update | `examples/pantry.rs` now previews `tuikit::checklist::Checklist` and the host's Controls adapter; ADR 0152 item 5 keeps `tui-pantry` and says the example moves with the controls, which it has not. | dated note appended |
| CSP-545 | Make the on-disk footprint consistent with ADR 0087 | To Do (m-20) | Unaffected | On-disk footprint (`graph_bin_path`, XDG paths); the framework writes no file (ADR 0150). | none |
| CSP-546 | Consolidate harness-hook docs | To Do (m-20) | Unaffected | Harness-hook docs; not TUI. | none |
| CSP-547 | Complete the documentation index and add an ADR index | To Do (m-20) | Needs a path or terminology update | `docs/index.md` gained `docs/plans/` and the framework's docs (CSP-764), and the ADR index now has 200 rows; its other omissions remain. | dated note appended |
| CSP-548 | Decide how public docs refer to Atelier | To Do (m-20) | Unaffected | How public docs refer to Atelier; not TUI. | none |
| CSP-549 | Fix or document known functional gaps | To Do (m-20) | Unaffected | An umbrella of functional gaps; the items it names keep their own outcomes (CSP-175: path update). | none |
| CSP-550 | Close the ADR-alignment items that shape the public story | To Do (m-20) | Unaffected | Umbrella over CSP-492, CSP-493, CSP-494; CSP-494 carries its own note. | none |
| CSP-551 | Add contributor onboarding | To Do (m-20) | Needs a path or terminology update | Onboarding now has a framework half: `crates/tuikit/docs/contributing.md`, its house rules and decision log, and the `tuikit-features` and `separation-rehearsal` checks. | dated note appended |
| CSP-553 | Decide the library API posture | To Do (m-20) | Unaffected | Library posture for `conspectus::api`; `tuikit` is internal by ADR 0150 and outside it; publishing is CSP-541's question. | none |
| CSP-584 | Cheaper post-hand-off rescans | To Do (m-18) | Unaffected | Daemon refresh round trips and pre-attach refreshes; overlaps CSP-644, not the framework. | none |
| CSP-616.08 | Build and maintain the opt-in transcript index | To Do (m-11) | Unaffected | The opt-in index build (ADR 0148); no TUI surface. | none |
| CSP-616.09 | Answer content searches from the index, falling back to the scan | To Do (m-11) | Needs a path or terminology update | Content search is a `tuikit::palette` with a slow provider and `palette::Progress` (ADR 0174, 0175) and Search view hits are a column schema (ADR 0185); "which source answered" fits the palette's status row. | dated note appended |
| CSP-630 | Verify Pi support against a live Pi install | To Do (m-22) | Unaffected | Pi verification against a live install; its viewer check (AC #5) reads the same with the viewer on the framework's pager. | none |
| CSP-644 | Attach to a launched pin without the pre-attach wait | In Progress (none) | Unaffected | Launch and attach flow is runtime code that stays in Conspectus (ADR 0200); the launching row's dim-and-spinner mark draws through `tuikit::chip::spinner_frame` unchanged. In Progress for another agent, so not edited. | none |
| CSP-655 | Sort repo groups by repo name instead of full path | To Do (none) | Needs a path or terminology update | `src/tui/ui/text.rs` and `compact_path_label` are gone; a header's name is `GroupRow.label`, set by `GroupRow::at_path` in `src/tui/rows/mod.rs` (CSP-752, ADR 0184). | note appended; references updated |
| CSP-677 | Treat exited zellij sessions as ghosts | To Do (m-23) | Unaffected | Zellij ghost rebuild in the mux executor; no TUI code it names moved. | none |
| CSP-686 | Rebuild every gone pinned slot at once | To Do (m-23) | Unaffected | A CLI command and a Pinned view action; menus and keys go where CSP-687's note says. The likely second long operation for a progress control (gap Tier 2). | none |
| CSP-687 | Reorder pin slots from the TUI | To Do (m-23) | Needs a path or terminology update | Menu entries are data in `actions::row_menu_for` (`ActionMenu`, ADR 0171), accelerators are keymap bindings with `available_when` (ADR 0192), and the write declares its lifecycle in `tui::effect::lifecycle`. | dated note appended |
| CSP-688 | Investigate why rebuilt worktree paths diverge from recorded roots | To Do (m-23) | Unaffected | Worktree path investigation; not TUI. | none |
| CSP-689 | Release history, rebuild, and pin slots as 0.2.0 | To Do (m-23) | Needs a path or terminology update | The version to bump is the root package's (`tuikit` keeps its own, ADR 0150), and `[Unreleased]` now carries the milestone's operator-visible TUI changes, which 0.2.0 must cover or ship apart. | dated note appended |
| CSP-690 | Retire the legacy pin loader | To Do (m-23) | Unaffected | Legacy pin loader removal; not TUI. | none |
| CSP-691 | Refresh pinned PRs that left the listing | To Do (m-23) | Unaffected | Forge refresh for pinned PRs; not TUI. | none |
| CSP-699 | Record the aider removal decision | To Do (m-24) | Needs a path or terminology update | `harness_aider` is an `ALIASES` row for the role `harness.aider` (ADR 0153); dropping it makes the key an `UnknownKey` warning; the independence guard's word list names aider and should keep it. | dated note appended |
| CSP-700 | Delete the aider harness adapter and aider-only code | To Do (m-24) | Needs a path or terminology update | Its file list predates the extraction: aider sits in `HARNESS_COLORS` and `ALIASES` (`src/tui/theme.rs`) and `badge_tests.rs`, not `badge.rs`; `crates/tuikit/tests/independence.rs` lists the word on purpose. | dated note appended |
| CSP-701 | Remove aider from operator docs, README, and changelog | To Do (m-24) | Unaffected | Operator docs; the files it names still hold the aider text, and the framework's own docs cannot name aider (independence guard). | none |
| CSP-702 | Remember expanded groups across TUI restarts | To Do (m-23) | Needs a path or terminology update | `ViewStateSlot.expanded` became `tree: TreeState<RowId>` (`tuikit::tree`, CSP-750, ADR 0182); persist `TreeState::expanded()` and restore with `set_expanded`. | dated note appended |
| CSP-704 | Say when the daemon snapshot is unreadable instead of silently rebuilding | To Do (m-20) | Unaffected | `try_daemon_snapshot` is still in `src/tui/runtime.rs` and `src/cli/mod.rs`; the TUI's warning goes through the feedback channels (ADR 0163). | none |
| CSP-766 | Assess the milestone's impact on the existing backlog | To Do (m-25) | Unaffected | This review. | closed with this review |
| CSP-770 | Stop fake-binary tests from racing on a busy executable | To Do (none) | Unaffected | A test flake filed during the milestone (ETXTBSY on fake binaries); unrelated to the framework. | none |
| CSP-776 | Dim overlay actions that cannot act now | To Do (none) | Unaffected | Filed by CSP-728 against the final framework (`keys::with_availability`, `Footer`); its text is current. Host side of the Tier 1 availability item. | none |
| CSP-802 | Fold the keymap's `Action` layer into `Msg` so every loop routes a key one way | To Do (none) | Unaffected | Filed by CSP-765 against the final runtime (ADR 0200); its text is current. | none |
