# Inventory 04: Menus, Pickers, Selection Lists, Dialogs, Toasts

Slice: `src/tui/widgets/{controls,multi_select,picker,row_menu,mux_menu,worktree_menu,clean_up,value_modal,messages,toast}.rs`
and their sibling `*_tests.rs`, `src/tui/actions.rs` (menu construction), the shared
primitives those widgets stand on (`popup_frame.rs`, `modal.rs`,
`keybindings/overlay.rs`, `cursor.rs`, `input.rs` as the text-prompt dialog), and
the cross-control tests in `src/tui/consistency_tests/{menus,keys,layout,actions}.rs`.

Method: read-only source reading in the `worktree-tui-framework` checkout. Nothing was
run (no cargo, no snapshots, no tmux), so every behavioral claim below is "from
source"; the few that read like defects are marked **(unverified at runtime)**.
Line numbers are for this worktree at `ca668a3`.

Classification codes used in the coupling tables:

- **(a)** parameterizable inside the framework (generic type parameter, closure, trait).
- **(b)** belongs in a Conspectus-side adapter outside the framework.
- **(c)** inherently domain; it stays with the app and is not part of any control.

---

## 1. Census: every menu, list-selection, dialog, and toast surface

| # | Surface | Interaction class | Opened by | File (production LOC / test LOC) | Overlay trait? |
|---|---|---|---|---|---|
| 1 | Row action menu (`.`) | Action menu, optional inline confirm | `Msg::OpenRowMenu` (`app.rs:1880`), `Msg::TogglePin` with several pins (`app.rs:1926` via `app/overlays.rs:56`) | `row_menu.rs` 188 / 72 | yes (`Ctx=()`) |
| 2 | Mux action menu (`m`) | Action menu (2 fixed verbs) | `runtime.rs:1786` | `mux_menu.rs` 156 prod + 43 inline tests | yes (`Ctx=()`) |
| 3 | Worktree menu (`w`, `X`) | Action menu, then a small mode machine: text prompt, y/n confirm x3, two-verb choice | `runtime.rs:1495,1529` | `worktree_menu.rs` 677 (about 215 of it is graph querying) / 365 | yes (`Ctx=()`) |
| 4 | Picker (retarget, search history) | Single-select list with type-to-filter | `app.rs:1948`, `app/content.rs:1099` | `picker.rs` 277 / 105 | yes (`Ctx=()`) |
| 5 | Multi-select checklist (sub-editor) | Multi-select checklist | `controls.rs:543,555,567` | `multi_select.rs` 322 / 117 | **no** (own `handle_key`) |
| 6 | Clean up modal | Multi-select checklist + empty-state notice | `Msg::OpenCleanUp` (`app.rs:2042`) | `clean_up.rs` 144 / 76 | yes (`Ctx=()`) |
| 7 | Controls overlay (`c`/`f`) | Sectioned settings panel with sub-editors; applies as you go | `App::open_controls_overlay` | `controls.rs` 1344 / 914 | yes (`Ctx=&ControlsContext`) |
| 8 | Controls sub-editor: harness / mux state / PR state | Multi-select (surface 5) | from 7 | in `controls.rs` | no |
| 9 | Controls sub-editor: max age / contains | Text prompt (single line) | from 7 | in `controls.rs` | no |
| 10 | Value modal (`o`, rebuild plan) | Read-only scrolling inspection modal, optional copy | `app.rs:773`, `runtime/launch.rs:688,715` | `value_modal.rs` 239 / 136 | yes (`Ctx=()`) |
| 11 | Messages overlay (`!`) | Scrolling log list with detail pane | `App::open_messages_overlay` | `messages.rs` 251 / 90 (+ model `tui/messages.rs` 281) | yes (`Ctx=&MessageLog`) |
| 12 | Rename / pin-rename prompt | Text-prompt dialog | `runtime/overlay_keys.rs:8-66`, `app/overlays.rs:42-53` | `input.rs` 204 + `modal.rs` `RenameOverlayState` | yes via wrapper |
| 13 | Worktree branch prompt | Text-prompt dialog inside surface 3 | `worktree_menu.rs:350` | in `worktree_menu.rs` | no |
| 14 | Worktree confirm: remove / merge / prune | Confirm dialog (y/n) | surface 3 modes | in `worktree_menu.rs:419-480,561-653` | no |
| 15 | Worktree close-down choice | Two-verb choice dialog (`m` / `d`) | `X` hotkey or menu entry | in `worktree_menu.rs:482-504` | no |
| 16 | Armed Forget entry | In-place two-step confirm | surface 1 | `row_menu.rs:106-111` | n/a |
| 17 | Armed Backspace in explorer | In-place two-press "leave right pane" gate (not destructive) | `app/explorer_nav.rs:324-350` | flag `explorer_back_armed` (`app.rs:156,1415`) | n/a |
| 18 | Toast | Transient feedback blip | `Effect::Toast` (40 call sites) and `App::post_toast` (12 call sites) | `toast.rs` 102 prod + 72 inline tests | n/a (not a modal) |

Out of this slice but adjacent (other inventories own them): Help, Search (quick select and
contents), Viewer, PaneViewer, NewMux / MuxLaunch / PinEdit forms. They matter here only where
they set the pattern the in-slice controls are compared against.

Notes on what does not exist: `pending_pin_remove` (named in `docs/tui-architecture-review.md`)
is gone; the Pins modal it belonged to was replaced by the row menu (ADR 0144). `Unpin` from
the menu has no confirm. There is no `Delete`-twice path left. The only armed flags in the
tree are the row-menu `armed: Option<usize>` and `explorer_back_armed: bool`.

There is **no mouse handling anywhere in the TUI** (`grep` for `MouseEvent`/`EnableMouseCapture`
finds nothing), although ADR 0031 section 6 lists "mouse-click support when the terminal
supports it" as part of the Controls decision.

---

## 2. Shared primitives these controls stand on

### 2.1 `popup_frame.rs` (127 LOC) - the framing helper

| Item | Where | What |
|---|---|---|
| `centered_rect(area, w, h)` | `popup_frame.rs:43` | Pure centering math. Every overlay computes its own `w`/`h` and calls this (CSP-464). |
| `themed_popup(body, title, theme)` | `:58` | `tui_popup::Popup` with `Borders::ALL`, border style `fg(theme.panel_focus_accent)`; title unstyled. Body must be `KnownSize + Widget`. |
| `LinesBody` | `:75` | Reusable `Vec<Line>` body. **No caller anywhere** (`grep` over `src`, `examples`, `tests`: only its own definition). Each modal instead declares its own `*Body` struct with copy-pasted `KnownSize` impl (`inner_width`/`inner_height` fields): `MultiSelectBody`, `PickerBody`, `ValueModalBody`, `MessagesBody`, `ControlsBody`, `TextInputBody`. |
| `wrapped_lines(text, indent, width)` | `:99` | Word wrap with indent. **Leak:** calls `crate::viewer::render::word_wrap` (the viewer module). |
| `ellipsize(text, width)` | `:110` | Cell-width-aware cut ending in `…`. Used by RowMenu only in this slice. |

`tui_popup::Popup` is exactly `Clear` + `Block` (borders, border style, title, style) + body,
centered from `KnownSize` (verified in `tui-popup-0.7.6/src/popup.rs:181-193`). The four
hand-rolled menus (RowMenu, MuxMenu, WorktreeMenu, CleanUp-empty) do `Clear.render` +
`Block::default().borders(ALL).title(..).border_style(fg(panel_focus_accent))` + `Paragraph`
by hand. Same pixels, no shared code. CSP-428 (done) claimed "every centered-bordered-modal now
routes through tui_popup"; that is not true of these four (and of the forms/Help/Search in other slices).

### 2.2 `modal.rs` (203 LOC) - the overlay contract (ADR 0085 contract 3)

- `Modal` enum, 16 variants (`modal.rs:32-96`). `App.modal_stack: Vec<Modal>`.
- `OverlayOutcome { Consumed, Commit(Box<Msg>), CommitAndStay(Box<Msg>), Close }` (`:107-122`).
  `Box<Msg>` because `Msg` is over 256 bytes.
- `trait Overlay { type Ctx<'a>; fn handle(&mut self, ctx, key) -> OverlayOutcome }` (`:137-150`).
  Rendering is *not* in the trait: each overlay has a separate `*Widget` struct plus a
  `draw_*` fn in `ui.rs:88-106` that peeks the top of the stack.
- Dispatch: `runtime.rs:887-945` `overlay_key_from_event` is a 16-branch `if app.x().is_some()`
  chain (one per `Modal` variant); because every accessor checks `modal_stack.last()`, at most one branch can hit, so the
  chain is a hand-written `match` on the top modal.
- Only the **top** modal is drawn and handled. No code path observed in this slice pushes a
  second modal over a live one: every commit pops first (`overlay_keys.rs` handlers), then
  dispatches the Msg, and the reducer opens the next. The "stack" is de facto a single slot
  plus pop-then-push chaining. **(from source; not exhaustively proven)**
- Cost of adding one overlay, measured with `grep -w` on RowMenu / Picker / CleanUp / MuxMenu /
  ValueModal: each is named in 6-8 production files besides its own widget file (`modal.rs`
  variant, `keymap.rs` `Action::XKey` variant, `runtime.rs` dispatch arm + `overlay_key_from_event`
  branch + a `matches!` list at `:788-797`, `runtime/overlay_keys.rs` handler fn, `app/overlays.rs`
  4 accessors `x`/`x_mut`/`open_x`/`close_x`, `ui.rs` `draw_x`, `keybindings/overlay.rs` key table +
  `MODAL_CONVENTION` row, and the opener in `app.rs`/`actions.rs`/`runtime.rs`) and in 2-3
  consistency-test files (`layout.rs`: `name_of` arm, `builders()` entry, `press` arm; `keys.rs`:
  `tables()` arm; sometimes `text.rs`).
- Runtime handlers: `overlay_keys.rs` contains 13 near-identical
  `handle_*_key` functions (rename, worktree, new-mux, mux-menu, row-menu, clean-up, picker,
  mux-launch, value-modal, messages, help, controls, pin-edit) plus search: each matches the
  four `OverlayOutcome` arms. Differences are only (i) pop fn, (ii) whether `Close` posts a
  `"... : cancelled"` status (6 do: rename, worktree, new-mux, mux-menu, mux-launch,
  pin-edit; 8 do not: row-menu, clean-up, picker, value-modal, messages, help, controls,
  search), (iii) `dispatch` vs `dispatch_live`.
