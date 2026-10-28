# Inventory 03: Text Entry, Path Completion, and Forms

Slice: `src/tui/widgets/{input,path_omnibox,form,launch_spec_form,new_mux,mux_launch,pin_edit}.rs`
(+ `*_tests.rs`), the branch-name prompt in `worktree_menu.rs`, the rename
overlay wiring, the query lines of `search.rs` and `picker.rs`, the filter
text sub-editors in `controls.rs`, ADR 0030 and ADR 0097, and
`src/tui/consistency_tests/forms.rs`. I also found a fourth, independent
text-entry implementation outside the slice (`src/viewer/find.rs`) and record
it because it is the same concept.

All paths are relative to the worktree root. Line numbers are as of branch
`worktree-tui-framework` (HEAD `ca668a3`). Statements marked **(by reading)**
were derived from the code and not confirmed by running it (this exercise
forbids running cargo/tmux).

---

## 0. Headline numbers

| Measure | Count |
| --- | --- |
| Text-entry surfaces in the TUI | 11 (section 1; 10 on `TextInputState`, 1 on `FindPrompt`) |
| Distinct text-edit *implementations* | 2 (`tui-input` via `TextInputState`; hand-rolled `FindPrompt`) |
| Distinct text-edit *renderers* of one editing state | 4 (modal, form-row window, read-only query line, viewer find footer) |
| Cursor-visible windowing algorithms | 3 (`input.rs::visible_window`, `form.rs::window`, `viewer/widget.rs::draw_find_prompt`) |
| Multi-field forms | 3 (new mux, mux launch, pin edit) |
| Field and row kinds enumerated in section 3 | 16 (11 input kinds, 5 read-only or chrome kinds) |
| Hard-coded `Color::` literals in form/input files | 19 (17 in `form.rs`, 2 in `pin_edit.rs`) vs 2 uses of `theme.error` |
| Per-modal runtime key-handler functions copy-pasted | 5 (`runtime/overlay_keys.rs`: rename, worktree, new mux, mux launch, pin edit) |
| Production consumers of `LaunchSpecFormState` | 1 (`MuxLaunchFormState`); ADR 0097's second consumer (`PinCreateState`) no longer exists |
| Dead public helpers in `form.rs` | 2 (`tagged_input_row`, `warning_row`) |
| Production callers of `PathOmniboxState::expanded_value()` | 0 |

---

## 1. Every text-entry surface

| # | Surface | Opened by | State type | Renderer | Edit engine | Submit / cancel |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Rename session / mux (alias or tmux name) | `R` -> `runtime/overlay_keys.rs:8-66` | `TextInputState` wrapped by `RenameOverlayState` (`modal.rs:165-202`) in `Modal::Rename` | `TextInputWidget` via `ui.rs:256-264` | `tui_input::Input` | Enter -> `Msg::CommitRename(String)`; Esc / Ctrl-C -> Close |
| 2 | Rename pin | row menu "Rename..." -> `Msg::OpenSlotRename` (`app.rs:1944-1947`, `actions.rs:802`) | same `Modal::Rename`, plus side flag `App::slot_prompt = Some(SlotPrompt::Rename)` (`app.rs:110-113`, `overlays.rs:43-53`) | same | same | Enter -> `Msg::CommitRename` re-routed by `slot_prompt` in the reducer (`app.rs:1820-1826`, `2175-2189`); empty -> toast "pin: nothing entered" |
| 3 | New worktree branch name | worktree menu "New worktree..." -> `Mode::BranchInput` (`worktree_menu.rs:279,350`) | `TextInputState` inside `WorktreeMenuState` | `TextInputWidget` (`worktree_menu.rs:537-543`) | `tui_input::Input` | Enter -> `Msg::CommitWorktreeCreate` (empty -> silent no-op, `:405-410`); Esc -> back to the menu list (not close) |
| 4 | Controls "max age" filter | Controls overlay row -> `SubEditor::MaxAge` (`controls.rs:328-331`) | `TextInputState` (title `" max age "`) | `TextInputWidget` (`controls.rs:848-851`) | `tui_input::Input` | Enter parses `parse_max_age` (`:659-680`); invalid -> editor stays open with **no message** (`:520-526`); Esc closes editor |
| 5 | Controls "contains" (content filter) | `SubEditor::Content` (`controls.rs:336-342`) | `TextInputState` (title `" contains "`) | `TextInputWidget` | `tui_input::Input` | Enter: empty -> `ClearContentSearch`, else `StartContentSearch` |
| 6 | `/` quick select and `s` content search query | `SearchOverlayState.input` (`search.rs:166,204`) | `TextInputState` (title `" search "`, never rendered) | read-only line `"/" + query`, no cursor, no windowing (`search.rs:821-831`) | `tui_input::Input` after the search key table misses (`:532-544`) | Enter picks a hit; Esc cancels; history paging replaces the whole input (`fill_query`, `:601-606`) |
| 7 | Picker filter (retarget a pin, search history) | `PickerState.query` (`picker.rs:50,63`) | `TextInputState` (title `" filter "`, never rendered) | read-only `"/" + query` (`picker.rs:217-222`) | `tui_input::Input` (`:157-160`) | Enter picks; Esc closes |
| 8 | New mux form (name + cwd) | `n` / `m` menu -> `Msg::OpenNewMuxForm` (`app.rs:2054-2064`, `runtime.rs:1575-1588`) | `NewMuxFormState` (`new_mux.rs:53-57`): `TextInputState` + `PathOmniboxState` | `NewMuxFormWidget` over `form::{input_row,path_row}` | `tui_input::Input` | Enter on name advances; Enter on cwd commits `Msg::CommitMuxNew` |
| 9 | Mux launch form (11 field stops) | `m` menu -> `Msg::OpenMuxLaunchForm` (`app.rs:2066-2089`) | `MuxLaunchFormState` (`mux_launch.rs:121-132`) over `LaunchSpecFormState` | `MuxLaunchFormWidget` | `tui_input::Input` | Enter on any row submits -> `Msg::CommitMuxLaunch` |
| 10 | Pin edit form (per-pin-kind rows) | row menu "Edit..." -> `Msg::OpenSlotEdit` (`app.rs:1934-1938`) | `PinEditFormState` (`pin_edit.rs:208-219`) | `PinEditFormWidget` | `tui_input::Input` | Enter on any row saves -> `Msg::CommitPinEdit`; no-change save just closes (`:531-533`) |
| 11 | Transcript / pane-viewer find prompt | `/` in viewer (`viewer/find.rs`, `pane_viewer.rs:28,136,293,378,506`) | `FindPrompt { text, cursor }` (`viewer/find.rs:46-113`), 412 LOC file | `viewer/widget.rs:492-545` `draw_find_prompt` (footer line, display-width aware) | **hand-rolled** (insert/backspace/delete/left/right/home/end/clear_line) | Enter -> find; Esc / Ctrl-C cancel; `VIEWER_FIND_KEYS` (`keybindings/overlay.rs:691-702`) |

(Row 11 is outside the nominal slice but is a fourth independent
implementation, so it counts in section 7.)

Pin rename therefore has **two** UIs and two store paths: the text prompt
(`StoreOp::RenameSlot`) and the first row of the edit form (`PinEditRequest.rename`
-> `StoreOp::SavePin`), with different validation (`slot_prompt_effect`
trims and toasts on empty; the form calls `pin_slots::validate_id` and checks
`known_pin_names`, `pin_edit.rs:482-492`).

---

## 2. Control-by-control inventory

### 2.1 `TextInputState` / `TextInputWidget` (`src/tui/widgets/input.rs`, 204 LOC + 123 LOC tests)

**Purpose.** Single-line editor (wrapping `tui_input::Input`) plus a centered
3-row bordered modal that hosts it (ADR 0030).

**Public surface.**
- `TextInputState::new(title: impl Into<String>, initial: impl Into<String>)` (`:47`), cursor lands at end.
  Accessors `title()`, `value()`, `cursor()` (codepoint index).
- `TextInputState::handle_key(KeyEvent) -> InputOutcome { Continue | Confirm(String) | Cancel }` (`:72`). Resolves `TEXT_INPUT_KEYS` (Ctrl-C -> Cancel, Enter -> Confirm, Esc -> Cancel, Tab -> swallowed) and forwards everything else to `tui_input` (`:78-81`). Does not trim.
- `TextInputWidget::new(&state).theme(&Theme)` (`:94-104`), `impl Widget`. Without `.theme()` falls back to default `tui_popup::Popup` (`:123-126`); all four production callers (`ui.rs:262`, `worktree_menu.rs:539`, `controls.rs:849`, `examples/pantry.rs:417`) pass a theme, so the fallback branch is unused.
- `centered_modal_rect(area) -> Rect` (`:165-168`): width `min(60, area.width-4).max(20)`, height 3.
- Private `visible_window` (`:179-200`): keeps the cursor in view, **cursor pinned to the right edge once scrolled**, no edge marks.

**Variants / callers.** The *only* varying inputs are `title` and `initial`.
Titles in play: `" rename session "`, `" rename mux "` (`overlay_keys.rs:33,43`), `"Rename pin"` (`app.rs:1946`), `"New worktree — branch"` (`worktree_menu.rs:350`), `" max age "` (`controls.rs:591`), `" contains "` (`controls.rs:338`), plus `" search "` / `" filter "` / form labels that are never drawn as titles. **Title padding and case are inconsistent**: padded lowercase (`" rename mux "`, `" max age "`), unpadded capitalised (`"Rename pin"`, `"New worktree — branch"`); `Popup` adds no padding (`input.rs:119` passes `Line::from(title)` unchanged), so unpadded titles sit flush against the border glyphs **(by reading)**.

**Near-copies of the same idea.** Section 7 items D1-D4, D12.

**Coupling.**

| Reference | Class |
| --- | --- |
| `crate::tui::Theme` (`:20`), uses only `panel_focus_accent` indirectly via `themed_popup` | (a) generic over a small theme trait / struct of semantic colors |
| `crate::tui::keybindings::overlay::{TEXT_INPUT_KEYS, TextInputKey, resolve}` (`:73`) | (a) key table should be a parameter or live beside the control; `KeyRow`/`resolve`/`footer` machinery itself is generic |
| `crate::tui::widgets::popup_frame::{themed_popup, centered_rect}` (`:121,167`) | (a) framework-internal once popup_frame moves |
| `tui_input`, `tui_popup::KnownSize` | external, wrapped (`tui_input::Input`) / used directly (`KnownSize`) |
| ADR 0030 locks "Tab never cycles focus while an overlay is open" | (c) is policy, but expressible as a framework rule |

No domain strings. The control is already almost entirely generic.

**External crates.** `tui-input 0.15` (`Cargo.toml:145`, `default-features=false, features=["ratatui-crossterm"]`, ADR 0030 said `crossterm`), wrapped as a shim (state struct hides `Input`). `tui-popup 0.7` (`Cargo.toml:130`) for the frame. `tui-textarea` is **not** a dependency; CSP-430 (To Do, m-11) defers it until a story needs multi-line (`backlog/tasks/csp-430 ...md`: candidates are pin `reason` notes, richer alias editing, PR-comment composer).