- The generic part of this contract (outcome enum, trait, stack, dispatch) is layer 2; it has to become
  generic over the message type `M` and over a registry instead of an enum of Conspectus states.

### 2.3 `keybindings/overlay.rs` (834 LOC) - key tables that generate footers (ADR 0123)

Generic machinery (about the first 140 lines): `Hint {Shown{label,verb}, Alias}`,
`KeyRow<A> {Bind{key,action,hint}, Note}`, `resolve(rows, event)`, `footer(rows)` (segments joined by
` · `, rows that share a `verb` merge labels with `/`), `mislabeled(rows)` +
`label_names_key` (test hook that a shown label names the key it binds). Builders `key/ch/ctrl/shown/alias`.
Depends on `super::KeyMatcher` and `super::key_label` (`keybindings.rs:65,566`), which are also generic.

Domain-free tables in this slice: `MENU_KEYS` (`:229`), `CONTROLS_KEYS` (`:259`),
`MULTI_SELECT_KEYS` (`:290`), `TEXT_INPUT_KEYS` (`:320`), `PICKER_KEYS` (`:375`),
`VALUE_MODAL_KEYS` (`:463`), `MESSAGES_KEYS` (`:503`), `CONFIRM_KEYS` (`:540`),
`CLOSE_DOWN_KEYS` (`:560`). Every table binds `Ctrl-C` first to an `Abort` action.
`MODAL_CONVENTION` (`:782-830`) states per modal what Enter and Space mean
(`EnterMeans {Submits, Activates, Nothing}`, `SpaceMeans {Toggles, Pages, Types}`) and an
`exception` reason; `consistency_tests/keys.rs` checks each real key table against it.

Gap (verified): `MULTI_SELECT_KEYS` and `TEXT_INPUT_KEYS` carry `shown` hints (Space toggle, Enter
confirm, Esc cancel) but **no widget renders `footer()` for them**; the hints exist only in the
table (and, for text input, in the status bar at rename open, `overlay_keys.rs:62-65`).
`PICKER_KEYS` is all aliases, so the picker has no footer by construction.

### 2.4 `cursor.rs` (52 LOC)

`wrap_step(index, len, delta)` and `clamp_step(index, len, delta)`. Generic. Adoption is partial:
`wrap_step` is used by Controls, Picker, and the form/search code; `clamp_step` only by explorer and
tree. RowMenu, MuxMenu, WorktreeMenu, Messages hand-roll `saturating_sub` / `(c + 1).min(last)`.

### 2.5 Third-party crates in this slice

| Crate | Version (Cargo.toml) | Used by | How wrapped | Notes |
|---|---|---|---|---|
| `tui-popup` | 0.7 (`:130`) | all `themed_popup` users; `input.rs` and `multi_select.rs` also construct `tui_popup::Popup::new(body)` directly in an unthemed fallback branch | thin helper `popup_frame::themed_popup`; per-modal `KnownSize` body wrappers | `PopupState` area is `pub(crate)` upstream, so the in-tree absolute width caps are reproduced by reporting `cap - 2` from `KnownSize` (CSP-428). No drag/reposition. |
| `ratatui-cheese` | 0.7 (`:128`) | `multi_select.rs` only | **shim** (`MultiSelectState` wraps `CheeseMultiSelectState`; `MultiSelectWidget` builds `CheeseMultiSelect` per frame) | See 3.5: needs a per-frame "mirror" state and a hand-written `Clone`; theme bridge `cheese_styles_from_theme` maps 4 theme fields onto 8 upstream style slots, 3 of which (`title`, `description`, `unchecked`) stay default. Unused upstream features: `limit`, validators, `enabled` (disabled rows), `select_all`/`deselect_all`, `description`. |
| `ratatui-comfy-toaster` | 0.4, no default features (`:129`) | `toast.rs` only | **shim** (`engine()`, `builder_for()`, `engine_dismiss_all()`, `ToastEngineHolder` newtype for `Debug`) | Upstream hardcodes border color from `ToastType` (Success = green); Conspectus theme cannot recolor it (CSP-427 known limitation). Unused upstream: Info/Warning/Error types, titles, progress bar, click/shortcut handling, `set_area_avoiding`, queue depth. |
| `tui-input` | 0.15 (`:145`) | `input.rs` | direct, behind `TextInputState` | ADR 0030. |
| `ratatui` | 0.30 | all | direct | `List` / `ListState` are used **nowhere** in `src/tui` (grep: 0 hits). Every list is a hand-built `Paragraph` of `Line`s. |

CSP-433 (rat-widget kit spike) was a no-go; these three small crates were adopted piecemeal in CSP-426/427/428.

---

## 3. Per-control entries

### 3.1 Row action menu - `row_menu.rs` (188 + 72 tests)

**Purpose.** Verb list for the selected entity; Enter commits one `Msg`; an entry marked `confirm`
needs a second Enter.

**Public surface.**
- `RowMenuEntry { label: String, subtitle: String, msg: Msg, confirm: bool }`; `new(label, subtitle, msg)`; `.confirming()` (`:24-48`).
- `RowMenuState::new(title, entries)` (`debug_assert!(!entries.is_empty())`), `title()`, `entries()`, `cursor()`, `armed() -> Option<usize>` (`:51-86`).
- `impl Overlay for RowMenuState { Ctx = () }` (`:88-117`); `RowMenuWidget::new(&state, &Theme)` (`:119-185`).

**Variants / callers.** Single widget, three construction sites:
- `actions.rs:679-773` `row_menu_for(app)`: builds entries from graph facts. Entries and their lines: `Pin` (`:723`), `Rebuild` (`:728`), `Show rebuild plan` (`:740`), `Edit…` / `Rename…` / `Retarget…` / `Refresh snapshot` / `Reveal in its view` (`pin_management_entries`, `:794-824`), `Unpin <name>` (`unpin_entry` `:775`, used `:750,:825`), `Forget` + `.confirming()` (`:762`, the only confirming entry in the tree), `Clean up…` (`:765`). Title from `menu_title` (`:828-858`): `pin <n>`, `<kind> <label>`, `mux <id>`, `session <8ch>`, `PR #<n>`, fallback `Actions`.
- `app/overlays.rs:56-72` `open_pin_menu`: title `Pinned`, one `Unpin <slot>` entry per pin when several pins point at one entity.
- Tests (`row_menu_tests.rs`), `ui_tests.rs` snapshots `slot_row_menu` / `history_row_menu`, `consistency_tests/menus.rs`.
Differences between instances: only title and entry list. Entries differ in `confirm` (1 of ~12 distinct entries).

**Coupling.**
| Import / content | Class | Note |
|---|---|---|
| `crate::tui::Msg` (field `msg`, `Overlay` return) | (a) | Make state generic over the commit payload `M`, or store `FnOnce`-free values; `OverlayOutcome<M>`. |
| `crate::tui::modal::{Overlay, OverlayOutcome}` | (a) | Moves with the layer-2 contract. |
| `crate::tui::theme::Theme` (`panel_focus_accent`, `error`) | (a) | Framework theme tokens. |
| `popup_frame::{centered_rect, ellipsize, wrapped_lines}` | (a) | `wrapped_lines` leaks `viewer::render::word_wrap` - needs a framework text util. |
| `keybindings::overlay::{MENU_KEYS, footer, resolve}` | (a) | |
| `actions::row_menu_for` (doc link only) | (b) | The entry builder stays in Conspectus. |
| Strings: `" — press Enter again to confirm"` (`:141`) | (a) | Generic; should be a framework string with its key taken from the table, not hand-typed. |
| Subtitle `"pin this (p)"` etc. (`actions.rs:723`) | (b) | The only way a key accelerator is shown in a menu is as free text in a subtitle. |

**Look and feel.** Frame: hand-rolled `Clear`+`Block` titled `" {title} "` (no accent on title), border `fg(panel_focus_accent)`.
Width `min(72, area.width)` (note: not `width - 4`), height `lines+2` clamped `(8, area.height)` (`:169-173`).
Body: blank line; per entry a label row (`►` or space, one space, label, `ellipsize`d to inner width) then subtitle wrapped with a 5-space indent (`wrapped_lines(.., 5, ..)`); blank; wrapped footer from `MENU_KEYS` = `↑/↓ move · Enter pick · Esc close`.
Focused label: `BOLD`, no reverse. Armed: label gets `" — press Enter again to confirm"` appended and the row is `fg(error)`+`BOLD`. No border change when armed. No scroll, no scroll indicator, no disabled entries, no sections, no in-row shortcut column.

**Keys** (`MENU_KEYS`): `↑`/`k` up, `↓`/`j` down (clamped, no wrap), `Enter` pick, `Esc` close, `Ctrl-C` abort (== close). Moving the cursor disarms. Unbound keys are swallowed (`None => Consumed`). No `q`, `g/G`, `Home/End`, `PgUp/PgDn`, digits, `Ctrl-N/P`.

**Tests.** Unit: plain commit, armed needs 2nd Enter, moving disarms, Esc closes and cursor clamps. Snapshot: 2 `insta` frames at 100x24. The `history_row_menu` baseline shows a 10-entry menu **clipped by the frame: the footer row is not visible** and nothing scrolls (see D18).

**Verdict: extract after refactor** as the framework's base `ActionMenu<M>` (see 4.1). Needs: generic payload, scrolling that keeps the cursor visible, safe height clamping, the shared popup frame, the shared cursor/selection style, and the confirm idiom decided (D10).

---

### 3.2 Mux action menu - `mux_menu.rs` (156 prod + 43 inline tests)

**Purpose.** `m` menu fronting two verbs. A hard-coded two-entry specialization of the row menu.

**Surface.** `MuxMenuChoice {NewBareMux, LaunchHarnessMux}` with `label()`/`subtitle()`; `MuxMenuState::new()` (no args), `cursor()`, `entries()`; `Overlay` impl commits `Msg::OpenNewMuxForm` / `Msg::OpenMuxLaunchForm`; `MuxMenuWidget`.
**Caller.** One: `runtime.rs:1786`, which also sets a hand-typed status hint `"mux: ↑/↓ move · Enter pick · Esc close"` (`:1789`).

**Differences from RowMenu** (all incidental): title `" Mux "` plus an in-body BOLD heading `"Mux actions"` (`:118`) that repeats the title; width 62 vs 72; height clamp floor 10 vs 8; no `confirm`/`armed`; no ellipsis/wrapping of text; footer unwrapped; entries are an enum rather than data. Labels: `"New tmux session (bare shell)"` (opens a form, no `…`) vs `"Launch harness in new mux…"` (has `…`).

**Coupling.** `Msg::OpenNewMuxForm`/`OpenMuxLaunchForm` (c, but trivially expressible as `ActionMenu` data), `Theme`, `MENU_KEYS`, `popup_frame`.
**Keys.** Identical to RowMenu (`MENU_KEYS`).
**Tests.** 3 inline tests (Enter first, Enter second, cursor clamps) - the same three behaviors `row_menu_tests.rs` covers.

**Verdict: stays in Conspectus as *data*; the widget is deleted** in favor of `ActionMenu<M>` constructed with two entries. Visible deltas to catalog: title unchanged, heading line removed, width 62 -> shared width.

---

### 3.3 Worktree menu - `worktree_menu.rs` (677 + 365 tests)

**Purpose.** Context-sensitive list of worktree verbs that opens further steps in place.

**Two very different halves.**
1. **Domain context builder** (`:51-265`, about 215 LOC): `WorktreeMenuContext {context, repo_root, branch, guard_sessions, can_mutate, reveal_target}`, `context_for_node(snapshot, node, can_mutate)` walking the graph (`GraphSnapshot`, `SnapshotIndex`, `CheckoutNode`, `RepoId`, `path_is_ancestor_of`, `RowId`). Pure Conspectus. **(b/c)**
2. **UI state machine** (`:276-673`, about 400 LOC): `Mode {List, BranchInput(TextInputState), ConfirmRemove, ConfirmMerge, CloseDownChoice, ConfirmPrune}`; `WorktreeMenuState {ctx, actions: Vec<WorktreeAction>, cursor, mode}`; `new(ctx) -> Option<Self>` filters the policy list (`WIRED` at `:41-49`, drop mutations without `repo_root`, drop reveals without a target); `new_close_down(ctx)` opens straight into the choice mode (the `X` hotkey).

**Sub-dialog patterns inside it (these are separate controls that happen to live in one file).**
| Mode | Interaction | Keys (table) | Commit |
|---|---|---|---|
| `List` | action menu, no subtitles | `MENU_KEYS` | enters another mode, or `SelectRow` for reveal |
| `BranchInput` | text prompt "New worktree — branch"; empty trims silently stay open | `TEXT_INPUT_KEYS`; Esc -> back to List | `CommitWorktreeCreate` |
| `ConfirmRemove` | y/n with live-session guard list | `CONFIRM_KEYS` (Enter/y yes, Esc/n no) | `CommitWorktreeRemove{force}` |
| `ConfirmMerge` | y/n, same guard list | `CONFIRM_KEYS` | `CommitWorktreeMerge` |
| `ConfirmPrune` | y/n, no guard list | `CONFIRM_KEYS` | `CommitWorktreePrune` |
| `CloseDownChoice` | two-verb choice, guard list | `CLOSE_DOWN_KEYS` (m merge, d discard, Esc/q back) | `CommitWorktreeCloseDown{discard}` |

**Variants / duplication.** The three confirm handlers (`:419-480`) are identical except for the `Msg` they build; the three renders repeat the `▲ hosts N live session(s):` + `  - item` guard block (`:567-579`, `:592-604`, `:622-634`; the close-down version differs by wording "ends ... (transcripts preserved)"). Entry labels come from `WorktreeAction::label()` (`worktree_actions.rs`): `New worktree…`, `Merge back & close`, `Remove worktree`, `Close down stream…`, `Prune merged worktrees…`, `Reveal checkout`, `Reveal sessions`. Ellipsis use is inconsistent: `Prune merged worktrees…` opens a confirm and has one, `Remove worktree` and `Merge back & close` also open confirms and do not.

**Look and feel.** Hand-rolled frame titled `" Worktree "` (the same title for every mode except `BranchInput`, which delegates to `TextInputWidget` with its own title and frame); width `min(56, area.width)`; height `lines+2` clamped `(5, area.height)` (`:657-661`); `Paragraph` with `Wrap {trim:false}`. List rows: `"► "`/`"  "` + label, focused `BOLD`. **No footer in List mode** (MENU_KEYS footer is not rendered here, unlike RowMenu/MuxMenu); confirm and close-down modes render their table footer unstyled. Guard header is `BOLD` (no color).