**What `tui-input` 0.15.3 gives for free** (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tui-input-0.15.3/src/backend/crossterm.rs:26-67`): Backspace / Ctrl-H (delete previous char), Delete, Left / Ctrl-B, Right / Ctrl-F, Ctrl-Left / Meta-B (previous word), Ctrl-Right / Meta-F (next word), Ctrl-U (delete **whole line**), Ctrl-W / Meta-D / Backspace+META / Backspace+ALT (delete previous word), Ctrl-Delete (delete next word), Ctrl-K (kill to end), Ctrl-Y (yank), Ctrl-A / Home, Ctrl-E / End; plain and Shift chars insert; Ctrl+Alt chars insert; Tab and everything else dropped. Notes **(by reading)**:
- Word-jump bindings are `Char('b'|'f'), KeyModifiers::META`, not `ALT`. crossterm reports ESC-prefixed Alt-b as `ALT`, so ADR 0030's claimed Alt-B / Alt-F probably do nothing; Ctrl-Left / Ctrl-Right work. Needs a manual check.
- Cursor is a codepoint index; `tui-input` also exposes `visual_cursor()` and `visual_scroll(width)` (unicode-width aware, `input.rs:441-474`). Conspectus uses neither: both windowing functions treat one `char` as one cell (`input.rs:181`, `form.rs:120`). Wide (CJK/emoji) or zero-width text will misplace the cursor. Only the viewer find prompt is width-aware.
- No `Event::Paste` handling anywhere in `src` (no `BracketedPaste`; `ratatui::init` does not enable it). A pasted multi-line string arrives as key events including Enter, so it **submits the form / prompt** at the first newline **(by reading)**.

**Look and feel.** Frame: `tui_popup::Popup` with `Borders::ALL`, border `theme.panel_focus_accent` (`popup_frame.rs:62-70`), title unstyled. Body: one row, `Paragraph` of the window, cursor cell `Modifier::REVERSED` (`input.rs:158-160`). No placeholder for empty value, no scroll marks, **no footer** (the hint `Enter confirm · Esc cancel` for rename is pushed to the status bar by `overlay_keys.rs:62-65`; the worktree-branch, max-age and contains uses of the same widget set no hint).

**Keys.** `TEXT_INPUT_KEYS` (`keybindings/overlay.rs:320-330`): Ctrl-C, Enter (Confirm, shown), Esc (Cancel, shown), Tab (alias, swallowed). Everything else is `tui-input`'s map above. `MODAL_CONVENTION` row "Rename": Enter Submits, Space Types (`:795`).

**Tests.** `input_tests.rs` (11): confirm/cancel/tab-swallowed/ctrl-c, typing, backspace, no-trim, modal rect caps (60 / 26 / floor 20), `visible_window`. Pantry previews 3 variants (`examples/pantry.rs:707-725`: empty, mid-edit, long value) - the only form-family control in the pantry. No insta snapshot.

**Verdict: split into generic core + thin Conspectus adapter.** Core = `LineEdit` (pure editing; returns `changed`) + `TextPrompt<M>` (title, modal frame, Enter/Esc/Ctrl-C/Tab contract, `on_confirm` mapper replacing `RenameOverlayState`'s hard-wired `Msg::CommitRename`). Merge target: the inline form-row text field (2.3), the search/picker query lines (2.8), and `FindPrompt` (2.9).

---

### 2.2 `PathOmniboxState` (`src/tui/widgets/path_omnibox.rs`, 333 LOC + 107 LOC tests)

**Purpose.** Inline path text field with ranked completion candidates, filesystem prefix suggestions, `~` handling, and an existence check. Documented as "deliberately domain-neutral" (`:3`).

**Public surface.**
- `PathCandidate { path, source, rank: i32 }` + `PathCandidate::new` (`:16-31`).
- `PathValidation { Empty | Exists | Missing }` (`:33-38`); `PathSuggestion { path, source, exists }` (`:40-45`).
- `PathOmniboxState::new(title, initial)`, `value()`, `cursor()`, `set_value()`, `set_known_candidates(Vec<PathCandidate>)`, `validation()`, `expanded_value()`, `suggestions() -> Vec<PathSuggestion>`, `handle_key(KeyEvent) -> PathOmniboxOutcome { Continue | Changed | Completed | NoCompletion }` (`:47-144`).
- Owns a `TextInputState` (so it inherits that control's whole key map); has no render function of its own - `form::path_row` / `form::path_mark` / `pin_edit::render_row` draw it.

**Full behavior.**

1. *Known candidates* (`normalize_known_candidates`, `:146-170`): trim, drop empty and duplicate paths, sort by `rank` descending then path ascending.
2. *Matching* (`path_matches`, `:223-225`): empty query matches everything; otherwise `starts_with` **or substring `contains`** (not prefix-only, despite the module doc and the task wording "prefix matching").
3. *Scoring* (`match_score`, `:227-239`): exact 100, prefix 60, substring 20, else 0. Known candidates score `rank + match_score`; filesystem entries score `10 + match_score` (`:190,204`). Conspectus ranks are 300-1000, so known paths always beat filesystem entries.
4. *Filesystem suggestions* (`:241-278`): for a non-empty query, `read_dir` the parent (or the query itself when it ends with `/`), keep names that `starts_with` the typed final component. **Files are included as well as directories** (no `is_dir` filter) and dotfiles appear when the prefix is empty. `source = "filesystem"`, `exists = true`.
5. *Tilde* (`expand_home_prefix_with`, `collapse_home_prefix_with`, `:289-329`): `~` and `~/x` expand for matching/validation; when the query starts with `~`, suggested paths are collapsed back to `~/...` (`tilde_query`, `:175,183,272`). Reads `$HOME` straight from `std::env` (`:292,312`).
6. *Cap*: `MAX_CANDIDATES = 6` (`:14`) - but see 8.
7. *Validation* (`validation`, `:87-96`): trim; empty -> `Empty`; `Path::exists` on the home-expanded value -> `Exists` else `Missing`. Advisory only; never blocks typing or committing. It is `exists`, not `is_dir`: a file shows as OK in a field the CLI later rejects as "not an existing directory" (`cli/mux.rs:103,230`).
8. *Tab* (`handle_key`, `:106-126`; `OMNIBOX_KEYS` in `keybindings/overlay.rs:621-626` binds only Tab -> Complete): replaces the **whole value** with the first suggestion whose path differs from the current value (`selected_suggestion_path`, `:128-135`), cursor to end, returns `Completed`; `NoCompletion` when there is nothing. The `selected` index is only ever set to 0 (`:62,77,82-84,111,122`) - there is no way to step through suggestions, so `selected` and `MAX_CANDIDATES` are vestigial; only the single best suggestion is ever consumed. Repeated Tab can only move on to other paths that contain the new value (a longer name), never across to a sibling that diverged earlier. Completing a directory does not append `/`, so descending requires typing `/` before the next Tab.
9. *Change detection*: any non-Tab key is forwarded to `TextInputState`; `Changed` iff the value string differs (`:117-124`), resetting `selected`. `Changed` can never come from Tab (Tab only yields `Completed`/`NoCompletion`), so the `| Changed` in every Tab arm (`new_mux.rs:135`, `mux_launch.rs:381`, `pin_edit.rs:589`) is unreachable.
10. *I/O cost*: `validation()` (one `stat`) runs per path row per frame (`form.rs:251`), and `suggestions()` (a `read_dir` plus one `stat` per matching known candidate for `PathSuggestion.exists`, `:194`) runs per frame while a path row is focused (`form.rs:293-300`) and again on every Tab. No caching and no injection point for the filesystem or `$HOME`; the tilde test needs a real `$HOME` (`path_omnibox_tests.rs:90-92`).
11. *Dead data*: `PathCandidate.source` and `PathSuggestion.source` / `.exists` are produced and unit-tested but never rendered (no badge, no dim-if-missing). `expanded_value()` (`:98-100`) has **no production caller**: the forms commit the raw typed text (`new_mux.rs:104-113`, `mux_launch.rs:321`, `pin_edit.rs:499-517`), and the CLI takes `--cwd` as a `PathBuf` and tests `is_dir()` with no tilde expansion (`cli/mux.rs:99-106`). So a `~/proj` that shows the green mark can be rejected by the launched CLI **(by reading; needs a runtime check)**.

**The tab hint** (`form.rs:289-320`): `completion_hint` returns `"Tab: <remainder>"` for the first suggestion `!= value` (only while the row is focused). `completion_remainder` is `suggestion.strip_prefix(typed)`, falling back to basename-prefix remainder. For a substring match (typed `src`, suggestion `/home/x/src/foo`) neither strips, so **no hint is shown even though Tab would rewrite the field** to the full suggestion; when the basename fallback applies, the hint shows only the basename tail while Tab replaces the whole path. Hint and action can disagree **(by reading)**.

**Variants: how each caller customizes it.**

| Caller | Title arg | Candidates | Rendered by | Mark | Tab hint | After Tab success |
| --- | --- | --- | --- | --- | --- | --- |
| New mux cwd (`new_mux.rs:67,74-77`) | `"cwd"` | `known_cwd_candidates(app, Some(&seeded))` (`runtime.rs:1582`, `app.rs:2060`) | `form::path_row` | yes, in a 32-cell reserved suffix | inline `Tab: x` | focus stays |
| Mux launch cwd (`launch_spec_form.rs:73`, `mux_launch.rs:212-217`) | `"cwd"` | same builder (`app.rs:2075`) | `form::path_row` | yes (32-cell suffix) | inline | focus stays, error cleared (`mux_launch.rs:381-384`) |
| Pin edit `cwd` / `location` / `path` rows (`pin_edit.rs:263-270`) | the raw field name | `cwd_candidates` in `PinEditInit` (`overlays.rs:542`) | `pin_edit::render_row` Path arm (`:814-823`) | yes (2 cells, `form::path_mark`) | **not inline**: the focused-row detail line says "Tab completes the path" (`:749-755`) | row `cleared = false` (`:592`) |
| Mux launch worktree **repo** (a path!) | n/a | none | `input_row` | **none** (plain `TextInputState`, `mux_launch.rs:123,160`) | none | n/a |

Domain content is limited to the caller: `runtime::known_cwd_candidates` (`runtime.rs:1601-1650`) builds candidates from graph nodes with hard-coded ranks and source strings: selected 1000, pin 500, agent 450, mux 440 / 430 (active pane path), process 400, checkout 350, repo 320, workspace 300. That is the adapter, correctly outside the widget.

**Coupling.**

| Reference | Class |
| --- | --- |
| `crate::tui::widgets::input::TextInputState` (`:12`) | framework-internal |
| `crate::tui::keybindings::overlay::{OMNIBOX_KEYS, OmniboxKey, resolve}` (`:107`) | (a) the one table that lives in the app-wide `overlay.rs`; move beside the control or take the key as config |
| `std::fs::read_dir`, `Path::exists`, `std::env::var_os("HOME")` | (a) hide behind a `PathSource` trait (candidates, `list_dir`, `exists`, `is_dir`, home) so tests and non-Unix hosts can substitute |
| rank / source vocabulary | already caller-supplied: (b) stays in Conspectus (`known_cwd_candidates`) |

No `crate::model`, `discovery`, or graph imports. The domain-neutral claim is **substantially true for imports and vocabulary**, but false in three behavioral respects: (1) directories vs files are conflated while every caller means "directory"; (2) validation and rendering assume a local Unix filesystem and run I/O in the render path; (3) the contract between *validity of what is shown* and *what is committed* is broken for `~` (item 11).

**Tests.** `path_omnibox_tests.rs` (6): ranking, Tab completes best candidate, missing path not rejected, filesystem prefix (tempdir), tilde expansion/collapse, tilde candidate completion. Form-level: `consistency_tests/forms.rs:58-97` (Tab completes in mux-launch and new-mux), `mux_launch_tests.rs:327,354,417`, `new_mux_tests.rs:120-152`, `pin_edit_tests.rs:312`, `form_tests.rs:42` (hint remainder).

**Backlog.** CSP-455 (To Do): "Hybrid cwd omnibox" follow-up asks for multiple ranked completions inline or in a sidecar; CSP-434 (To Do): `ratatui-explorer` browse mode as an alternative.

**Verdict: extract after refactor** as `PathField` (state + `PathSource` trait + `PathKind { Any, Dir, File }` + validity enum + `complete()`), with the rendering (mark + hint) moved into the framework form-row kit. Fix or delete the vestigial `selected` / `MAX_CANDIDATES` / `source` / `exists` / `expanded_value` surface as part of extraction (decide: render a source badge and a candidate list, or remove).

---

### 2.3 Form row kit (`src/tui/widgets/form.rs`, 741 LOC + 141 LOC tests)

**Purpose.** Shared row renderers and popup bodies for the three multi-field forms, plus unrelated string helpers.

**Public surface (all free functions returning `Line<'static>` or `Vec<Span>`).**

| Function | Line | Role | Production callers |
| --- | --- | --- | --- |
| `Cols { inner, label }`, `Cols::new`, `.with_label` | 29-47 | width budget; `LABEL_WIDTH = 11` (`:21`) | all 3 forms |
| `prefix(label, active, label_width)` | 59-67 | `"> "` / `"  "` + padded label + 2-space gap; bold when active | `value_row`, `harness_row`, `static_value_row`, `note_row`, `warning_row`, `pin_edit::render_row` |
| `entry_style(active)` | 69-75 | value cell style: White on DarkGray when active, Gray otherwise (fixed colors) | `value_cells`, `mux_launch::name_line` |
| `window(...)` (private) | 101-159 | the inline text field window: `"< "`/`"  "` left mark, `" >"`/`"  "` right mark, cursor cell, `-` for an empty value; always exactly `width` cells | via `value_cells` / `value_row` |
| `value_cells` | 162 | public wrapper over `window` | `pin_edit` x4 |
| `value_row(label, value, cursor, active, cols, Option<Suffix>)` | 177-199 | the canonical text row | `input_row`, `path_row`, `mux_launch::name_line` |
| `input_row(label, &TextInputState, active, cols)` | 201 | text row | new_mux, mux_launch x5 |
| `tagged_input_row` | 207-230 | text row + dim note | **none (dead)** |
| `path_row` / `path_suffix` / `path_mark` | 234-287 | value + 32-cell suffix (`PATH_SUFFIX_WIDTH`, `:50`) holding the mark and the inline `Tab:` hint; `path_mark` is the 2-cell mark alone | `path_row`: new_mux, mux_launch; `path_mark`: pin_edit |
| `completion_hint`, `completion_remainder` | 289-320 | the `Tab: x` text | `path_suffix` only |
| `harness_row`, `harness_chips` | 325-398 | value slot (18 cells, `VALUE_SLOT_WIDTH`, `:24`) + `[key]` chips (selected = cyan bold, others DIM, truncated with ` …`, budget `min(48)`), or ` [custom]` (yellow) when the value is not a known key | `harness_row`: mux_launch; `harness_chips`: pin_edit |
| `options_row` | 402-420 | `[x] a [ ] b` from `HarnessLaunchOption`s checked by argv fragment | mux_launch, pin_edit |
| `choice_row` | 423-435 | `[x] a [ ] b` from `(&str, bool)` | mux_launch (pin choice) |
| `static_value_row` | 438-449 | label + 2-space gutter + truncated text | options_row, choice_row, worktree toggle |
| `note_row` | 452-470 | dim italic label + value | pin_edit |
| `derived_row` | 476-499 | `  └ label` lead-in, dim italic; yellow italic when `warn` | mux_launch `command` row |
| `warning_row` | 501-516 | dim label + yellow value | **none (dead)** |
| `wrap_footer(footer, width)` | 520-539 | splits the footer string on `" · "` and rewraps dim | 3 forms |
| `default_argv(Option<&LaunchDefaults>, harness, cwd)` | 543-563 | domain: profile / `atelier exec` / harness default | mux_launch (x3), pin_edit |
| `checkbox(label, selected)` | 565-571 | `[x] label` / `[ ] label` | options_row, choice_row, mux_launch socket checkbox |
| `truncate_chars`, `fit`, `wrap_words`, `display_argv`, `shell_display_arg` | 573-631 | string helpers | various |
| `modal_rect(area, width_cap, content_lines, min_height)` | 634-639 | width `min(cap, area.width-4).max(44)`, height clamp | 3 forms |
| `ScrollLinesBody`, `SplitBody`, `scroll_offset_for_cursor` | 642-737 | `KnownSize` popup bodies: whole-body scroll vs pinned top / scrolling rows / pinned bottom | `ScrollLinesBody`: new_mux, mux_launch; `SplitBody`: pin_edit |

**Layout rules (label / value columns).** Prefix = marker(2) + label padded to `cols.label` (default 11) + gap(2) = 15 cells; then the value window (2-cell left mark + content + 2-cell right mark, so the value text starts at column 17); static rows insert `TEXT_GUTTER` ("  ") so their text starts in the same column. `derived_row` lead-in is `"  └ "` + label padded to `label-2` so the value column still lines up. Labels longer than the label width are not truncated, they push the value right; pin_edit therefore computes `label_width = clamp(max label, 11, 16)` (`pin_edit.rs:644-650`) while mux_launch always uses 11 (its longest label, `launch argv`, is exactly 11). Pin edit alone draws a right-aligned 11-cell "tag" column through its own `Grid` (`pin_edit.rs:761-774`); the other forms use per-row suffixes of different widths (0, 2, 32 cells).

**Scroll indicators.** Horizontal only: `< ` / ` >` in cyan bold (cyan bold on DarkGray when active), inside the 4 edge cells (`window`, `:101-159`). Windows narrower than 5 cells drop the marks and show only the cursor cell (`:125-136`). Vertical: none; `ScrollLinesBody` / `SplitBody` scroll silently by `Paragraph::scroll` / row skipping.

**Coupling.**

| Reference | Class |
| --- | --- |
| `crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment}` (`:17`) in `options_row` | (a) take `&[(label, checked)]` - `choice_row` already does; the two functions are the same widget |
| `crate::launch_default::LaunchDefaults`, `discovery::harness::launch_argv_for` in `default_argv` (`:543-563`) | (b) Conspectus adapter; not a UI helper at all |
| `crate::tui::widgets::{input,path_omnibox}` | framework-internal |
| `Color::{White,DarkGray,Gray,Black,Cyan,Green,Yellow}` literals (17 sites) | (a) replace with theme semantic roles (the `Theme` doc, `theme.rs:9-14`, says selection uses `REVERSED` precisely so it adapts to light/dark terminals; form cells ignore that) |
| `"√"` literal (`:253,283`) | the glyph registry already has `icons::marks::OK = "√"` documented as "A path or value that checked out, in forms" (`icons.rs:130-131`) and `FAIL "×"`; form.rs bypasses it and invents `!` / `?` marks that are in no registry |
| the `" · "` separator split in `wrap_footer` (`:524`) | string-typed contract with `overlay::footer` (joins with `" · "`, `overlay.rs:97`); should pass `Vec<Segment>` |
| `display_argv` / `shell_display_arg` (`:614-631`) | (b) argv-domain; duplicates `argv::shell_quote` (`src/argv.rs:10-24`, different safe-char set: `_-./:=+` vs `_@%+=:,./-`) |

**Tests.** `form_tests.rs` (9): long-value window and edge marks, empty dash, label widening, completion remainder, harness chip highlighting and `[custom]`, options checked state, footer wrap, scroll-offset rule, `display_argv`. `consistency_tests/layout.rs` checks that moving selection never moves other text, for `PinEdit`, `RowMenu`, `Picker`, `MuxLaunch`, `NewMux` (`:406-415`).

**Verdict: split.** Generic: `Cols`/grid, `window` (as the single text-field windowing), `value_row`/`input_row`/`path_row`/`harness_row`->`ChoiceRow`/`CheckboxRow` (merge `options_row` + `choice_row`) /`static_value_row`/`note_row`/`derived_row` (merge `note_row` + `warning_row` + `derived_row` into one read-only row with tone), `wrap_footer`, `modal_rect`, `ScrollLinesBody` + `SplitBody` (unify), `fit`, `wrap_words`. Adapter: `default_argv`, `display_argv`, `completion_remainder` can stay generic. Delete: `tagged_input_row`, `warning_row` (or use them).

---

### 2.4 `LaunchSpecFormState` (`src/tui/widgets/launch_spec_form.rs`, 457 LOC + 158 LOC tests; ADR 0097)

**Purpose.** State bag for the launch-spec fields: harness, cwd (omnibox), mux name, mux socket, launch argv, worktree toggle + branch, known harness keys, known live mux names, error. Plus harness cycling, launch-option toggling, argv parsing helpers.

**Public surface.** `LaunchSpecInit<'a>` (12 fields, `:50-63`), `LaunchSpecFormState::new`, 10 read accessors (`:90-119`), 6 `*_mut` accessors (`:123-140`), `set_*` replacers that **take the title again** (`set_harness(title, value)` etc., `:147-161`), `cycle_harness`, `cycle_harness_keeping_argv`, `launch_options`, `effective_argv`, `toggle_launch_option`, `mux_name_collides_with_known`, `cwd_handle_key`, `handle_text_key(SpecTextField, KeyEvent)` (`:347-355`), free fns `optional_string` (`:376`) and `parse_launch_argv(raw, error_prefix)` (`:390-453`, a shell-style tokenizer).

**Status vs the ADR.** ADR 0097 designed this to be shared by `PinCreateState` and the mux-launch form. `PinCreateState` no longer exists (no `pins.rs`), so the primitive has exactly **one** consumer, `MuxLaunchFormState` (`mux_launch.rs:122`). The ADR's field order for the cursor (harness -> cwd -> mux name -> ...) is stale: the code starts on **mux name** (`mux_launch.rs:161`). Pin edit (the intended third wrapper, ADR "Open Questions Deferred") reimplemented harness cycling, option toggling, and default-argv resolution instead of using it (item D8).

**Coupling.** Heavy domain: `crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment, argv_with_fragment, argv_without_fragment, launch_options_for, strip_known_launch_option_fragments}` (`:20-22`), `crate::tui::cursor::wrap_step` (generic), `form::display_argv`. Class (c): inherently Conspectus (harness launch). Only `parse_launch_argv` + `optional_string` + the generic parts of `cycle_harness` (cycle a value through a known list, with a custom-value escape) are reusable.

**Tests.** `launch_spec_form_tests.rs` (12): cycle, collision, argv parser, option toggle, harness switch drops flags.

**Verdict: stays in Conspectus** (it is the mux-launch domain model). Extract only: `cycle_in_list`, a shell-argv tokenizer (reconcile with `argv::split_command_line`, `src/argv.rs:39`), `optional_string`. Retire the `LaunchSpecFormState` abstraction when MuxLaunch moves to the generic `Form` (the state becomes the form's field values).

---

### 2.5 New mux form (`src/tui/widgets/new_mux.rs`, 215 LOC + 169 LOC tests; ADR 0095)

**Fields.** `name` (text), `cwd` (path). `NewMuxField { Name, Cwd }`, `toggle()` (`:35-47`).

**Public surface.** `NewMuxFormState::new(name, cwd)`, `.with_cwd_candidates`, `name_value()`, `cwd_value()`, `focus()`; `impl Overlay` (`handle(&mut self, (), KeyEvent) -> OverlayOutcome`, `:122-158`); `NewMuxFormWidget::new(&state, &Theme)` (`:162-211`).

**Keys** (`NEW_MUX_KEYS`, `keybindings/overlay.rs:336-344`, footer `↑/↓ move · Tab complete/next · S-Tab back · Enter next/commit · Esc cancel`):
- Up / Down / S-Tab: toggle focus (2 fields). Tab: on cwd, try completion first; if `Completed`/`Changed` stay, else toggle; on name, toggle.
- **Enter**: on name, non-empty -> advance to cwd (empty -> silent no-op); on cwd, both non-empty -> `Msg::CommitMuxNew`; empty name snaps focus back to name; empty cwd -> silent no-op (`try_commit`, `:94-116`).
- Esc / Ctrl-C: `OverlayOutcome::Close`. Left / Right / Space edit text (no option rows).
- **No error display at all**; empty fields do nothing.

**Look and feel.** Modal width cap 76, floor 44, min height 6 (`:175-192`), title `" New tmux session "`, rows via `input_row("name")` / `path_row("cwd")`, blank line, wrapped footer; `ScrollLinesBody` (footer scrolls away on a short terminal).

**Coupling.** (b)/(c): `crate::tui::Msg` (`:24`, builds `Msg::CommitMuxNew{name,cwd}` at `:113`), `crate::tui::modal::{Overlay, OverlayOutcome}` (`:25`; the trait's `Commit(Box<Msg>)` binds every form to the app message type). Pure UI otherwise. The "seed from selected row / `$HOME` fallback" logic is duplicated at the call sites (`runtime.rs:1575-1588` with a hand-written status string that omits "S-Tab back", versus `app.rs:2054-2064`) - item D11.

**Tests.** `new_mux_tests.rs` (14) + `consistency_tests/forms.rs:81-97`. No snapshot.

**Verdict: collapses onto generic `Form`** (2 fields: Text, Path). Its "Enter advances" rule is a per-form submit policy (open question Q1).

---

### 2.6 Mux launch form (`src/tui/widgets/mux_launch.rs`, 717 LOC + 559 LOC tests; ADR 0096)

**Field stops** (`MuxLaunchField`, `:59-74`), in visible order (`visible_fields`, `:253-273`): `MuxName` (with an inline `[ ] socket` checkbox suffix; `MuxSocketToggle` is a Tab-only stop), `MuxSocket` (only when enabled), `Harness`, `LaunchOptions` (only if the harness offers some), `Cwd`, `LaunchArgv`, [derived `command` row, not a stop], `WorktreeToggle`, `WorktreeBranch` and `WorktreeRepo` (only when the toggle is on), `Pin`.

**Field types used.** Text (name, socket, argv, branch, repo), conditional text (socket, branch, repo), harness chip-choice with custom escape, checkbox group (`options_row`), path (cwd), derived read-only (`command`), toggle (worktree, with state-dependent label text hand-written at `:584-588`), inline reveal-checkbox (socket), one-of-3 (`PinChoice` rendered by `choice_row`, `:78-101,618-631`).

**Public surface.** `MuxLaunchFormState::new(harness, cwd, mux_name, known_harness_keys, known_mux_names)` + builder methods `with_defaults`, `with_worktree_repo`, `with_worktree`, `with_cwd_candidates`; `spec()/spec_mut()`; `focus()`; `error()`; `impl Overlay` (`:357-481`); `MuxLaunchRequest` (8 fields, `:106-115`); `MuxLaunchFormWidget::new(&state, &Theme)`.

**Keys** (`MUX_LAUNCH_KEYS`, `overlay.rs:590-601`, footer `↑/↓ move · Tab complete/next · S-Tab back · ←/→/Space option · Enter launch · Esc cancel`):
- Up / Down: move between **rows**, wrapping (`advance_row`, `:277-286`; the socket checkbox counts as the name row).
- Tab / S-Tab: move between **stops**, wrapping, the socket checkbox included (`advance_stop`, `:290-295`). Tab on cwd tries completion first and only moves on when nothing completed (`:376-390`).
- Left / Right / Space = `OptionPrev` / `OptionNext` (`FormKey`): on Harness cycle the known keys while carrying the typed argv across (`cycle_harness_keeping_argv`, `launch_spec_form.rs:290-312`); on LaunchOptions toggle; on Pin cycle 3-way; on the socket checkbox and worktree toggle flip. **On text rows they fall through and edit** (Space types, arrows move the cursor), so on the Harness row Left / Right can never move the text cursor (`:398-410`).
- Enter: validate-and-submit from **any** row (`try_commit`, `:297-305`); `build_request` checks, in order: harness required, mux name required, name not already a live mux, cwd required, argv parses, worktree branch and repo required when on (`:307-354`).
- Esc / Ctrl-C: `Close`, discarding silently.

**Validation display.** One error string in `LaunchSpecFormState.error`; rendered as blank line + a single `Line` styled `theme.error` before the footer (`:632-638`), unwrapped (clipped at the border if long); cleared on any edit, toggle, cycle, or successful completion, not on navigation. Messages are prefixed `"mux launch: ..."` (`:310-340`). Path validity is advisory (`!` yellow, text and CLI error later); harness validity is `[custom]` (yellow) only.

**Look and feel.** Cap 76, floor 44, min height 10; title `" Launch harness in new mux "` (`:654`); snapshots at `src/tui/widgets/snapshots/conspectus__tui__widgets__mux_launch__tests__the_form_at_80_columns.snap` etc. Labels: `mux name`, `mux socket`, `harness`, `options`, `cwd`, `launch argv`, `command`, `worktree`, `branch`, `repo`, `pin`; `options`, `command`, `worktree`, and `pin` are inline literals, not in the label constants block (`:47-53`).

**Customization points** that exist today are all builder methods on the concrete state. There is no config for field set or order beyond the hard-coded `visible_fields`.

**Coupling.** (c) `Msg::CommitMuxLaunch`, `MuxLaunchRequest`, `crate::launch_default::LaunchDefaults` (`:131,170`), `discovery::harness` (via `LaunchSpecFormState`), `PinChoice` (ADR 0141 pin semantics), `form::default_argv`. (a) `Overlay` / `OverlayOutcome` / `FormKey` / `wrap_step`. Everything else is layout.

**Tests.** `mux_launch_tests.rs` (29 tests incl. 3 insta snapshots at 120 / 80 columns and with the socket shown), `consistency_tests/forms.rs` (arrow navigation, Tab completion, harness options, read-only `command` row).

**Verdict: split into generic `Form` + Conspectus adapter** (field list, conditions, request builder, `PinChoice`, `LaunchDefaults` hint).

---

### 2.7 Pin edit form (`src/tui/widgets/pin_edit.rs`, 850 LOC + 486 LOC tests)

**Rows.** One for the pin name, one for `label`, then `PinKind::overridable_fields()` (`model/slot.rs:115-130`): Mux = `name, socket, cwd, harness, argv, worktree_branch`; Session = `resume_into`; Checkout = `branch, base, location`; Repo = `remote, path`. Row labels are the **raw field names**.

**Field types.** `Control { Text(TextInputState), Path(PathOmniboxState), Harness(TextInputState) }` (`:69-73`): Path for Mux `cwd` / Checkout `location` / Repo `path`, Harness for Mux `harness`, Text otherwise (`:263-275`). Extra stop `Stop::Options` under the harness row (checkbox group). Read-only `note_row` rows and a wrapped help paragraph at the top, and a 2-line focused-row explanation (always exactly two lines, so the height never changes, `:711-717`).

**The override-with-reset model** (the Ctrl-R feature). Each `Row` carries `initial`, `overridden`, `inherited: (String, Option<FieldSource>)`, `source`, `cleared` (`:103-117`). The right-hand 11-cell tag column (`TAG_WIDTH`, `:204`) shows, in priority order: `will follow` (cleared), `will keep` (edited), `kept` (override present), else the layer word `running` / `last seen` / `saved` / `default` / `kept` / `unset` (`:126-141,192-201`). Ctrl-R (`FormKey::Inherit`) replaces the value with the inherited one and sets `cleared` (`:466-479`); on the pin-name row it restores the initial text. Typing after Ctrl-R re-overrides (`typed`, `:543-561`). `request()` (`:482-526`) turns the rows into `set` / `unset` lists; emptying an overridden field unsets it. The focused-row hint (`:145-188`) explains in plain words what following would give.

**Keys** (`PIN_EDIT_KEYS`, `overlay.rs:603-615`): same as mux launch plus `Ctrl-R follow again`; Up / S-Tab = back, Down / Tab = forward, one linear ring (no separate "rows" vs "stops", unlike mux launch); Tab on a Path row completes first (`:584-597`); Left / Right / Space cycle the harness row and toggle the options stop, edit text elsewhere; Enter saves, Esc closes silently.

**Validation display.** `error` string at the bottom, `fit`-truncated to the detail width and prefixed with two spaces (`:719-724`), cleared only when a *value* changes (`typed`, `:552-555`).

**Look and feel.** Cap **84**, floor 44, min height 10; title `" Edit pin "` (`:743`). `SplitBody` pins the top band (pin summary + help) and the bottom band (detail lines, error, footer) while only the rows scroll (`form.rs:670-721`) - the only form that keeps its footer on screen when short. Grid: `2 + label + 2 + value + 1 + 11`. Path rows show only the 2-cell mark inline (no `Tab:` hint; the detail line carries "Tab completes the path"). The harness row repeats the chip/custom logic of `form::harness_row` locally (`:786-813`).

**Coupling.** (c) `crate::model::{FieldSource, FieldValue, PinKind, RecipeField, SlotFields}` (`:27`), `crate::pin_slots::validate_id` (`:487`), `crate::pin_ops::parse_override` (`:516`), `discovery::harness`, `launch_default`, `Msg::CommitPinEdit`. The layer words (`running`/`last seen`/`saved`/`default`) are Conspectus vocabulary: (b). The *mechanism* (value + inherited value + source word + pending state + reset key + two-line explanation) is generic: (a) `OverrideField { inherited: String, source_word: &str, hint: fn }`.

**Tests.** `pin_edit_tests.rs` (22): row list, source words, navigation wrap, no-change close, override set / drop, Ctrl-R, rename validation, option toggle, harness cycle, Tab completion, column stability (the tag column never moves, path edits never push it out), jargon ban (`inherit`, `slot`, `snapshot` must not appear). No insta snapshot.

**Verdict: split.** Generic `OverrideRow` decoration + `SplitBody` layout + the grid with a tag column go into the framework; the field map, layer words, request builder, and validation stay in Conspectus.

---

### 2.8 Query lines, branch prompt, filter sub-editors

- **Search query line** (`search.rs:821-831`): `span("/", panel_focus_accent)` + `query.to_string()`; no cursor cell, no horizontal windowing, so a long query clips at the right border and the typing position is not visible. The comment (`:816-820`) says this is deliberate ("single bordered modal"). Edit engine is `TextInputState::handle_key` called after the search key table misses; the returned outcome is discarded (`:538`) because Enter/Esc are consumed earlier. `Ctrl-F`, `Ctrl-R`, `Ctrl-T`, `Ctrl-O`, `Ctrl-S`, `Ctrl-N`, `Ctrl-P` are bound by `SEARCH_CONTENT_KEYS` / `SEARCH_KEYS` before the line editor sees them; of these only `Ctrl-F` collides with a `tui-input` binding (forward char), so content search loses that readline key. The search holds `past`-query paging that replaces the whole `TextInputState` (`fill_query`, `:601-606`), which resets the cursor to the end.
- **Picker query line** (`picker.rs:217-222`): identical `"/"` + text render, again no cursor; text keys reach `TextInputState::handle_key` and re-filter on *every* key whether or not the value changed (`:157-161`). Height clamp `max_height.clamp(8, 20.max(max_height))` (`:181`) makes the `20` a no-op, so the picker is always nearly full height - probably a bug.
- **Worktree branch prompt** (`worktree_menu.rs:396-421`): `Mode::BranchInput(TextInputState)`; Esc returns to the menu list rather than closing (differs from rename, where Esc closes); empty Enter is a silent no-op; no hint is set in the status bar when it opens (rename sets one); rendered by the same 3-row `TextInputWidget` with title `"New worktree — branch"`.
- **Controls sub-editors** (`controls.rs:168-177,484-531,822-852`): the editing is by pop-up `TextInputWidget`; the row in the Controls list is a *button* (`"{name:12} {value}  [ Edit ]"`, `:1153`). So there are two idioms for an editable text value: edit-in-place (forms) and open-a-prompt (Controls rows). Invalid max-age silently keeps the editor open with no message (`:520-526`), the only text field whose validation failure is invisible.

**Verdict.** These collapse onto `TextPrompt<M>` (prompt idiom) and one inline `QueryLine` (a `TextField` variant with a prefix glyph and cursor). The Controls "[ Edit ]" button idiom is a separate framework decision (belongs with the Controls inventory).

---

### 2.9 Viewer find prompt (outside slice; recorded for the dedupe)

`FindPrompt` (`viewer/find.rs:46-113`) is a hand-rolled single-line editor, char-index based, with `byte_at` conversions. It has its own key table `VIEWER_FIND_KEYS` (`overlay.rs:691-702`: Enter find, Esc cancel, Backspace, Delete, Left, Right, Home, End, **Ctrl-U = delete before the cursor**) and `typed_char` (`overlay.rs:706-717`). No word motion, no Ctrl-W / Ctrl-K / Ctrl-A / Ctrl-E, **no Ctrl-U-deletes-whole-line** (tui-input's Ctrl-U deletes the whole line, `FindPrompt::clear_line` deletes only before the cursor). Renderer `draw_find_prompt` (`viewer/widget.rs:492-545`) is the only unicode-width-aware implementation, with a reversed block cursor. The viewer is designed behind an extraction boundary (ADR 0052), which is why it cannot import `tui::widgets::input`; a framework crate below both would remove that reason.

---

## 3. Field-type matrix

| Field type | Implemented in | # divergent implementations | Keys | Scroll indicator | Validity / error | Label/value layout |
| --- | --- | --- | --- | --- | --- | --- |
| Plain text (inline) | `form::value_row`/`input_row` + `window` (`form.rs:101-199`) | 1 inline renderer, but see "text prompt" and "query line" below | tui-input map; Tab = next field; Enter = form policy | `< ` / ` >` cyan bold, 4 edge cells | none inline; form-level error line | prefix 15 cells; value from col 17; empty = `-` |
| Plain text (modal prompt) | `TextInputWidget` (`input.rs`) | 1 (but 6 titles, 6 callers) | tui-input map; Enter confirm; Esc cancel; Tab swallowed | none; cursor pinned to right edge when scrolled | silent no-op (branch, max age) / toast (pin rename) / none | n/a (single row, border title) |
| Query line (read-only render of a text state) | `search.rs:821`, `picker.rs:217` | 2 identical copies | tui-input map after table miss | **none, no cursor** | n/a | `/` accent glyph + text |
| Viewer find (footer prompt) | `viewer/widget.rs:492` + `FindPrompt` | 1 (separate engine) | own subset | width-aware scroll, no marks | n/a | `/` bold + text + dim hint |
| Path with completion | `PathOmniboxState` + `form::path_row/path_suffix` + `pin_edit::render_row` Path arm | 2 renderers (32-cell suffix vs 2-cell mark) | Tab complete, else next field | same as text | mark `?` DarkGray / `√` Green / `!` Yellow (advisory); inline `Tab: x` (cyan, only focused, only prefix matches) | suffix 32 cells reserved even when empty, eats value width |
| Path field that is not a path field | mux-launch `repo` (`mux_launch.rs:160`) | 1 | plain text | same | **none** | plain |
| Harness choice (text + chips + cycle) | `form::harness_row` (`:325-362`), `pin_edit::render_row` Harness arm (`:786-813`) | 2 near-copies | Left / Right / Space cycle known keys (arrows therefore cannot move the cursor); typing allowed | same | `[custom]` yellow when not a known key | value slot 18, chips budget `min(48)` (mux_launch) vs remainder of the grid (pin_edit) |
| Checkbox group (adapter options) | `form::options_row` (`:402`) | 1 (used by 2 forms) | Left / Right / Space toggle | none | n/a | `[x] a [ ] b` joined by one space |
| One-of-N choice | `form::choice_row` (`:423`), same output shape as `options_row` | 1 | Left / Right / Space cycle | none | n/a | `[x]` / `[ ]` identical to checkboxes - radio vs checkbox are visually indistinguishable |
| Single toggle row | hand-written text in `mux_launch.rs:582-591` (`"[x] create or reuse a worktree for the branch"` / `"[ ] no worktree"`) + inline `[ ] socket` (`:665-694`) | 2 (and a 3rd checkbox idiom in `controls.rs:1185`, a 4th in `search.rs:1068`) | Left / Right / Space | none | n/a | sentence-style label that changes with state |
| Conditional field | `visible_fields` (`mux_launch.rs:253`), `stops()` (`pin_edit.rs:347`) | 2 hand-coded | focus rings recomputed | n/a | n/a | rows appear / disappear (height changes; no layout test for it) |
| Override-with-reset | `pin_edit.rs` `Row`/`tag`/`hint`/`inherit` | 1 (unique) | Ctrl-R | n/a | tag word `will follow` / `will keep` / `kept` / layer; yellow when pending | 11-cell right-aligned tag column |
| Derived read-only row | `form::derived_row` (`:476`) | 1, plus near-copies `note_row`, `warning_row` | none, never focusable | n/a | `warn` -> yellow | `  └ label`, value aligned with field values |
| Note / help paragraph / detail lines | `note_row`, `form::wrap_words`, `fit` | 1 | n/a | n/a | n/a | dim, 2-space indent |
| Error line | `mux_launch.rs:632-638`, `pin_edit.rs:719-724`, absent in `new_mux.rs` | 2 + none | n/a | n/a | `theme.error` | blank line + plain `Line` (mux launch) vs `fit`-truncated (pin edit) |
| Footer hint | `wrap_footer(footer(TABLE))` | 1 (+ status-bar copy elsewhere) | n/a | n/a | n/a | dim, wrapped at `" · "`; scrolls away in 2 forms, pinned in 1 |

---

## 4. Key handling matrix

| Key | TextPrompt (rename, branch, max age, contains) | NewMux | MuxLaunch | PinEdit | Search / Picker query | Viewer find |
| --- | --- | --- | --- | --- | --- | --- |
| Enter | confirm (value as-is; callers trim) | name: next field; cwd: commit | submit from any row | save from any row | pick | find |
| Esc | cancel (rename closes; branch prompt returns to list) | close | close | close | cancel | cancel |
| Ctrl-C | cancel | close | close | close | cancel | cancel |
| Tab | swallowed (no-op) | complete (cwd) else toggle field | complete (cwd) else next stop | complete (path) else next row | scope toggle (search) / swallowed (picker) | unbound |
| Shift-Tab | not bound (falls to tui-input, dropped) | toggle field | prev stop | prev row | scope / granularity (search) | unbound |
| Up / Down | not bound | toggle field | prev / next **row**, wraps | prev / next row, wraps | list cursor | unbound |
| Left / Right | cursor | cursor | harness cycle, options / toggle / pin step; cursor on text rows | harness cycle, options toggle; cursor on text rows | cursor | cursor |
| Space | types a space | types | harness cycle / toggle / pin step; types on text rows | harness cycle / option toggle; types on text rows | types | types |
| Home / End / Ctrl-A / Ctrl-E | tui-input | tui-input | tui-input | tui-input | tui-input | Home / End only |
| Ctrl-W / Ctrl-K / Ctrl-Y / Ctrl-U | tui-input (Ctrl-U = whole line) | same | same | same | same, but Ctrl-F / Ctrl-R / Ctrl-T / Ctrl-O / Ctrl-S shadowed by search table | Ctrl-U = before cursor only |
| Ctrl-R | tui-input unbound | unbound | unbound | **Inherit (follow again)** | **earlier search** (content mode) | unbound |
| Paste | none (no bracketed paste; embedded newline = Enter) | same | same | same | same | same |
| Footer | none in modal; rename only: status bar | in form | in form | in form | in overlay | in footer line |

Key decisions that are *tables* already (good, extractable): `KeyRow<A>`, `Hint`, `resolve`, `footer`, `mislabeled` (`overlay.rs:21-141`) are generic; the per-overlay tables and `FormKey` (10 actions, shared by 3 tables) are app data. A consistency test (`consistency_tests/keys.rs`) enforces Esc / Enter / Space meaning against `MODAL_CONVENTION` (`overlay.rs:782-830`), and `overlay_tests.rs:142-154,160-195` pins footer wording and label-vs-key agreement.

---

## 5. Look and feel, sizing, theme

| Surface | Width rule | Height | Title | Border | Cursor cue | Colors |
| --- | --- | --- | --- | --- | --- | --- |
| TextInput modal | `min(60, w-4).max(20)` | 3 | per caller, inconsistent padding/case | `panel_focus_accent` | `REVERSED` cell | none besides border |
| Picker | `min(80, w-4).max(30)` | nearly full (see 2.8) | `" {title} ({shown}/{total}) "` | accent | none (no cursor in query) | accent `/`, `theme.placeholder` |
| NewMux | cap 76, floor 44 | content+2, min 6 | `" New tmux session "` | accent | black on white cell | fixed |
| MuxLaunch | cap 76, floor 44 | content+2, min 10 | `" Launch harness in new mux "` | accent | black on white cell | fixed (+ `theme.error`) |
| PinEdit | cap **84**, floor 44 | content+2, min 10 | `" Edit pin "` | accent | black on white cell | fixed (+ `theme.error`) |
| Viewer find | footer line | 1 | n/a | none | `REVERSED` block | `theme.secondary_text` DIM hint |

Form value cells use `Color::White on DarkGray` (active) / `Gray` (inactive), cursor `Black on White`, edges `Cyan` bold, valid mark `Green`, warning `Yellow` (17 literals in `form.rs`, 2 in `pin_edit.rs:798,836`). The rest of the TUI states a design rule that selection uses `Modifier::REVERSED` and theme roles (`theme.rs:9-14`, `controls.rs:1136-1138`, `search.rs`), so forms are the one family that ignores `[tui.theme]` overrides.

Glyphs: focus marker `"> "` (forms, controls, search) vs `"► "` in the worktree menu list and `icons::marks::POINTER`; `[x]`/`[ ]` hard-coded in 4 places; scroll marks `<` / `>`; validity `√` `!` `?` (only `√` has a registry entry); derived lead-in `└`.

Width computation `min(cap, area.width-4).max(44) - 2` is written out in each of the three form widgets (`new_mux.rs:176-177`, `mux_launch.rs:502-503`, `pin_edit.rs:642-643`) and again inside `form::modal_rect` (`:635`). At the 44-cell floor the mux-launch path value width is `42 - 15 - 32 = -5`, saturating to 1 cell; the 32-cell path suffix then overflows the row **(by reading)**.

---

## 6. A generic Form and which existing surfaces collapse onto it

Sketch (names provisional, for the control-set checkpoint):

- `text::LineEdit` - pure editing state (value, cursor, `visual_cursor`), `handle(KeyEvent) -> Edit { changed, consumed }`; no Enter / Esc / Tab semantics. Wraps `tui-input`.
- `text::TextField` - inline renderer of a `LineEdit`: one windowing algorithm (display-width aware, optional edge marks, optional prefix glyph such as `/`), cursor cue from theme, empty placeholder.
- `text::TextPrompt<M>` - modal host: title, `LineEdit`, 3-row frame, optional footer, Enter / Esc / Ctrl-C / Tab-swallow contract, `on_confirm: Fn(String) -> PromptResult<M>`; `PromptResult { Commit(M), Reject(String), Stay }` replaces the three ad-hoc empty-input behaviors.
- `path::PathField` - `LineEdit` + `PathSource` trait + `PathKind` + candidate ranking + validity mark + `complete()` + inline hint.
- `form::Form<M>` - ordered `Vec<Field>`; a `FieldKind` enum: `Text { validator }`, `Path { source, kind }`, `Choice { options, allow_custom }` (harness), `Toggle`, `CheckGroup`, `OneOf`, `Derived` (read-only), `Note`; per-field `visible_if` and `tab_stop`; optional `Override { inherited, source_word, hint }` decorator; `FormKeys` (the `FormKey` action set + table) generating the footer; `submit_policy` (Enter-from-any-row vs Enter-advances); `on_submit: Fn(&Values) -> Result<M, String>`; layout = grid `{marker, label, value, tag?}`, `ScrollBody` / `SplitBody`, error line, detail lines, pinned footer.

Collapse map:

| Existing surface | Becomes | What stays in Conspectus |
| --- | --- | --- |
| Rename (session / mux), pin rename | `TextPrompt<Msg>` | title strings, the `SlotPrompt` routing (should vanish: two distinct `on_confirm` closures) |
| Worktree branch prompt | `TextPrompt<Msg>` inside the worktree menu mode machine | repo-root capture, menu |
| Controls max age / contains | `TextPrompt<Msg>` with a validating `on_confirm` (fixes the silent failure) | `parse_max_age`, filter plumbing |
| Search / picker query | `TextField` with prefix glyph | ranking, results |
| Viewer find | `TextField` / `TextPrompt` variant | find logic |
| New mux | `Form` with `Text` + `Path` | `Msg::CommitMuxNew` mapping, cwd/name seeding |
| Mux launch | `Form` with all kinds incl. conditional fields, `Derived` command row, `OneOf` pin | field conditions, `MuxLaunchRequest`, `LaunchDefaults` preview |
| Pin edit | `Form` with `Override` decorator, `Note` rows, detail lines, `CheckGroup` | field map by `PinKind`, layer words, `request()` to `set`/`unset`, `validate_id` |
| `LaunchSpecFormState` | dissolves into the form's values | argv option logic (`discovery::harness`) |

Behavior changes that unification would force (each needs a regression / consistency-win call):

1. **Enter**: NewMux advances then commits; MuxLaunch and PinEdit submit from any row. Either NewMux becomes submit-from-anywhere, or all forms adopt "advance until last row". Footer wording changes either way (`Enter next/commit` vs `Enter launch` / `Enter save`).
2. **Up vs S-Tab**: MuxLaunch distinguishes row ring (Up/Down) from stop ring (Tab/S-Tab, includes the socket checkbox); NewMux and PinEdit have one ring. A generic form needs a `tab_stop` flag per field and a rule for "two controls on one row".
3. **Validation timing and place**: NewMux silent no-op; MuxLaunch error after Enter (blank line + unwrapped error line, `mux launch:` prefix); PinEdit error under the detail lines, truncated; branch prompt silent; max age silent; pin rename toast. Unifying means an error line in every prompt and form (new rows in 3 modals' heights).
4. **Footers**: forms embed a wrapped footer; prompt modals have none (rename hint goes to the status bar; branch / max-age / contains have no hint). Unifying adds a footer row to the 3-row prompt (height 3 -> 4-5) or standardises on status-bar hints.
5. **Path hint**: inline 32-cell `Tab: x` (new mux, mux launch) vs mark only plus detail line (pin edit). One rule needed; the inline hint costs 32 cells of value width.
6. **Footer pinning**: NewMux and MuxLaunch footers scroll away on short terminals; PinEdit pins them.
7. **Colors**: moving 19 literals to theme roles changes the look under custom themes (consistency win, but visible).
8. **Cursor cue and empty value**: black-on-white cell vs `REVERSED`; `-` placeholder vs blank. Pick one.
9. **Modal widths**: 60 / 76 / 80 / 84 caps with floors 20 / 30 / 44.
10. **Labels**: `mux name` vs `name`, `mux socket` vs `socket`, `launch argv` vs `argv`, `branch` vs `worktree_branch` for the same fields in two forms; pin edit shows raw field names.
11. **Radio vs checkbox glyph**: `[x]` for both today.
12. **Harness row arrows**: Left / Right cycle (cursor cannot move by arrow); keep or give the harness a separate "choose" mode.
13. **Esc with unsaved edits** discards silently everywhere; consistent today, so keep unless the operator wants a dirty prompt.
14. **Esc in the branch prompt** returns to the menu; rename closes. Needs a stack rule (Esc pops one level) - arguably already the convention if the menu is modeled as a stack.
15. **Ctrl-R**: reset-to-inherited (PinEdit) vs earlier-search (Search). The framework should treat these as per-component bindings, not a global.

---

## 7. Duplications and inconsistencies

Counts and locations are exact unless marked.

**D1. Text-edit engines: 2.** `tui_input::Input` through `TextInputState` (10 surfaces) vs hand-rolled `FindPrompt` (`viewer/find.rs:46-113`), with divergent keys (`Ctrl-U`: whole line vs before cursor; no word motion in find).

**D2. Renderers of one editing state: 4.** (1) `TextInputBody` (`input.rs:149-162`, REVERSED cell, cursor pinned right), (2) `form::window` (`form.rs:101-159`, edge marks, black-on-white cursor), (3) read-only `"/" + text` lines (`search.rs:821`, `picker.rs:217`, no cursor), (4) `draw_find_prompt` (`viewer/widget.rs:492`, width-aware).

**D3. Windowing algorithms: 3** (`input.rs:179`, `form.rs:101`, `viewer/widget.rs:492-545`); only the last is display-width aware, though `tui-input` ships `visual_cursor` / `visual_scroll` and `unicode-width` is already a dependency.

**D4. Outcome mapping of `InputOutcome::Confirm`: 7 consumer match sites** (`modal.rs:197`, `worktree_menu.rs:405`, `controls.rs:490,509`, `pin_edit.rs:559`, `mux_launch.rs:478`, `new_mux.rs:155`) plus the producer (`input.rs:75`). In the three forms `Confirm` is unreachable because `FormKey::Submit` is resolved first (`new_mux.rs:154`: "Enter is intercepted above; kept for safety"). The forms resolve their own table and then `TextInputState::handle_key` resolves `TEXT_INPUT_KEYS` a second time on the same event.

**D5. "Did the value change?" probes: 3** (`path_omnibox.rs:117-124`, `search.rs:537-541`, `pin_edit.rs:544-555`) copy the value string before the key and compare after; `TextInputState` should report `changed`. The picker skips the check and refilters on every key (`picker.rs:157-160`).

**D6. Per-modal plumbing copy-paste: 5 handlers** in `runtime/overlay_keys.rs` (rename `:72-98`, worktree `:103-129`, new mux `:131-155`, mux launch `:268-292`, pin edit `:605-634`), each the same five-arm outcome match differing in the cancelled-status string; plus the accessor quartets (`x`, `x_mut`, `open_x`, `close_x`) in `app/overlays.rs` and the dispatch ladder in `runtime.rs:880-940`. (Layer 3, but it multiplies per form.)

**D7. Row-body types: 3 near-copies** of "Vec<Line> in a popup with a scroll offset": `form::ScrollLinesBody` (`:642-665`), `controls::ControlsBody` (`controls.rs:1101-1107`), `popup_frame::LinesBody` (no scroll). Plus `scroll_offset_for_cursor` twice (`form.rs:724`, `controls.rs:1109`, Option variant) and `compute_scroll` twice (`search.rs:1423`, `multi_select.rs:295`).

**D8. Harness / launch-option logic: 2 copies.** `cycle_harness` + `toggle_launch_option` + default-argv resolution in `launch_spec_form.rs:198-312` vs `pin_edit.rs:371-464` (about 60 duplicated lines, including the `[only], -1|1` step rule); `default_argv` has three definitions (`form.rs:543`, `mux_launch.rs:220`, `pin_edit.rs:371`). Harness-row rendering: `form::harness_row` (`:325-362`) vs `pin_edit::render_row` Harness arm (`:786-813`), differing in constants (slot 18, chip cap 48).

**D9. Shell-style argv code: 2 tokenizers and 3 quoters that disagree.** Tokenizers: `launch_spec_form::parse_launch_argv` (`:390-453`, strict: errors on an unclosed quote or trailing backslash, keeps `\x` inside double quotes unless `x` is `"` `\` `$` or backtick) and `argv::split_command_line` (`argv.rs:37-85`, lenient: swallows an unclosed quote, drops a trailing backslash, drops the backslash before any char inside double quotes). Pin edit uses the first to toggle options and compare defaults (`pin_edit.rs:384,439`) but saves through `pin_ops::parse_override` -> the second (`pin_ops.rs:457-459`), so text such as `"a\b"` is read one way for the toggle and another on save **(by reading)**. Quoters: `form::display_argv` / `shell_display_arg` (`form.rs:614-631`, safe set `_-./:=+`), `argv::shell_quote` (`argv.rs:10-24`, safe set `_@%+=:,./-`, special-cases `;`), `cli/hook.rs:723-731 shell_quote` (safe set `/._-`). `tui/preview::split_command_line` is a re-export of the lenient one.

**D10. Path-mark duplication.** `path_suffix` (`form.rs:250-276`) and `path_mark` (`:280-287`) repeat the `(symbol, style)` match; the `√` / `!` / `?` literals bypass `icons::marks::OK` / `FAIL`.

**D11. Duplicate seeding and hints for the new-mux form.** `Msg::OpenNewMuxForm` (`app.rs:2054-2064`, `$HOME`-or-`/` fallback) and `open_new_mux_form_action` (`runtime.rs:1575-1588`, `default_new_mux_cwd`); the latter hard-codes a status string (`:1585`) that omits "S-Tab back", although ADR 0123 says hand-written copies of footers are exactly what the generated tables replace.

**D12. Modal frame size calc.** `centered_modal_rect` is defined 5 times, with differing caps (`input.rs:165`, `multi_select.rs:310`, `help.rs:882`, `search.rs:1413`, `controls.rs:1264`) plus `form::modal_rect`; the form width expression `min(cap, w-4).max(44) - 2` appears 3 times in the form widgets and once in `modal_rect`.

**D13. Checkbox glyph literals: 4** (`form.rs:565`, `controls.rs:1194`, `search.rs:1068`, `mux_launch.rs:585-587` which hand-writes the box instead of calling `form::checkbox`).

**D14. Read-only row variants: 3** (`note_row`, `derived_row`, `warning_row`) with the same prefix + gutter + truncated-value shape; `options_row` and `choice_row` are the same widget with different input types; `tagged_input_row` and `warning_row` have no callers.

**D15. Title style.** 8 distinct prompt/form titles, mixed padded-lowercase and unpadded-capitalised (section 2.1).

**D16. Pin rename UIs: 2** (prompt vs edit-form first row), with different validation and store ops.

**D17. Status-hint vs footer: 3 mechanisms** for key hints: in-overlay footer (forms, search), status bar (rename, new mux), none (branch prompt, max age, contains).

**D18. Dead / vestigial API.** `PathOmniboxState::selected` and `MAX_CANDIDATES = 6` (only the first suggestion is used), `PathCandidate.source`, `PathSuggestion.source` / `.exists` (never rendered), `expanded_value()` (0 production callers), `tagged_input_row`, `warning_row`, the `TextInputWidget` no-theme fallback, `| Changed` in 3 Tab arms, `InputOutcome::Confirm` arms in 3 forms.

**D19. `~` expansion: 3 implementations.** `path_omnibox.rs:289-307`, `legacy_pins.rs:163-174`, `config.rs:1724-1731`; none applied on form commit.

**D20. Stale documentation.** ADR 0097 describes `PinCreateState`, a harness-first field order, and a `LaunchSpecFormWidget` that were never or are no longer built; `modal.rs:159-163` still mentions "H-EXT config editing surfaces"; `input.rs:1-8` says "the inline mux picker shares this primitive" (it does not).

**D21. Risks shared by all text entry (not duplications).** No wide-character support (D3), no bracketed paste (paste containing a newline submits), Alt-B/F likely inert, per-frame filesystem I/O for path rows, validation of `exists` not `is_dir`, silent invalid-input paths (max age, branch prompt, new mux).

**D22. Look vs theme.** 19 hard-coded colors in form files; `[tui.theme]` overrides do not reach forms.

---

## 8. Tests that anchor this slice

| Area | Where | Kind |
| --- | --- | --- |
| Text input | `input_tests.rs` (11) | unit; no render test of the widget itself |
| Path omnibox | `path_omnibox_tests.rs` (6) | unit with real tempdirs and real `$HOME` |
| Form rows | `form_tests.rs` (9) | unit on `Line` text |
| Launch spec | `launch_spec_form_tests.rs` (12) | unit |
| New mux | `new_mux_tests.rs` (14) | unit on key sequences + a rendered-text check |
| Mux launch | `mux_launch_tests.rs` (29) | unit + 3 insta snapshots (`src/tui/widgets/snapshots/*mux_launch*`) |
| Pin edit | `pin_edit_tests.rs` (22) | unit + rendered-text column/jargon assertions; no snapshot |
| Cross-form | `consistency_tests/forms.rs` (7), `keys.rs` (modal convention), `layout.rs` (selection movement leaves text alone; modal fits), `text.rs` (no developer artifacts on screen) | consistency harness |
| Key tables | `keybindings/overlay_tests.rs:142-195` | footer wording and label-vs-key |
| Pantry | `examples/pantry.rs:377-420,707-725` | 3 TextInput variants only; no forms, no omnibox, no query lines |

Gaps for extraction: no snapshot for the text modal, new mux, pin edit, path row with hint, or the harness row at narrow widths; no test of conditional-row height changes; path tests depend on the host filesystem and `$HOME`.

---

## 9. Framework verdicts

| Control / primitive | Candidate base-control name | Verdict | Merges with |
| --- | --- | --- | --- |
| `TextInputState` core | `LineEdit` | extract after refactor (add `changed`, `visual_cursor`; split off Enter/Esc contract) | `FindPrompt` engine |
| `TextInputWidget` + `RenameOverlayState` | `TextPrompt<M>` | split generic core + adapter (`on_confirm` closure replaces `Msg::CommitRename` wrapper) | worktree branch prompt, Controls max-age / contains editors |
| `form::window` / `value_row` / `input_row` | `TextField` (inline) | extract after refactor (theme roles, display width) | modal `visible_window`, query lines, `draw_find_prompt` |
| `PathOmniboxState` + `path_row` / `path_mark` / hint | `PathField` | extract after refactor (`PathSource`, `PathKind`, resolve dead surface, ~ commit contract) | the mux-launch `repo` field; CSP-455 follow-up |
| `harness_row` / `harness_chips` + pin_edit Harness arm | `ChoiceField { allow_custom }` | extract after refactor (merge the two copies) | `choice_row` |
| `options_row` + `choice_row` + worktree toggle + socket checkbox | `CheckGroup` / `OneOf` / `Toggle` (one kit) | extract after refactor | Controls `checkbox_row`, search options pane |
| `note_row` / `derived_row` / `warning_row` | `ReadOnlyRow { tone }` | extract after refactor | detail-line helpers |
| `Cols`, `prefix`, `entry_style`, `fit`, `wrap_words`, `wrap_footer` | grid / text helpers | extract as-is (after theme-role swap) | `popup_frame::ellipsize`, `ui::text` truncators |
| `ScrollLinesBody` + `SplitBody` + `scroll_offset_for_cursor` + `modal_rect` | `FormBody` | extract after refactor (unify with `ControlsBody`, `LinesBody`) | other modals' size helpers |
| `FormKey` + form key tables + `KeyRow` machinery | `FormKeys` | generic machinery extract as-is; concrete tables stay | `TEXT_INPUT_KEYS`, `OMNIBOX_KEYS` |
| `NewMuxFormState` | (instance of `Form`) | collapses onto Form | - |
| `MuxLaunchFormState` | (instance of `Form`) | split generic Form + adapter | - |
| `PinEditFormState` | (instance of `Form` + `OverrideRow`) | split generic Form + adapter | - |
| `LaunchSpecFormState`, `default_argv`, `parse_launch_argv` | n/a | stays in Conspectus (argv option logic); reconcile tokenizers | `argv::split_command_line` |
| Search / picker query lines | `QueryLine` = `TextField` + prefix glyph | extract after refactor | `TextField` |
| Viewer `FindPrompt` | n/a | stays with viewer until viewer sits above the framework; then drop for `LineEdit` | `LineEdit` |

---

## 10. Open questions for the operator

1. **Enter in multi-field forms.** NewMux uses Enter-advances-then-commits; MuxLaunch and PinEdit submit from any row. Which is the standard? (This changes one or two forms' footers and muscle memory.)
2. **Tab vs arrows.** Should Tab visit the same stops as Up / Down? Today MuxLaunch's socket checkbox is a Tab-only stop; the others have one ring.
3. **Where do validation errors go**, and when do they appear (on Enter only, or live under the field)? Today it is three different answers plus silent failures (branch prompt, max age, new mux).
4. **Key hints in single-field prompts.** Add a footer to the 3-row prompt (taller modal), standardise on status-bar hints, or keep none?
5. **Theme.** Is it acceptable for forms to start honoring `[tui.theme]` (visible change for anyone with custom colors)? Which semantic roles replace Cyan / Green / Yellow / DarkGray / White?
6. **Path semantics.** Should path fields be directory-only for cwd (stop suggesting files, validate `is_dir`)? Should the committed value be home-expanded? (By reading, `~/x` currently passes validation and is sent unexpanded to a CLI that does not expand it.)
7. **Path candidates UI.** Keep "single best remainder" or grow into the CSP-455 follow-up (visible ranked list, cycle with repeated Tab)? That decides whether `selected` / `source` / `exists` are kept or removed.
8. **Unicode.** Is width-correct editing for CJK / emoji required? (Cheap with `visual_cursor`; changes all windows.)
9. **Paste.** Enable bracketed paste so a multi-line paste cannot submit a form?
10. **Word motion.** ADR 0030 promised Alt-B / Alt-F; they probably do not work. Fix (map ALT in a wrapper), document Ctrl-Left / Ctrl-Right, or drop the claim?
11. **Multi-line (CSP-430).** Is `tui-textarea` (or an equivalent) in scope for the framework's first cut, or explicitly deferred?
12. **Idiom for an editable value in a list.** Controls uses "[ Edit ]" rows that open a prompt; forms edit in place. Should the framework offer both, or move Controls filters to in-place fields?
13. **Minibuffer (CSP-393 / CSP-394).** Planned to host rename / search / filter prompts in a bottom bar. Should `TextPrompt` be designed host-agnostic (modal now, minibuffer later)?
14. **Pin rename duplication.** Keep both the prompt and the edit-form row, or consolidate?
15. **Esc discards silently** in all forms with unsaved edits. Keep?
16. **Labels.** Adopt one vocabulary (`mux name` vs `name`, `launch argv` vs `argv`, `branch` vs `worktree_branch`) and stop showing raw field names in pin edit?
17. **Radio vs checkbox glyphs** for one-of-N (e.g. `(o)` / `( )`) - currently identical `[x]`.
18. **Retire `LaunchSpecFormState` / ADR 0097** when mux launch moves to the generic Form? (Its premise, a second consumer, is gone.)