**Keys deviation.** `Ctrl-C` (`Abort`) in any sub-mode goes **back to the list** (`:435,457,474,489`; BranchInput maps Abort->Cancel->List), while in `Controls` `Ctrl-C` closes the whole overlay even inside a sub-editor (ADR 0123 text). `Enter` is "yes" in every confirm, so Enter on the menu entry followed by a second Enter removes a worktree (the same two-Enter gesture as the row menu's armed Forget, but implemented as a separate screen with different keys).

**Coupling.** (c) `GraphSnapshot`, `NodeId`, `RepoId`, `CheckoutNode`, `RowId`, `Msg::Commit*`/`SelectRow`, `WorktreeAction` policy, strings about squash/rebase/fast-forward/worktrunk. (a) the mode machine itself: a list that can open `ConfirmDialog`, `TextPrompt`, `ChoiceDialog` screens, each a pure function of (title, body lines, warnings, keys, outcome).

**Tests.** 17 unit tests: 5 on `context_for_*` / graph helpers (domain), the rest on the flow (offered actions, create, remove forced/unforced, cancel back to list, close-down hotkey, prune, reveal).

**Verdict: split into generic core + Conspectus adapter.** Core: `ActionMenu` + `ConfirmDialog` + `ChoiceDialog` + `TextPrompt` composed as a *flow* (a menu whose pick can open a follow-up dialog). Adapter (stays): `WorktreeMenuContext`, `context_for_node`, `WorktreeAction` policy, all `Msg::CommitWorktree*`, the guard-session wording.

---

### 3.4 Picker - `picker.rs` (277 + 105 tests)

**Purpose.** Pick-one list with type-to-filter (all words must appear in label or detail, case-insensitive).

**Surface.**
- `PickerItem {label, detail, value}` (all `String`) (`:27-33`).
- `PickerPurpose {Retarget{pin}, SearchHistory}` (`:37-43`) **(leak)** and `PickerState::new(title, purpose, items)`, `.at_value(&str)`, `title()`, `query()`, `total()`, `cursor()`, `visible_items()`, `current()` (`:46-135`). `commit()` maps purpose + item to `Msg::CommitRetarget{pin,target}` or `Msg::ShowSearchHistory(usize)` (`:124-134`).
- `impl Overlay for PickerState {Ctx=()}`; `PickerWidget::new(&state, &Theme)`.

**Variants / callers.** Two: retarget (`app.rs:1948-1963`, items from `actions::retarget_candidates` `:488-552`, detail `"running"` or `"gone · <context>"`, value = node id or history record id) and search history (`app/content.rs:1099-1129`, label `"<query>"`, detail `entry.describe(now)`, value = index; opens with `at_value`).

**Coupling.**
| Item | Class |
|---|---|
| `Msg::CommitRetarget`, `Msg::ShowSearchHistory`, `PickerPurpose` enum | (a): replace with `on_pick: Fn(&Item) -> M` or an item payload `T`; the enum is the leak |
| `value: String` doubling as node id / record id / stringified index (`parse().unwrap_or_default()`) | (a): carry a typed payload |
| Empty-state text `"(nothing else of this kind to point at)"` (`:232`) | (b): caller-supplied empty message |
| `crate::tui::cursor::wrap_step`, `modal::Overlay`, `TextInputState`, `Theme`, `PICKER_KEYS` | (a) |

**Look and feel.** `themed_popup` (so `tui-popup` `Clear`+`Block`); title `" {title} ({visible}/{total}) "`; width `min(80, w-4).max(30)`; height `max_height.clamp(8, 20.max(max_height))` which is always `max_height` (the `20` cap is dead, see D20), so the picker is always nearly full height. Row 1: accent `/` then the raw query text - **no caret is drawn** even though state holds a `TextInputState` (the query cursor moves invisibly). Rows: `"> "`/`"  "`, label, then `"  · {detail}"` in `placeholder` (DIM); the **whole selected row is `REVERSED`** (marker, label, and detail). Labels and details are not truncated or wrapped (clipped by the paragraph). No footer. Stateless scroll: `first = cursor + 1 - rows` once `cursor >= rows` (`:241-245`). No scrollbar, no "n above" marker. Empty states: `(no matches)` or the nothing-to-pick note, both in `placeholder`.

**Keys** (`PICKER_KEYS`, all aliases so no footer): `↑`/`↓`, `Ctrl-P`/`Ctrl-N`, `Enter` pick, `Esc` cancel, `Ctrl-C` abort. Wraps. Everything else (including `j`, `k`, `q`, `Space`) types into the query; `Tab` is swallowed by the text input. No `PgUp/PgDn`, `Home/End`. Cursor index is *clamped* (not reset) after each refilter (`:121`), so the highlighted item changes under the cursor as the list narrows. Enter with no matches closes.

**Tests.** 7 unit tests (narrowing, detail searchable, wrap, commit, empty-filter close, Esc, cursor stays on list). No render test, no snapshot, no pantry story.

**Verdict: extract after refactor** as `FilterPicker<T>`. It overlaps heavily with the Search quick-select list (other inventory): same keys (`PICKER_KEYS` is documented as "the picker shares the list-and-query keys"), same `> ` + `REVERSED` row, same dim detail column. Decide there whether both become one `FilterList`.

---

### 3.5 Multi-select checklist - `multi_select.rs` (322 + 117 tests)

**Purpose.** Cursor + checkbox list; Enter confirms with selected indices.

**Surface.**
- `MultiSelectOutcome {Continue, Confirm(Vec<usize>), Cancel}`.
- `MultiSelectState::new(title, item_count, initially_selected: &[usize])`; `title()`, `cursor()`, `item_count()`, `selected_indices()`, `is_selected()`, `handle_key(KeyEvent)`. Hand-written `Clone` that rebuilds the cheese state (because upstream holds a `Box<dyn Fn>` validator).
- `MultiSelectWidget<'a, T: AsRef<str>>::new(&state, &[T]).theme(&Theme)`; items are passed at render so the state is item-free (index-based).
- `pub fn centered_modal_rect(area, item_count)`; `fn compute_scroll(cursor, visible, total)`; `fn cheese_styles_from_theme(&Theme)`.
- **Not an `Overlay`.** The wrapper (`CleanUpState`, `SubEditor`) translates the outcome.

**How the cheese shim works (cost of the wrapper).** Because the widget renders behind `&self` but upstream needs `&mut state`, `MultiSelectBody::render` builds an options `Vec` for only the *visible window* and a throw-away **mirror** `CheeseMultiSelectState` with shifted cursor/selection each frame (`:252-293`); scrolling is computed in-house (`compute_scroll`), not by the crate. `Clone` is rebuilt by hand. The theme bridge sets 5 of 8 style slots (`title`/`description` stay default because the frame draws the title; `unchecked` stays default so labels render as plain text). The `Option<&Theme>` field and the unthemed `tui_popup::Popup::new` branch are **dead in production**: every caller passes `.theme(..)` (callers: `controls.rs:826,836,843`, `clean_up.rs:111`).

**Variants / callers.** Four: Controls `" harness "` (items = `harness_options()` from the adapter registry), `" mux state "`, `" PR state "` (all `controls.rs`), and Clean up `"Clean up: forget these records"` (`clean_up.rs:42`; all preselected; labels are preformatted strings). Title padding differs: Controls titles carry explicit spaces, Clean up's does not.

**Coupling.** None domain: `crate::tui::icons::marks::{OK,BULLET}` (a, framework glyph tokens), `keybindings::overlay::MULTI_SELECT_KEYS` (a), `popup_frame`, `Theme`. This is the cleanest control in the slice.

**Look and feel.** `themed_popup`; width `min(60, w-4).max(20)`; height `items+2` clamped `(3, max(h-4, 3))` (grows to fit, so short lists never need scrolling); rows from cheese: cursor `>` (accent), `√` checked (accent+BOLD label), `•` unchecked, label **silently cut** at width (`truncate_to_width`, no ellipsis). No row reverse-highlight, no count of selected, no footer (the `shown` hints in `MULTI_SELECT_KEYS` are never rendered), no select-all/none, no scroll indicator, no disabled rows (upstream supports `enabled`; shim does not expose it).

**Keys** (`MULTI_SELECT_KEYS`): `↑/k`, `↓/j` (wrap, via upstream `prev/next`), `Space` toggle, `Enter` confirm (an empty selection confirms with `[]`), `Esc` cancel, `Ctrl-C` abort->cancel. Not bound: `Ctrl-N/P`, `g/G`, `Home/End`, `PgUp/PgDn`, `a`/`A` (select all/none), digits.

**Tests.** 11 unit tests incl. wrap, vim keys, toggle, out-of-range initial indices, empty list, scroll helper. Pantry has 3 stories (empty, mid-selection, large list).

**Verdict: extract after refactor, with an open build-vs-keep decision.** The shim exists to reuse about 100 lines of cursor/toggle logic, and pays for it with the mirror state, hand-written `Clone`, and an unused-features dependency. A framework `Checklist` could be written natively (about 100 LOC) and gain what the app lacks (disabled rows, count in title or footer, select all/none, ellipsis, rendered footer, shared scroll). Merge target: the same list model as `ActionMenu` / `FilterPicker` (a `ListModel` with cursor, optional checks, optional filter).

---

### 3.6 Clean up modal - `clean_up.rs` (144 + 76 tests)

**Purpose.** History view's "forget these records" checklist, all preselected.

**Surface.** `CleanUpState::new(candidates: Vec<history::reap::Candidate>, now)`; `candidates()`, `labels()`, `selected()`, `is_empty()`; `Overlay` impl: `Confirm(indices)` -> `Msg::ReapRecords(ids)`; zero selected -> `Close`. `CleanUpWidget`.

**Coupling.**
| Item | Class |
|---|---|
| `history::reap::Candidate` (`kind`, `id`, `label`, `last_seen`), `Msg::ReapRecords`, `format_relative_age` | (b) adapter builds `labels` and maps indices -> ids |
| Label format `"{kind:<8} {label} · last seen {age} ago"` (`:32-37`) | (b) |
| Empty notice text incl. `` `[history.reap_after]` `` config key and hand-typed `"Esc/Enter close"` (`:116-124`) | (b)/(c); the key hint string escapes the hygiene test (no ` · ` in it) |
| Title `"Clean up: forget these records"` | (b) |

**Look and feel.** Non-empty: exactly the multi-select modal. Empty: a separate hand-rolled frame (`Clear`+`Block`, title `" Clean up "`, width `min(68, w)`, height `lines+2` min area) with 6 lines of prose and a hand-typed close hint. This is a dismiss-only **notice dialog** (no framework equivalent exists today).
**Keys.** Non-empty: `MULTI_SELECT_KEYS`. Empty: `MENU_KEYS` (`Esc`/`Enter`/`Close`/`Abort` dismiss; other keys swallowed). `MODAL_CONVENTION` row: Enter `Submits`, Space `Toggles`.
**Tests.** 5 unit tests; layout/keys consistency tests via `builders()`/`tables()` (`consistency_tests/keys.rs:48` consults both tables).

**Verdict: stays in Conspectus as a thin adapter** over `Checklist` plus a framework `NoticeDialog`.

---

### 3.7 Controls overlay - `controls.rs` (1344 + 914 tests)

**Purpose.** One scrolling modal fronting view switching, grouping, filters, sort, preview modes, and content search; changes apply immediately. ADR 0031 (menu-first discovery with accelerators layered on).

**Public surface.**
- `ControlsContext<'a>` (13 fields of live app state: `view`, `grouping`, `filter: &RowFilter`, `sort`, `mux_recency`, `preview_wrap`, `session_preview`, `content`, `search_kinds`, `content_group`, `search_history`) (`:84-112`).
- `ControlsCursor` enum with 24 variants naming each landable row (`:114-163`); `SubEditor {Harness, MaxAge, Content, MuxState, PrState}` (`:165-176`); `ControlsOutcome {Continue, Close, ApplyAndStay(Msg), ApplyAndClose(Msg)}` (`:178-194`).
- `ControlsOverlayState::new(&ctx)` / `new_at_filters` / `cursor()` / `sub_editor()` / `handle_key(&ctx, key) -> ControlsOutcome`.
- `ControlsOverlayWidget::new(&state, ctx, &Theme)`; `impl Overlay { Ctx = &ControlsContext }` maps `ApplyAndStay -> CommitAndStay`, `ApplyAndClose -> Commit`.
- Public helpers: `harness_options()`, `format_duration_for_input`, `parse_max_age`, `view_label`, `centered_modal_rect`, consts `VIEW_OPTIONS`, `SORT_OPTIONS`, `PR_STATE_OPTIONS`, `MUX_STATE_OPTIONS`; `flatten_rows` (`pub(crate)`).

**Anatomy: what is generic and what is Conspectus settings.**

| Lines | Part | LOC | Class |
|---|---|---|---|
| 1-44 | docs, imports | 44 | - |
| 45-82 | option constants (`VIEW_OPTIONS`, `SORT_OPTIONS`, harness/mux/PR option lists) | 38 | (c) |
| 84-112 | `ControlsContext` | 29 | (c); a framework panel would be generic over its own `Ctx` |
| 114-163 | `ControlsCursor` (24 row identities) | 50 | (c); the framework equivalent is a `RowId` generic or index into a row list |
| 165-176 | `SubEditor` enum | 12 | (a) shape (multi-select / text), (c) names |
| 178-194 | `ControlsOutcome` | 17 | (a) with `M` |
| 196-270 | state, `handle_key` skeleton: Ctrl-C always closes; sub-editor owns keys while open; Up/Down move; Space activates; Enter "done" | 75 | (a) |
| 271-307 | `done()`: Enter = press a button / open an editor / apply a choice and close / else just close | 37 | (a) policy, (c) the `matches!` lists of which rows are choices vs editors vs buttons - becomes a per-row `kind` |
| 309-434 | `activate()`: 24 arms building `Msg::SetFilter(RowFilter)`, `SwitchView`, `SetGrouping`, ... | 126 | (b)/(c) |
| 436-531 | `dispatch_sub_editor`: same 8-line pattern x3 (multi-select) + x2 (text); only the `with_*` mapping differs | 96 | (a) skeleton, (c) mapping |
| 534-633 | `build_*_editor`, `indices_to_*`, `with_*` (x3 families) | 100 | (c) |
| 635-680 | `format_duration_for_input`, `parse_max_age` (duration token parser mirroring the CLI) | 46 | (a) as a reusable validator, or (c) |
| 682-692 | `move_cursor` over the flat landable list, wraps | 11 | (a) |
| 694-756 | `flatten_rows(ctx)`: which rows exist per view | 63 | (c) |
| 758-820 | widget render skeleton: body + dim wrapped footer + scroll-to-cursor + sub-editor drawn on top | 63 | (a) |
| 822-852 | `render_sub_editor` mapping `SubEditor` -> widget + labels | 31 | (c) |
| 854-1062 | `body()`: all sections and rows for Conspectus | 209 | (c) |
| 1064-1125 | `Body` (records the cursor line as rows are pushed), `ControlsBody`, `scroll_offset_for_cursor` | 62 | (a) |
| 1127-1166 | row builders `section_header`, `row_line`, `filter_row`, `cycle_row` | 40 | (a) |
| 1168-1259 | value formatters (`pr_state_value`, `harness_value`, `content_value`, ...), `view_label`, `sort_label`, `checkbox_row` | 92 | (c) except `checkbox_row` (a) |
| 1261-1323 | `centered_modal_rect` (+`_for_content`) and `max_controls_content_lines` | 63 | (a) rect, (c) the hand-maintained row-count budget with magic numbers 7 / 8 / 9 / 4 per view, guarded by `modal_height_budget_covers_the_tallest_body` |
| 1325-1340 | `Overlay` impl | 16 | (a) |

Estimate (counting the generic skeleton halves of `done()` and `dispatch_sub_editor`): **about 25-30% (roughly 300-400 LOC) is generic panel machinery; about 70-75% (roughly 950 LOC) is Conspectus settings.** The 914 test LOC is almost entirely domain behavior.

**The generic panel is these ideas:**
1. A flat, ordered list of **landable rows** grouped under **section headers** (headers are not landable; the renderer records the cursor's line index while pushing rows).
2. Five **row kinds** and their visual forms (all exactly the strings below):
   - choice (radio-ish): `> label  active` (cursor `> ` + REVERSED; active = BOLD + trailing `  active` word) (`row_line`, `:1131-1144`)
   - editor row: `> name        value  [ Edit ]` (name padded to 12) (`filter_row`, `:1146-1154`)
   - cycle row: same with `[ Cycle ]` (`cycle_row`, `:1158-1166`)
   - checkbox: `> [x] label` / `[ ] label` (BOLD when checked) (`checkbox_row`, `:1185-1196`; duplicates `form::checkbox`)
   - button: `row_line(label, false, ..)` (e.g. `Clear all filters`)
3. **Space acts on the focused row; Enter applies a choice and closes** (or presses a button / opens an editor); Esc closes keeping everything already applied; sub-editors take over keys until they commit or cancel; `Ctrl-C` closes everything.
4. **Sub-editors** (multi-select, text) rendered as a separate popup on top (`render_sub_editor`), not inside the panel frame.
5. A dim, wrapped key footer plus a `Note("changes apply as you make them")`.
6. Scroll that keeps the cursor line visible.

**Look and feel.** `themed_popup`, title `" Controls "`, width `min(64, w-4).max(40)`, height = content+2 clamped `(8, max(h, 8))` (note: `h`, not `h-4`: it can fill the whole height). Section headers `BOLD`. Cursor `"> "` + `Modifier::REVERSED` (hard-coded, not `theme.selection_active`). The view rows show the global accelerator digit inline (`Sessions [1]`), but the digits are not handled while the overlay is open (the overlay owns all keys). Footer `↑/↓ move · Space select/toggle · Enter apply & close · Esc close · changes apply as you make them`, wrapped, `Modifier::DIM`. **No scrollbar** although a comment at `:788-789` talks about "the scrollbar's" column (stale). **No paging keys** for a list of about 30 rows. Invalid max-age input is **rejected silently** (`:522-526`: the parse error string is dropped and the editor just stays open).

**Keys.** `CONTROLS_KEYS`: `↑/k`, `↓/j` (wrap), `Space` toggle/activate, `Enter` done, `Esc` close (in a sub-editor `Esc` cancels the sub-editor only), `Ctrl-C` abort (closes all, even from a sub-editor). `MODAL_CONVENTION` marks Controls as the one modal with an exception ("changes apply as they are made, so Esc keeps them").

**Tests.** 39 unit tests in `controls_tests.rs`; pantry stories for default, harness sub-editor, max-age sub-editor, mux-state sub-editor (`examples/pantry.rs` requires building a real `ControlsContext` / `Grouping`, which shows the control is not renderable without app fixtures).

**Churn.** 37 commits touch `controls.rs` (`git log --oneline -- ...`), i.e. adding a setting is routine and touches the row enum, `flatten_rows`, `activate`, `body`, the height budget, and tests.

**Verdict: split into generic core + Conspectus adapter.** Framework `SettingsPanel<M>` takes `Vec<Section { header, rows: Vec<Row> }>` where `Row` is one of the five kinds with its current value and an `on_activate -> Outcome<M>`; owns cursor, wrap, scroll, footer, sub-editor routing (`SubEditor` = `Box<dyn Prompt>`), and the height rule (computed from rows, not a magic budget). Conspectus keeps `ControlsContext`, the section/row list builder, every `Msg`, and the filter-to-index mappings.

---

### 3.8 Value modal - `value_modal.rs` (239 + 136 tests)

**Purpose.** Read-only scrollable view of a long value, with optional copy.

**Surface.** `ValueModalState {label, value, scroll: u16, copy: Option<ValueCopy>, highlight_commands: bool}` (all `pub`); `new(label, value)`; `plan(label, text, script, commands)`; `copy_payload() -> (&str, String)`; `handle_key -> ValueModalOutcome {Continue, Close}`; `Overlay` impl; `ValueModalWidget::new(&state, &Theme)`.
`ValueCopy {text, what}` (noun phrase for the toast).

**Variants / callers.** (1) `app.rs:773` explorer `o`: label = field name, value = `long_value`. (2) `runtime/launch.rs:688` rebuild-plan *error* text (plain `new`). (3) `runtime/launch.rs:715` the rebuild plan: `plan(..)` turns on `highlight_commands` (lines starting `$ ` drawn in accent) and sets a copy payload (the commands as a script, `"3 commands"`). (4) layout test builder. `plan()` and `highlight_commands` are rebuild-plan specific **(b)**; they would become a `line_style: Option<fn(&str) -> Style>` and a generic `copy: Option<CopyPayload>`.

**Look and feel.** `themed_popup` using **Help's** `centered_modal_rect` (`help.rs:882`: width `min(78, w-4).max(40)`, height `28.clamp(10, max(h-2, 10))`). Title: `" " + " label " + " "` with the label span `BOLD`+accent (the only titles besides Messages drawn in accent; double padding). Body: `Layout::vertical([Min(1), Length(1)])` text + 1-row footer; text is `Paragraph` + `Wrap{trim:false}` + `scroll((scroll,0))`; footer `VALUE_MODAL_KEYS` colored `secondary_text` (a fg color, not a modifier), **not wrapped** (truncates in narrow modals). No scrollbar (Help has one).

**Keys** (`VALUE_MODAL_KEYS`): `j/↓`, `k/↑` 1 row; `PgDn`/`Space`/`PgUp` jump a *fixed 8 rows* (comment says "a viewport"); `g/Home` top, `G/End` bottom; `y` copy (the runtime does the clipboard write, `overlay_keys.rs:305-338`); `Esc`/`q`/`o` close; `Ctrl-C` abort. **`Bottom` sets `scroll = u16::MAX` and nothing clamps it**; ratatui 0.30's wrapped `Paragraph` returns early with nothing drawn once the scroll offset passes the content (`ratatui-widgets-0.3.0/src/paragraph.rs:432-436`), so `G` (and any `PgDn` past the end) most likely blanks the body **(unverified at runtime)**. The pantry story "Scrolled (bottom)" assumes `u16::MAX` "clamps to the last line", which that code path contradicts. Help clamps correctly (`help_max_scroll`).

**Coupling.** `crate::tui::Overlay` (a), `help::centered_modal_rect` (a, but cross-widget import), `Theme` (`panel_focus_accent`, `secondary_text`) (a), `$ ` command convention (b), `Msg` none. Cleanest read-only control.

**Tests.** 10 unit tests; 1 render assertion for command accent and footer. Pantry: 4 stories.

**Verdict: extract after refactor** as `Pager` / `TextViewer` shared with Help and the viewer help page (other inventories): one scroll model with clamping and optional scrollbar, one footer rule.

---

### 3.9 Messages overlay - `messages.rs` (251 + 90 tests) and model `tui/messages.rs` (281 + tests)

**Purpose.** Newest-first list of operation outcomes with the selected entry's full record below. ADR 0105.

**Surface.** `MessagesOverlayState {selected, detail_scroll}` (`pub` fields) with `selected_entry(&MessageLog)`; `Overlay` impl with `Ctx = &MessageLog`; `MessagesWidget::new(&state, &log, &Theme, now)`; `level_style(level, theme)`.
Model: `LogLevel {Info, Warning, Error}` (`glyph()` returns `√` `▲` `×` as literals - equal to `icons::marks::OK/WARNING/FAIL` but not shared), `LogTarget {Pin(String), Mux(MuxSessionId)}`, `CommandRecord {argv, exit_code, stdout, stderr}`, `LogEntry {at_epoch, level, summary, target, command, detail, repeats}` with `full_text()`, `output_tail()`, `MessageLog` (capacity 200, consecutive-duplicate folding to `(×N)`, `unseen` count of warnings and errors).

**Coupling.** Overlay widget itself is nearly generic: it needs a list of `{level, age, summary}` rows and a detail text. Conspectus pieces: `LogTarget`, `CommandRecord` (argv/exit/stdout/stderr), `MuxSessionId` import, `crate::discovery::current_epoch()` inside `LogEntry::new`, `crate::tui::rows::format_recency` for the age (`"{age:>4} ago  "`), `unseen` chip in the status bar (other slice), preview banner (`latest_failure_for`).

**Look and feel.** `themed_popup`; title `" " + " Messages · N " + " "` (accent+BOLD span); width `(w-4).clamp(40,110)`, height `(h-2).max(10)`. Body: list region (up to 40% of height, min 3 rows), a `─` rule in `divider` modifier, detail region (wrapped `Paragraph`, `detail_scroll`), 1-row footer in `placeholder` modifier (not wrapped). List row: `"{glyph} "` colored by level (`success`/`warning`/`error`) + `"{age:>4} ago  "` in `secondary_text` + summary; the **selected row has no glyph marker**; every span gets `theme.selection_active` (REVERSED|BOLD) - the one overlay that uses that theme token. Stateless bottom-anchored scroll (`first = selected - (list_height - 1)`). Empty state: 2 prose lines + footer. `detail_scroll` is unclamped (same blank-after-end risk as the value modal) and has no indicator.

**Keys** (`MESSAGES_KEYS`): `j/↓` next, `k/↑` prev (clamp, no wrap); `g/Home` first (newest), `G/End` last (oldest); `J`/`K`/`PgDn`/`PgUp`/`Space` scroll the detail (8 rows); `y` copy (`AnyModExceptCtrl('y')`, runtime-handled); `Esc`/`q`/`!` close; `Ctrl-C` abort. `j/k` here move the *selection* while in the value modal they scroll text: same keys, different object.

**Tests.** 3 unit tests (selection resets detail scroll, close keys, render of list+detail).

**Verdict: split into generic core + Conspectus adapter.** Generic `MasterDetail` / `LogList` (rows with level glyph + age + summary, detail pane, scroll, copy outcome, level styling from theme). The `MessageLog` model could ship in the framework as an app-agnostic `NoticeLog` (level, time, summary, detail text) with Conspectus adding `target` and `command` through an extension. Decision for the control-set checkpoint.

---

### 3.10 Toast - `toast.rs` (102 prod + 72 inline tests)

**Purpose.** Short-lived feedback blip. Shim over `ratatui-comfy-toaster::ToastEngine<()>`.

**Surface.** `TOAST_DURATION = 1500 ms`; `engine() -> ToastEngine<()>`; `builder_for(label) -> ToastBuilder` (`ToastType::Success`, `toast_bg(Reset)`, `ToastBorderMode::Full`, `ToastPosition::Center` + `offset(0, i16::MAX)` to pin to the bottom edge, fixed duration); `engine_dismiss_all(engine)`; `ToastEngineHolder(pub ToastEngine<()>)` (Deref/DerefMut newtype so `App` keeps `#[derive(Debug)]`).
**Wiring.** `App.toast` (`app.rs:284`), `post_toast(label)` drains the queue then shows one toast (replacement semantics) (`app/overlays.rs:922-927`); `prepare_toast_for_render(area)` calls `set_area` + `tick` before every draw (`:932-936`; snapshot mode does the same, `snapshot.rs:159`); `ui.rs:179-188` `draw_toast` renders `&ToastEngine` last (above all modals and the status bar).

**Variants / callers.** There is exactly one toast style. 40 `Effect::Toast(String)` constructions and 12 direct `post_toast` calls. No severity, no title, no duration override, no click/dismiss key.
Observed content drift (representative): `"copied: {label}"` (`runtime.rs:2280`), `"Copied {what}"` (`overlay_keys.rs:317`), `"copied message"` (`:360`), `"copy failed: {err}"` (x2, `:318,:361`), `"No searches yet"` (`content.rs:1101`), `"nothing to undo"` (`app.rs:1890`), `"pin: nothing entered"` (`app.rs:2180`), `"that isn't available in this mode"` (`executor.rs:16`). Capitalization and `topic:` prefixes are inconsistent; failures and successes look identical.

**Coupling.** None to Conspectus types (it takes a `String`). The `Effect::Toast` variant and `lifecycle` entry (`effect.rs:27,298`) are layer 3. Theme: **border color is not themeable** (upstream maps `Success` -> green via a hardcoded `From<ToastType> for Color`), so `theme.success/warning/error` cannot reach it; a failure toast draws with a green border (**D15**).

**Look and feel.** Full border, transparent background, white text, bottom-centered, auto width wrapped by `textwrap`, auto-dismiss at 1.5 s regardless of message length.
**Keys.** None (not focusable, not dismissible by key).
**Tests.** 5 inline tests (empty, expiry via `sleep`, visible, dismiss-all, paints border+label); a reducer test pins "newer replaces older".

**Verdict: extract after refactor** as `Toasts` with `Level {Info, Success, Warning, Error}` driven by theme tokens, a duration policy by level or text length, and the replacement semantic. Open: keep `ratatui-comfy-toaster` (needs an upstream/fork change for border color) or own the ~100 lines. Also decide whether toast, status-bar message and `MessageLog` share a `Notice` type (the app has three transient-feedback channels with no shared type).

---

### 3.11 Text-prompt dialog - `input.rs` (204) (+ `modal.rs` `RenameOverlayState`)

Covered in depth by the text-input inventory; the dialog-relevant facts are:

- `TextInputState::new(title, initial)`, `handle_key -> InputOutcome {Continue, Confirm(String), Cancel}` (`Confirm` carries the **raw** value; the doc comment says "trimmed"; every caller trims), `TextInputWidget::new(&state).theme(&theme)`; fixed 3-row frame `min(60, w-4).max(20)` (`input.rs:165`).
- Used as a *dialog* in four places: rename session/mux (`overlay_keys.rs:33,43` titles `" rename session "`, `" rename mux "`), pin rename (`overlay_keys` via `app/overlays.rs:49`, title `"Rename pin"`), worktree branch (`"New worktree — branch"`), Controls max age and contains (`" max age "`, `" contains "`). **Title style differs**: padded + lowercase vs unpadded + capitalized.
- No footer in the dialog; no validation message (invalid max-age and empty branch stay open silently); keys `TEXT_INPUT_KEYS` (`Enter` confirm, `Esc`/`Ctrl-C` cancel, `Tab` swallowed).
- `RenameOverlayState` (`modal.rs:166-202`) is a wrapper whose only job is mapping `Confirm(String)` -> `Msg::CommitRename`; a generic `TextPrompt<M>` with an `on_submit: Fn(String) -> M` removes the need for per-use wrappers.

---

### 3.12 Confirm patterns (two-step gates)

| Pattern | Where | Mechanism | Keys | Gated action |
|---|---|---|---|---|
| Inline arm | `row_menu.rs:106-111` | `armed: Option<usize>`; first Enter arms, second commits; moving disarms; row text turns `error`+BOLD with "— press Enter again to confirm" | Enter only | `Forget` (the only `confirming()` entry; `actions.rs:762`) |
| Sub-screen y/n | `worktree_menu.rs` `Confirm{Remove,Merge,Prune}` | mode switch, body prose + guard list + table footer | Enter/`y` yes, Esc/`n` no | remove, merge, prune worktree |
| Sub-screen choice | `worktree_menu.rs` `CloseDownChoice` | mode switch | `m` / `d` / Esc / `q` | close-down (merge or discard) |
| Review-then-commit | `clean_up.rs` | the checklist is the review; one Enter commits | Space toggles, Enter | reap |
| Press-again navigation gate | `explorer_nav.rs:324-350` | `explorer_back_armed: bool`, set on first Backspace with an empty breadcrumb in the right pane, cleared by any other Msg except `ExplorerBack` and viewport-change messages (`app.rs:1405-1416`) | Backspace x2 | leave the right pane |
| No confirm | `Unpin`, `Rebuild`, `Refresh snapshot` | straight commit | | state-changing but undoable or non-destructive |

---

## 4. Cross-control matrices

### 4.1 List model

| Surface | Cursor step | End behavior | Scroll to keep cursor visible | Disabled entries | Shortcut shown in row | Sections | Home/End/PgUp/PgDn |
|---|---|---|---|---|---|---|---|
| RowMenu | hand `saturating_sub` / `min(last)` | **clamp** | **none** (content clips; cursor can reach clipped rows) | none (verbs not applicable are omitted when building) | none (hints live in subtitle text, e.g. `pin this (p)`) | none | none |
| MuxMenu | same | clamp | none | none | none | none | none |
| WorktreeMenu list | same | clamp | none | none (omitted) | none | none | none |
| Picker | `cursor::wrap_step` | **wrap** | stateless, bottom-anchored (`picker.rs:241`) | none | none | none | none |
| MultiSelect / CleanUp | upstream `next/prev` | wrap | stateless, bottom-anchored (`compute_scroll`) | upstream supports; shim does not expose | none | none | none |
| Controls | `wrap_step` over landable rows | wrap | stateless, bottom-anchored by line (`scroll_offset_for_cursor`) | none (rows omitted per view) | `[n]` digit for views (display only; inert in overlay) | **yes** (BOLD headers, 7-9 per view, blank-line separated) | none |
| Messages | hand `saturating_sub` / `min(len-1)` | clamp | stateless (`first = selected - (h-1)`) | none | none | none | `g/G`, `Home/End` for first/last; detail pages 8 |
| ValueModal | scroll offset `u16` | `0` floor; **no ceiling** | n/a | n/a | n/a | n/a | PgUp/PgDn/Space = 8, `g/G`, `Home/End` |

"Stateless bottom-anchored" means the first visible row is recomputed from the cursor only, so
moving *up* from the end drags the window with the cursor pinned to the bottom row instead of a
sticky offset (what ratatui's `ListState` provides and nothing here uses).

### 4.2 Keys (from the `KeyRow` tables)

| Key | Menus (`MENU_KEYS`) | Controls | Multi-select | Picker | Value modal | Messages | Confirm / close-down | Text input |
|---|---|---|---|---|---|---|---|---|
| `↑` `↓` | move (shown) | move (shown) | move (alias) | move (alias) | scroll (alias) | select (alias) | - | - |
| `j` `k` | move (alias) | move (alias) | move (alias) | **types into query** | scroll (shown) | select (shown) | - | types |
| `Ctrl-N` `Ctrl-P` | - | - | - | move (alias) | - | - | - | - |
| `g` `G` / `Home` `End` | - | - | - | - | top/bottom (alias) | first/last (alias) | - | - |
| `PgUp` `PgDn` | - | - | - | - | 8 rows (alias) | detail 8 rows (alias) | - | - |
| `Space` | - | activate/toggle (shown) | toggle (shown) | types | page down (alias) | detail page down (alias) | - | types |
| `Enter` | pick (shown) | apply choice & close (shown) | confirm (shown) | pick (alias) | - | - | yes (shown) | confirm (shown) |
| `Esc` | close (shown) | close / cancel sub-editor | cancel (shown) | cancel (alias) | close (shown) | close (shown) | no / back | cancel (shown) |
| `q` | **-** | **-** | **-** | types | close | close | back (close-down only) | types |
| `Ctrl-C` | abort = close | abort = close **all** | abort = cancel | abort | abort = close | abort | abort = back one level | abort = cancel |
| other | | digits `1`-`6` shown in row labels, not bound | | | `y` copy, `o` close | `y` copy, `!` close, `J` `K` detail | `y`/`n`, `m`/`d` | Tab swallowed |

Convention check: `MODAL_CONVENTION` + `consistency_tests/keys.rs` assert only Esc leaves, Enter
applies/activates/does-nothing, and Space toggles/pages/types per modal. They do **not**
assert j/k, arrows, wrap vs clamp, `q`, `g/G`, or paging parity, which is where the divergences above sit.

### 4.3 Frame, size, title

| Surface | Framing | Width | Height | Title text and style |
|---|---|---|---|---|
| TextInput | `themed_popup` | `min(60, w-4).max(20)` | 3 | caller string, unstyled, padding chosen by caller |
| MultiSelect | `themed_popup` | `min(60, w-4).max(20)` | `items+2`, clamp `(3, max(h-4, 3))` | caller string, unstyled |
| Picker | `themed_popup` | `min(80, w-4).max(30)` | `h-4` (min 8), cap 20 dead | `" {title} ({v}/{t}) "` |
| RowMenu | **hand-rolled** | `min(72, w)` | `lines+2`, clamp `(8, h)` | `" {title} "` |
| MuxMenu | **hand-rolled** | `min(62, w)` | `lines+2`, clamp `(10, h)` | `" Mux "` + in-body heading `Mux actions` |
| WorktreeMenu | **hand-rolled** (BranchInput via TextInput frame) | `min(56, w)` | `lines+2`, clamp `(5, h)` | `" Worktree "` |
| CleanUp (empty) | **hand-rolled** | `min(68, w)` | `lines+2`, min `h` | `" Clean up "` |
| ValueModal | `themed_popup` + Help's rect | `min(78, w-4).max(40)` | `28.clamp(10, max(h-2, 10))` | `"  label  "`, accent+BOLD |
| Messages | `themed_popup` | `(w-4).clamp(40, 110)` | `(h-2).max(10)` | `"  Messages · N  "`, accent+BOLD |
| Controls | `themed_popup` | `min(64, w-4).max(40)` | content+2, clamp `(8, max(h, 8))` | `" Controls "`, unstyled |

Border always `fg(panel_focus_accent)` (cyan default). Hand-rolled menus use `min(W, area.width)`
(touches the screen edge on narrow terminals) where the `themed_popup` ones keep a 2-cell margin.

### 4.4 Selection highlight and state marks

| Surface | Cursor mark | Row emphasis | Active / checked mark | Theme token used |
|---|---|---|---|---|
| RowMenu / MuxMenu / Worktree list | literal `►` (menus; `icons::marks::POINTER` exists but is not used) | `BOLD` label only; armed = `error` + BOLD | - | `error` (armed) |
| Picker | `> ` | whole row `REVERSED` (hard-coded) | - | `panel_focus_accent` (query slash), `placeholder` (detail) |
| MultiSelect / CleanUp | cheese `>` in accent | none (checked row = accent+BOLD) | `√` checked, `•` unchecked (`marks::OK/BULLET`) | `panel_focus_accent`, `error`/`success` (unused validation slots), `placeholder` |
| Controls | `> ` | `REVERSED` (hard-coded); active = BOLD + `  active` word | `[x]`/`[ ]` for checkboxes | none for rows (footer uses raw `Modifier::DIM`) |
| Messages | none | `theme.selection_active` (REVERSED+BOLD) on all spans | level glyph `√ ▲ ×` colored `success/warning/error` | `selection_active`, `secondary_text`, `divider`, `placeholder` |

So: **three cursor glyph idioms** (`►`, `>`, none) and **four highlight idioms**. Only Messages and the
left panel read `theme.selection_active`.

### 4.5 Footer / hint rendering

| Surface | Footer rendered? | Source | Style | Wrapped? |
|---|---|---|---|---|
| RowMenu | yes | `MENU_KEYS` | plain | yes (`wrapped_lines`) |
| MuxMenu | yes | `MENU_KEYS` | plain | no |
| Worktree: List / BranchInput | **no** | - | - | - |
| Worktree: confirms, close-down | yes | `CONFIRM_KEYS`, `CLOSE_DOWN_KEYS` | plain | via `Paragraph` wrap |
| Picker | **no** (all aliases) | - | - | - |
| MultiSelect / CleanUp | **no** (hints exist in table, never drawn) | - | - | - |
| CleanUp (empty) | hand-typed `"Esc/Enter close"` | literal (`clean_up.rs:124`) | plain | no |
| Controls | yes | `CONTROLS_KEYS` | `Modifier::DIM` | yes |
| ValueModal | yes | `VALUE_MODAL_KEYS` | `fg(secondary_text)` | no |
| Messages | yes | `MESSAGES_KEYS` | `theme.placeholder` modifier | no |
| TextInput | **no** | - | - | - |

Footer verb grammar: `HELP_KEYS`, `VALUE_MODAL_KEYS`, `CONFIRM_KEYS` use "`Esc to close`" /
"`Enter to confirm`"; every other table uses bare verbs ("`Esc close`", "`Enter confirm`");
`CLOSE_DOWN_KEYS` uses "`m = merge & close`".

Hand-typed hints outside the generated tables (ADR 0123 intent violated, but outside
`tests/overlay_keymap_hygiene.rs`'s file set or heuristic): `runtime.rs:1514` ("worktree: ↑/↓ move
· Enter pick · Esc close"), `:1548` ("close down: m merge · d discard · Esc cancel", which already differs
from the table's "m = merge & close ..."), `:1587`, `:1789`; `clean_up.rs:124`; `row_menu.rs:141`.
The hygiene test's `looks_like_key_hint` requires a ` · ` separator and scans only
`src/tui/widgets/**`, `runtime/overlay_keys.rs`, `modal.rs`.

### 4.6 Outcome types (one concept, five enums)

| Type | Variants | Used by |
|---|---|---|
| `OverlayOutcome` | `Consumed`, `Commit(Box<Msg>)`, `CommitAndStay(Box<Msg>)`, `Close` | all `Overlay` impls |
| `MultiSelectOutcome` | `Continue`, `Confirm(Vec<usize>)`, `Cancel` | multi-select |
| `InputOutcome` | `Continue`, `Confirm(String)`, `Cancel` | text input |
| `ValueModalOutcome` | `Continue`, `Close` | value modal (duplicate of `OverlayOutcome` minus commits; also keeps an inherent `handle_key` next to `Overlay::handle`) |
| `ControlsOutcome` | `Continue`, `Close`, `ApplyAndStay(Msg)`, `ApplyAndClose(Msg)` | controls (maps 1:1 onto `OverlayOutcome`) |

A single `Outcome<T> { Pending, Submit(T), Close }` plus a "stay open" flag covers all five.

### 4.7 Theme token usage in this slice

| Token | Used for |
|---|---|
| `panel_focus_accent` | every border; titles of ValueModal and Messages; cursor/`checked` in MultiSelect; picker `/`; value-modal command lines |
| `error` | armed row (RowMenu); Messages Error glyph; unused cheese validation slot |
| `success` / `warning` | Messages levels |
| `secondary_text` | value-modal footer, Messages age |
| `placeholder` (modifier DIM) | picker detail and empty states, Messages footer, cheese `disabled` slot |
| `divider` | Messages rule |
| `selection_active` | Messages selected row only |
| (none; literals) | Controls `REVERSED`/`DIM`, Picker `REVERSED`, Worktree `BOLD`, toast colors (upstream) |

---

## 5. Evidence for the layer-2 harness (what the framework must carry)

- `consistency_tests/layout.rs` (535 LOC): `name_of` (exhaustive match so a new `Modal` variant stops compiling until named), `builders()` (one constructor per modal), `SKIPPED` with reasons, and three generic geometric checks: *nothing a modal draws escapes its frame* (5 widths x 2 heights; frame found by corner glyphs among cells that differ from the same app without the modal), *moving the selection moves no text* (frames after `Down` x4 + `Up`, at 80 and 120 columns x 40 rows), *no word is cut at the edge* (5 x 2, compared against a 240-col render); plus two coverage checks (every `Modal` variant is built or skipped with a reason; each builder opens the modal it names) and Search-specific sizing tests. This is domain-free apart from `App`/`Modal` and the fixture; it is the best candidate for a framework "overlay conformance" harness.
- `consistency_tests/keys.rs`: `tables(modal)` maps modal -> key tables; checks against `MODAL_CONVENTION`.
- `consistency_tests/menus.rs`: *entity-not-view* menu equivalence; domain (stays).
- `consistency_tests/text.rs` renders every overlay and checks for developer-artifact strings (CLAUDE.md rule); the string rule is repo policy, not framework.
- Snapshot coverage in this slice: `insta` snapshots exist only for the row menu (2 frames in `ui_tests`); Controls, Picker, MultiSelect, ValueModal, Messages, Toast assert on text/cells in unit tests with no snapshot. Pantry (`examples/pantry.rs`) has stories for MultiSelect (3), Controls (4), ValueModal (4), TextInput (3), none for RowMenu, MuxMenu, Worktree, Clean up, Picker, Messages, Toast (4 of 12 surfaces).
- The `tui --snapshot` driver (ADR 0067) is app-level and out of this slice.

---

## 6. Candidate base-control set from this slice (inputs to the dedupe phase)

| Base control | Absorbs | Generic payload | Stays in Conspectus |
|---|---|---|---|
| `ActionMenu<M>` | RowMenu, MuxMenu, Worktree list, Pin-unpin menu | entries `{label, detail?, shortcut?, enabled, confirm: None/Inline, outcome: M}`, title | `row_menu_for`, `menu_title`, `pin_management_entries`, mux entries, `WorktreeAction` policy |
| `ConfirmDialog<M>` / `ChoiceDialog<M>` / `NoticeDialog` | worktree Confirm x3, CloseDown choice, armed Forget (if inline is dropped), CleanUp empty notice | title, body lines, optional warning list, verbs, outcome | all wording and `Msg::Commit*` |
| `TextPrompt<M>` | rename wrappers, branch prompt, Controls max-age/contains | title, initial, validator -> error text, `on_submit` | parsers (`parse_max_age`) |
| `FilterPicker<T>` | Picker (+ candidate for Search quick-select) | items `{label, detail, T}`, `on_pick` | `retarget_candidates`, history items |
| `Checklist` | MultiSelect, CleanUp | labels, preselected, disabled, select-all, `on_confirm(indices)` | candidate labels, `ReapRecords` mapping |
| `SettingsPanel<M>` | Controls | sections of rows (choice / checkbox / editor / cycle / button) + sub-editor prompts | all 24 rows, `ControlsContext`, filters |
| `Pager` | ValueModal (+ Help, viewer help; other inventories) | text, optional line style, optional copy payload | plan/command convention |
| `LogList` / `MasterDetail` | Messages | rows `{level, time, summary}` + detail text | `LogTarget`, `CommandRecord`, preview banner |
| `Toasts` | toast.rs | level, text | all call sites |
| Layer-2 contract | `modal.rs`, `keybindings/overlay.rs` machinery, `popup_frame.rs`, `cursor.rs`, layout/keys conformance harness | `Overlay<M>`, `Outcome<T>`, `KeyRow`, footer generation | `Modal` enum variants, key tables' action enums, runtime handlers |

Visible deltas the extraction stories would need to catalog (consistency-first rule): single
cursor glyph and highlight; wrap-vs-clamp at list ends for menus; `q` on menus; footer grammar and
style; picker height; MuxMenu heading and width; Controls/Picker `REVERSED` -> theme token;
ellipsis instead of silent cut in the checklist; scrolling menus; clamped `G`.

---

## 7. Duplications and inconsistencies

Counts and locations; "n/m" means n of m surfaces in this slice.

**D1. Two framing paths for the same frame.** 6 surfaces use `themed_popup` (TextInput, MultiSelect, Picker, ValueModal, Messages, Controls); 4 hand-roll `Clear`+`Block`+`Paragraph` (`row_menu.rs:174-182`, `mux_menu.rs:145-153`, `worktree_menu.rs:661-671`, `clean_up.rs:126-139`). Visually identical. 6 copy-pasted `KnownSize` body wrappers; `LinesBody` (`popup_frame.rs:75`) has no production caller in this slice.

**D2. Five `centered_modal_rect` functions** (`input.rs:165`, `multi_select.rs:310`, `controls.rs:1264` + `_for_content` `:1268`, `help.rs:882`, `search.rs:1413`) plus `popup_frame::centered_rect`, and ten distinct width rules (caps of 56, 60, 62, 64, 68, 72, 78, 80, 110, plus the percent-based Search rule); the audit item A6 ("centered_modal_rect x6") was only partly closed by CSP-464, which unified the arithmetic but not the policy.

**D3. Six implementations of "scroll just enough to show the cursor".** `compute_scroll` byte-identical in `multi_select.rs:295` and `search.rs:1423`; `scroll_offset_for_cursor` in `controls.rs:1109` and `form.rs:724` (same logic, `Option` vs not); inline in `picker.rs:241` and `messages.rs:199`. All are stateless and bottom-anchored. ratatui `List`/`ListState` is unused (0 hits).

**D4. Wrap vs clamp at the list ends.** Wrap: Controls, Picker, MultiSelect/CleanUp (3). Clamp: RowMenu, MuxMenu, Worktree list, Messages (4). `cursor::clamp_step` exists (`cursor.rs:20`) and none of the four clamping widgets call it.

**D5. Row cursor.** Glyph: `►` (RowMenu, MuxMenu, Worktree list; `icons::marks::POINTER` is documented as "the row cursor in menus" and unused by them) vs `>` (Controls, Picker, MultiSelect via cheese) vs none (Messages). Highlight: BOLD only vs `REVERSED` vs accent glyph only vs `theme.selection_active`. Only 1 of 6 list surfaces reads `selection_active`.

**D6. Footers.** Fully rendered by 5 of 9 control families (RowMenu, MuxMenu, Controls, ValueModal, Messages) and by 2 of the Worktree menu's 6 modes (confirms and close-down); missing in MultiSelect/CleanUp, Picker, TextInput, and the Worktree list and branch prompt. 4 footer styles (plain, `DIM`, `secondary_text`, `placeholder`), 2 wrapping behaviours, 3 wrap implementations (`popup_frame::wrapped_lines`, `form::wrap_footer`, Help's `footer_rows`). 6 hand-typed hint strings (D-list in 4.5).

**D7. Five outcome enums for one concept** (4.6); `ValueModalState` keeps both `handle_key` and `Overlay::handle`, `MultiSelectState`/`TextInputState` have only an inherent `handle_key`.

**D8. Cancel feedback.** 6 handlers post `"<x>: cancelled"` on Close; 8 do not (2.2).

**D9. `Ctrl-C` semantics.** Controls: closes everything even from a sub-editor. Worktree menu: backs out one level in every sub-mode. Others: close. ADR 0123 states the Controls behavior as a general rule ("in Controls and the pins menu"); the pins menu no longer exists.

**D10. Confirmation idioms.** Inline arm (1 entry, Enter only), y/n sub-screen (3 modes, Enter/`y`/Esc/`n`), choice sub-screen (1), none for other state-changing entries (`Unpin`, `Rebuild`, `Refresh`), plus an unrelated press-twice navigation gate. Arming state and "press Enter again" text are bespoke to RowMenu.

**D11. Copy-paste inside controls.** `worktree_menu.rs`: 3 confirm handlers (`:419-480`) and 3 guard-session render blocks (`:567-634`). `controls.rs`: 3 multi-select dispatch arms (`:445-483`), 3 `build_*_editor`, 3 `indices_to_*`, 3 `with_*`, 3 value formatters, `filter_row` vs `cycle_row` (differ only in `[ Edit ]` / `[ Cycle ]`), `checkbox_row` vs `form::checkbox`. `sub-editor` for text x2 differ only in validation.

**D12. MuxMenu is RowMenu minus features.** Differences: width 62/72, height floor 10/8, heading line, no ellipsis/wrap/confirm; three tests duplicate `row_menu_tests`.

**D13. Domain strings inside "generic" widgets.** Picker empty note (`picker.rs:232`), `PickerPurpose` variants; CleanUp empty prose with a config key (`clean_up.rs:120-122`); Messages empty prose naming launches/attaches/renames (`messages.rs:167-168`); `ValueModalState::plan` and `$ ` command styling; `WIRED`/`WorktreeAction` in the worktree menu.

**D14. Theme/glyph token bypass.** Hard-coded `REVERSED` (Picker `:254`, Controls `:1137,1149,1161,1189`), `Modifier::DIM` (Controls footer `:796`), `"►"` literals (`row_menu.rs:138`, `mux_menu.rs:123`, `worktree_menu.rs:552`) despite `icons::marks::POINTER`, `LogLevel::glyph` literals despite `marks::OK/WARNING/FAIL`, `"> "` row-cursor literals in 5 files (`picker.rs`, `controls.rs` x4, `form.rs`, `mux_launch.rs`, `search.rs` x4) plus cheese's default `>`. `ui.rs:333` is a fourth `"► "` literal for pane focus.

**D15. Toast has no severity and is not themeable.** 52 call sites, 1 style: failures (`copy failed`, `nothing to undo`) and successes both get the green Success border; fixed 1.5 s regardless of text length (some toasts are full sentences, e.g. `app.rs:1954`). Copy tone varies (`Copied`, `copied:`, `copied message`). Three feedback channels (status message, toast, `MessageLog`) with no shared type.

**D16. Scroll hygiene.** `ValueModalState::scroll` and `MessagesOverlayState::detail_scroll` have a floor but no ceiling; `G` sets `u16::MAX`; with a wrapped `Paragraph` ratatui draws nothing past the end **(unverified at runtime)**. Help clamps (`help_max_scroll`) and is the only modal with a scrollbar; none of the 5 scrolling surfaces in this slice (Controls, Picker, MultiSelect/CleanUp, ValueModal, Messages) shows a scroll position (Controls even has a stale comment about a scrollbar column, `controls.rs:788`). Page keys move a fixed 8 rows (value modal, messages) although docs say "a viewport".

**D17. Latent panic in hand-rolled heights (unverified at runtime).** `u16::clamp(min, max)` panics when `min > max`: `row_menu.rs:172` `clamp(8, area.height)`, `mux_menu.rs:143` `clamp(10, area.height)`, `worktree_menu.rs:660` `clamp(5, area.height)` panic on terminals shorter than 8 / 10 / 5 rows. The `themed_popup` modals use `max(h)` guards. Layout tests start at height 20.

**D18. RowMenu cannot scroll.** `history_row_menu` baseline (24 rows, 10 entries x 2 lines) already loses the footer; the cursor can move onto entries that are clipped. The layout test checks "nothing escapes the frame", not "cursor row is visible".

**D19. Truncation policies.** `ellipsize` (RowMenu labels only), silent cut (cheese labels `truncate_to_width`, Picker/Controls paragraph clip), word wrap (subtitles, Worktree). In CleanUp the *age* (`last seen ...`) is the first thing silently dropped.

**D20. Dead code.** Picker height `max_height.clamp(8, 20.max(max_height))` always equals `max_height` (the 20 cap never applies); `Option<&Theme>` fallback branches in `MultiSelectWidget` (`multi_select.rs:221-227`) and `TextInputWidget` (`input.rs:120-126`) are unreachable in production (all callers pass a theme); `PICKER_KEYS` carries no shown hints; `MULTI_SELECT_KEYS`/`TEXT_INPUT_KEYS` shown hints are never rendered; `LinesBody` unused here; cheese `validation_*` styles mapped but validators unused.

**D21. Title style.** Accent+BOLD with double padding (ValueModal, Messages) vs plain `" Title "` (Controls, RowMenu, MuxMenu, Worktree, CleanUp-empty) vs caller-supplied with/without padding and case (` rename mux `, `Rename pin`, ` harness `, `Clean up: forget these records`, `New worktree — branch`).

**D22. Invisible feedback on rejected input.** Controls max-age parse errors (`controls.rs:522-526`) and empty worktree branch (`worktree_menu.rs:407`) keep the editor open with no message; Picker query has no caret.

**D23. Ellipsis convention.** `…` marks "opens a follow-up" for `Edit…`, `Rename…`, `Retarget…`, `Clean up…`, `New worktree…`, `Close down stream…`, `Prune merged worktrees…`, `Launch harness in new mux…`, but not for `New tmux session (bare shell)`, `Remove worktree`, `Merge back & close`, `Show rebuild plan` (opens a modal).

**D24. Per-overlay registration cost.** 6-8 production files plus 2-3 consistency-test files per overlay beyond its widget; 14 handler fns of the same shape; a 16-branch dispatch chain (2.2).

**D25. `q`.** Closes Help, ValueModal, Messages, close-down, viewers; does not close any list menu, Controls, Picker (types) or checklist.

**D26. Key-parity not tested.** `MODAL_CONVENTION` checks Esc/Enter/Space only; arrows/`j k`/wrap/`q`/paging parity are untested, which is where D4, D9, D25 live.

**D27. Mouse.** ADR 0031 promises click support; there is none.

---

## 8. Open questions for the operator

1. **Cursor and highlight style.** Which is the canonical list-row look: `►` + BOLD (menus, and `docs/tui-columns.md` calls `►` "a menu cursor"), `> ` + `REVERSED` (Controls, Picker, Search, forms), or the themeable `selection_active` modifier (Messages, left panel)? Recommendation to test against: one glyph, one `theme.selection_active` highlight.
2. **Wrap or clamp at list ends** for short action menus (today clamp) vs long lists (today mostly wrap)?
3. **Confirm idiom.** Standardize on in-place arming, or a y/n dialog, or both with a rule (arm for single-entry verbs, dialog when there is something to show such as the live-session guard)? Do `Unpin`, `Rebuild`, `Refresh snapshot`, and the Clean up commit need one?
4. **`q` and paging keys.** Should `q` close menus/pickers/checklists, and should long lists (Controls about 30 rows, Picker, Checklist, Messages) gain `PgUp/PgDn/Home/End`?
5. **Toast severity.** Should failures move to warning/error styling (needs theme-able border, which means an upstream/fork change or owning the widget), a longer duration, or only the Messages log? Is one `Notice` type for status bar, toast, and log in scope for the framework or for the app?
6. **Dependency policy (CLAUDE.md requires an ADR).** Moving `ratatui-cheese`, `ratatui-comfy-toaster`, `tui-popup`, `tui-input` into the framework crate changes where they are owned. Keep them behind the framework API, or replace the two shims (checklist mirror hack, toast color) with in-tree code? This feeds the control-set ADR.
7. **Scope of new base features.** Add now, or defer to the gap analysis: select all/none and selected count in the checklist, disabled rows with reasons, in-row accelerator column, scroll indicators, validation message line in `TextPrompt`, mouse support?
8. **Modal stacking.** Does the framework contract need real stacking (a dialog drawn over a dimmed menu) or is "pop, then open the next" (what the app does today) the contract?
9. **Controls semantics.** Keep "changes apply as you make them" (Esc keeps them) as a first-class `SettingsPanel` behavior, or provide a staged-apply variant too?
10. **MuxMenu.** Remove the dedicated widget and fold its two entries into `ActionMenu` (changes: title heading, width 62 -> shared)? Same for any future one-off menu.
11. **Defect fixes as part of extraction.** Treat unclamped value-modal/messages scroll (D16), the short-terminal clamp panics (D17), the dead picker height cap (D20), and silent input rejection (D22) as "consistency wins" inside the extraction stories, or file them separately first?
12. **Wording convention.** Footer grammar (`Esc close` vs `Esc to close`), title case and padding, ellipsis rule (D23), and toast tone: pick one set for the framework's default strings.
