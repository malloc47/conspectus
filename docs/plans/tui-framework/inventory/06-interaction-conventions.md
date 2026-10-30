# Inventory 06: Interaction Conventions (Layer 2) And Test/Preview Harnesses

Evidence-gathering notes for the TUI framework extraction plan
(`docs/plans/tui-framework/README.md`). Slice: key tables, the modal/overlay
contract, focus modality, effect/undo/clipboard/feedback conventions, the
snapshot dev mode, the cross-control consistency suite, the pantry preview
harness, and the hygiene tests that guard them. Read from the worktree at
branch `worktree-tui-framework` (base `ca668a3`). Nothing was run: no cargo,
no tests, no tmux. Every claim cites `path:line` read in this worktree; a few
behavioral claims are marked *(from reading, not run)*.

All paths are relative to the repository root. "L1" = layer 1 (components and
tokens), "L2" = layer 2 (interaction conventions and test kit), "L3" = layer 3
(app runtime: MVU loop, effect executor, terminal suspend/resume).

## 0. Map Of The Slice

| Primitive | Files | LOC | One-line verdict |
| --- | --- | --- | --- |
| Key matcher + overlay key tables (CSP-468 / ADR 0123) | `src/tui/keybindings/overlay.rs`, `keybindings.rs` (matcher half) | 834 + ~100 | **Extract after refactor.** The strongest L2 contract. Generic core `KeyMatcher`/`KeyRow<A>`/`Hint`/`resolve`/`footer`/`mislabeled`/`typed_char`/`key_label`; standard tables for standard components extract with it; app tables stay. |
| Global keymap (`KEYBINDINGS`, `translate`, `remap_for_focus`) | `src/tui/keybindings.rs`, `src/tui/keymap.rs` | 677 + 337 | **Stays in Conspectus; framework designs a fresh scoped keymap.** `Action`, `Msg`, `View`, `App` are wired into every row. Its shape (single mode, `fn() -> Action`, dead `help_text`) is not worth extracting. |
| Modal convention table (`MODAL_CONVENTION`, `EnterMeans`, `SpaceMeans`) | `overlay.rs:734-830`, `consistency_tests/keys.rs` | ~100 | **Extract after refactor** (generic convention plus a `KeyRole` classifier instead of matching `Debug` names). |
| `Overlay` trait, `OverlayOutcome`, `Modal` stack (CSP-497 / ADR 0085 c.3) | `src/tui/modal.rs`, `app/overlays.rs`, `runtime/overlay_keys.rs`, `runtime.rs:883-937`, `ui.rs:83-274` | 202 + 937 + 634 + ~200 | **Split.** Generic `Overlay<M, Cx>` + `Outcome<M>` + `ModalStack` in the framework; the `Modal` enum, 16 handlers, accessors, and draw ladder are Conspectus wiring that mostly should disappear once the stack is generic. |
| Help generation | `widgets/help.rs`, `widgets/viewer_help.rs:39`, `widgets/pane_viewer.rs:563` | 891 + 128 + ~60 (of 616) | **Split.** Page renderer and `sections_for<A>` generator extract (and de-duplicate); the 48 hand-written `Binding::new` prose rows and legends stay. |
| Focus modality | `keymap.rs:146-207`, `app.rs:360`, `app.rs:1517`, `ui.rs:331`, `actions.rs:79-105` | n/a | **Rules extract, wiring stays.** Framework owns the conventions and their tests; `Focus {Left, Right}` and `remap_for_focus` are app code. |
| Effect lifecycle + `undo_class` | `src/tui/effect.rs`, `src/tui/undo.rs` | 387 + 98 | **Stays** (L3 / domain). The pattern "exhaustive classifier fn plus a test that iterates variants" is the generic convention. |
| Clipboard (OSC 52) | `src/tui/clipboard.rs` | 137 | **Extract as-is, then fix the stdout coupling.** Zero `crate::` imports. |
| Message log | `src/tui/messages.rs` | 281 | **Split.** Bounded log with repeat-collapse and an unseen counter extracts; `LogTarget {Pin, Mux}` and `CommandRecord` are adapter-side. |
| Toast shim | `src/tui/widgets/toast.rs`, `app/overlays.rs:909-936` | 175 + 28 | **Extract after refactor** (theme-honoring border; drop the `Success`-green hardcode). |
| Snapshot dev mode | `src/tui/snapshot.rs`, `src/cli/tui.rs:85-150` | 571 | **Split.** Key DSL, ANSI emitter, `TestBackend` driver extract into a test kit; fixture loading and the `apply_action` subset stay. |
| Consistency suite | `src/tui/consistency_tests*` (22 files + root) | 6,434 | **Mostly stays.** About 30 of 204 tests are directly liftable into test-kit helpers, about 55 more are Conspectus-world instances of generic rules whose shape becomes a helper, about 115 are pure domain behavior (section 6.3). |
| Pantry preview harness | `examples/pantry.rs` | 1,030 | **Pattern extracts** (a generic gallery with a registry and the theme harness); the six variant structs stay as Conspectus fixtures. |
| Hygiene guards | `tests/overlay_keymap_hygiene.rs`, `tests/comment_hygiene.rs`, `tests/test_module_hygiene.rs`, `tests/tui_columns_doc.rs` | 136 / 124 / 87 / 251 | **Mechanisms extract** (source-scan lint, doc-frame sync); their rule sets are repo policy. |

Test volume in the slice (`#[test]` counts): 204 in `consistency_tests`
(per file in 6.3), 9 in `keybindings/overlay_tests.rs`, 3 in `keybindings.rs`,
9 in `clipboard.rs`, 5 in `widgets/toast.rs`, 8 in `messages_tests.rs`, 15 in
`snapshot_tests.rs`, 2 in `tests/overlay_keymap_hygiene.rs`.

---

## 1. The Key-Table Model

### 1.1 What a key binding is

There are two declarative layers that look alike and are not the same.

**Layer A: overlay tables (`KeyRow<A>`), the good one.**
`src/tui/keybindings/overlay.rs` (834 LOC, ADR 0123).

```rust
pub enum Hint { Shown { label: &'static str, verb: &'static str }, Alias }   // :21
pub enum KeyRow<A: 'static> { Bind { key: KeyMatcher, action: A, hint: Hint },
                              Note(&'static str) }                           // :35
pub fn resolve<A: Copy>(rows: &[KeyRow<A>], event: KeyEvent) -> Option<A>    // :46
pub fn footer<A>(rows: &[KeyRow<A>]) -> String                               // :57
pub fn mislabeled<A>(rows: &[KeyRow<A>]) -> Vec<(&'static str, String)>      // :102
pub fn typed_char(event: KeyEvent) -> Option<char>                           // :706
```

Each overlay owns an action enum `A` (12 enums defined in `overlay.rs`:
`SearchOptionKey` :188, `MenuKey` :221, `ControlsKey` :247, `MultiSelectKey`
:281, `TextInputKey` :312, `SearchKey` :351, `PagerKey` :427, `MessagesKey`
:487, `ConfirmKey` :534, `CloseDownKey` :553, `FormKey` :576, `OmniboxKey`
:622; plus 3 external message enums used as `A`: `PaneViewerMsg`,
`ViewerMsg`, `ViewerFindMsg`). The handler does `match resolve(TABLE, key)` and
matches `A` exhaustively, so a bound-but-ignored action fails to compile. The
footer is `footer(TABLE)`: rows sharing a `verb` join their labels with `/`
(`↑/↓ move`), segments join with ` · `, `Note` rows insert text, `Alias` rows
are bound but omitted. `mislabeled` flags `Shown` labels that do not name their
key (`label_names_key`, :115, knows `↑ ↓ ← → S-Tab Space PgUp PgDn` and
`Ctrl-<x>`, the latter case-insensitively).

Key-matcher variants (`KeyMatcher`, `keybindings.rs:65-80`, `matches` :84-102):

| Variant | Semantics | Used for |
| --- | --- | --- |
| `Exact { modifiers, code }` | exact modifier set | digits, `Ctrl-<c>` chords, Shift/Alt-Enter |
| `UpperChar(c)` | char with NONE or SHIFT only | `S R X N F E O`, `G` in the two viewers |
| `AnyModExceptCtrl(c)` | char, any modifier but CONTROL | the 20-odd lowercase single-char globals |
| `AnyMod(code)` | code only, modifiers ignored | arrows/nav, and every overlay `ch(c)`/`key(code)` builder |

Const builders at `overlay.rs:149-183`: `key(code)`, `ch(c)`, `ctrl(c)`,
`shown(...)`, `alias(...)`. They are private `const fn`; a framework must make
them public.

**Layer B: the global table (`KEYBINDINGS`), the weak one.**
`src/tui/keybindings.rs:121-543`: 58 `KeyBinding { mode, key, action: fn() ->
Action, help_text }` rows. Consumed by `translate_via_table`
(:548) only. Facts that matter for extraction:

- `KeyMode` has exactly one variant, `Global` (:51-54). The "mode / focus"
  column promised by the module doc (:1-17) and by CSP-468 never materialized.
  `remap_for_focus` (`keymap.rs:158-207`) is a 50-line hand `match` on `Msg`
  variants, not table-derived.
- `help_text` (58 strings) is **not read by any production code**. The only
  reference outside the table is a failure message in
  `widgets/help_tests.rs:71`. The help overlay renders from a separate
  hand-written `keymap_sections()` (`widgets/help.rs:307-496`, 48
  `Binding::new` rows with long prose).
- The "drift test" is `every_keybindings_entry_appears_in_help_sections`
  (`widgets/help_tests.rs:20`): it substring-matches `key_label(&binding.key)`
  against the haystack of help key strings, using a hand-maintained
  `equivalent_forms` map (`Ctrl-c -> Ctrl-C`, `Home|End -> g|G`,
  `1..5 -> "1 / 2 / 3 / 4"`). `key_label` itself lower-cases chord letters
  (`Ctrl-c`, `keybindings.rs:566-585`), while `viewer_help::key_notation`
  upper-cases them (`Ctrl-D`, `viewer_help.rs:72-85`).
- Seven keys are still hand-matched in `keymap::translate` outside the table:
  `PageDown`, `PageUp` (need viewport height, `keymap.rs:239-240`), `o` (:244),
  `i` (:251), `Tab` (:254), `J` (:255), `K` (:256). CSP-468 recorded "2 arms
  remain" (only the paging keys) when it closed; the code has 7. The drift test
  cannot see them.
- The duplicate-key test `no_two_bindings_match_the_same_global_key`
  (`keybindings.rs:628`) only compares `Exact` against `Exact`. Matching is
  first-hit-wins in table order, so ordering is load-bearing (Shift-Enter and
  Alt-Enter rows precede `AnyMod(Enter)`, :469-497) and **shadowed rows are
  undetectable**.
- `Action` (`keymap.rs:30-136`) carries `Box<Msg>`, `View`, and 16 overlay
  key-forwarder variants; the table cannot be extracted without inverting
  `Action` to a type parameter.

### 1.2 Scoping by pane / overlay / mode

| Scope | Mechanism | File:line |
| --- | --- | --- |
| Global (no overlay) | `translate` -> `translate_via_table` -> 7 hand arms | `keymap.rs:217-261` |
| Pane focus | `remap_for_focus(action, focus)` rewrites the *output* action (`NavDown -> ExplorerNavDown`, drops `h/l`, `Enter -> ExplorerEnter`, `F -> ExplorerToggleFullDetail`) | `keymap.rs:158-207` |
| Overlay | `overlay_key_from_event` returns `Action::<X>Key(key)` when the top modal is X; the global keymap never sees the key | `runtime.rs:883-937` |
| Sub-mode inside an overlay | handler-level: Search `Scope`/`Granularity`, viewer `find.prompt`, Controls sub-editors, `ConfirmKey` vs `MenuKey` inside WorktreeMenu | `search.rs:466`, `overlay_keys.rs:582`, `worktree_menu.rs:379-486` |

The order in `action_for_event` (`runtime.rs:878-881`) is the whole precedence
rule: overlay first, then `translate`, then `remap_for_focus`. The live loop
and the `--snapshot-keys` driver share it (a good property to preserve).

### 1.3 Rendering into footers and `?` help

Overlay footers: generated from the table at 24 call sites (grep for
`overlay::footer(` / `footer(<TABLE>_KEYS` outside tests). Three kinds of hint
text exist for the same overlay:

1. **In-modal footer**, generated (`row_menu.rs:164`, `mux_menu.rs:136`,
   `new_mux.rs:186`, `mux_launch.rs:504`, `pin_edit.rs:726`, `controls.rs:792`,
   `messages.rs:163`, `value_modal.rs:228`, `worktree_menu.rs:580-649`,
   `search.rs:839-996`, `help.rs:182,555`).
2. **Status-bar hint on open**, via `Msg::SetStatus(Some(..))`. Six sites
   generate it from the table (`app/overlays.rs:640,653`,
   `overlay_keys.rs:64`, `runtime.rs:547,699`); **four are hand-written
   literals that duplicate or drift from the table**:

   | Site | Literal | Table-generated text | Drift |
   | --- | --- | --- | --- |
   | `runtime.rs:1514` | `worktree: ↑/↓ move · Enter pick · Esc close` | `footer(MENU_KEYS)` = same | duplicate |
   | `runtime.rs:1548` | `close down: m merge · d discard · Esc cancel` | `m = merge & close · d = discard & close · Esc cancel` | different wording |
   | `runtime.rs:1587` | `mux: ↑/↓ move · Tab complete/next · Enter next/commit · Esc cancel` | `NEW_MUX_KEYS` adds `S-Tab back` | missing key |
   | `runtime.rs:1789` | `mux: ↑/↓ move · Enter pick · Esc close` | `footer(MENU_KEYS)` | duplicate |

   `tests/overlay_keymap_hygiene.rs:20-36` (`overlay_sources`) scans
   `src/tui/widgets/**`, `runtime/overlay_keys.rs`, and `modal.rs` only.
   `runtime.rs` is outside its walk, which is exactly where these four live.
3. **Main-view status-bar hints** (`ui/header.rs:481-509,597-660`): all
   hand-written (`"j/k move · h/l fold · Enter default"`, `"Tab focus"`,
   `"q quit"`, `"Enter/a attach {label}"`, ...) and priority-droppable via
   `fit_status_segments` (`header.rs:517`). They are not generated from
   `KEYBINDINGS` and nothing checks them.

The main `?` help is hand-written prose (`help.rs:307-496`). The two viewer
helps are **generated** from `KeyRow<ViewerMsg>` / `KeyRow<PaneViewerMsg>` plus
a per-message `help()` (section, description): `viewer_help.rs:39`
`sections_for(table)` and `pane_viewer.rs:563` `help_sections(table)` are
line-for-line copies that differ only in the message type. That generator
(table rows grouped by message, keys sorted plain-first/chords-last, joined
with ` / `) is the model the main help should adopt and the framework should
own as `help_sections<A: HelpAction>(&[KeyRow<A>])`.

### 1.4 Checks on the key tables

| Check | Where | Generic? |
| --- | --- | --- |
| Every `Shown` label names its key | `overlay_tests.rs:138` `every_overlay_table_labels_its_keys_truthfully` (hand list of 15 tables) | Yes. Needs a table registry; today's list omits `CONTROLS_KEYS`, `SEARCH_OPTION_KEYS`, `VIEWER_KEYS`, `PANE_VIEWER_KEYS` (the last two are all aliases) |
| Footer wording pinned | `overlay_tests.rs:165` `generated_footers_keep_the_established_wording` | Snapshot-style; domain strings |
| Overlay code never matches `KeyCode::` by hand | `tests/overlay_keymap_hygiene.rs:84` | Yes (path list is repo-specific) |
| No hand-written key hint literals in overlay code | `tests/overlay_keymap_hygiene.rs:112` (`looks_like_key_hint`: contains ` · ` and a word `Esc/Enter/Tab`) | Yes; coverage hole above |
| Esc leaves / Enter applies / Space changes per `MODAL_CONVENTION` | `consistency_tests/keys.rs:101,113,137` | Yes, after `KeyRole` refactor |
| Every Modal has a convention row, every row names a Modal | `keys.rs:72` | Pattern generic |
| Every non-empty help text, no two `Exact` collisions | `keybindings.rs:615,628` | Weak (see 1.1) |
| KEYBINDINGS entry present in help | `help_tests.rs:20` | Obsolete if help is generated |
| Ctrl-C is the first row, bound to Abort, in every table | convention in 19 of 20 tables (`OMNIBOX_KEYS` is one row); **no test** | Yes; trivial to add |

### 1.5 The 20 overlay tables, who consumes them, and divergences

| Table (enum) | Rows | Consumers (handler / footer) | Footer text (generated) |
| --- | --- | --- | --- |
| `MENU_KEYS` (`MenuKey`) | 7 | `row_menu.rs:92`, `mux_menu.rs:82`, `clean_up.rs:73`, `worktree_menu.rs:379` / `row_menu.rs:164`, `mux_menu.rs:136` | `↑/↓ move · Enter pick · Esc close` |
| `CONTROLS_KEYS` (`ControlsKey`) | 9 | `controls.rs:240` / `controls.rs:792`, `runtime.rs:547,699` | `↑/↓ move · Space select/toggle · Enter apply & close · Esc close · changes apply as you make them` |
| `MULTI_SELECT_KEYS` | 8 | `multi_select.rs:160` / **no render-path footer call** | `Space toggle · Enter confirm · Esc cancel` |
| `TEXT_INPUT_KEYS` | 4 | `input.rs:73` / `overlay_keys.rs:64` | `Enter confirm · Esc cancel` |
| `NEW_MUX_KEYS` (`FormKey`) | 7 | `new_mux.rs:124` / `new_mux.rs:186` | `↑/↓ move · Tab complete/next · S-Tab back · Enter next/commit · Esc cancel` |
| `MUX_LAUNCH_KEYS` (`FormKey`) | 10 | `mux_launch.rs:361` / `:504` | `... ←/→/Space option · Enter launch · Esc cancel` |
| `PIN_EDIT_KEYS` (`FormKey`) | 11 | `pin_edit.rs:568` / `:726` | `... · Ctrl-R follow again · Enter save · Esc cancel` |
| `PICKER_KEYS` (`SearchKey`) | 7 | `picker.rs:141` / **none (all aliases, footer is "")** | `` |
| `SEARCH_KEYS` (`SearchKey`) | 9 | `search.rs:466-471` / `search.rs:839-996`, `overlays.rs:640` | `↑/↓ move · Enter pick · Esc close · Tab current/all` |
| `SEARCH_CONTENT_KEYS` | 14 | same | `... · S-Tab sessions/messages · Ctrl-R earlier search · Ctrl-O options · Ctrl-F filter lists · Ctrl-S Search view` |
| `SEARCH_OPTION_KEYS` | 11 | `search.rs:550` | `↑/↓ move · Space toggle · Esc back` |
| `HELP_KEYS` (`PagerKey`) | 15 | `help.rs:40` / `help.rs:182,555`, both viewer helps | `j/k/PgDn/PgUp to scroll · Esc/q/? to close` |
| `VALUE_MODAL_KEYS` (`PagerKey`) | 16 | `value_modal.rs:94`, `overlay_keys.rs:310` / `value_modal.rs:228` | `y to copy · Esc/q/o to close · j/k to scroll` |
| `MESSAGES_KEYS` | 18 | `messages.rs:61`, `overlay_keys.rs:353` / `messages.rs:163` | `j/k select · J/K scroll detail · y copy · Esc close` |
| `CONFIRM_KEYS` | 5 | `worktree_menu.rs:420,444,466` | `Enter/y to confirm · Esc/n to cancel` |
| `CLOSE_DOWN_KEYS` | 5 | `worktree_menu.rs:485` | `m = merge & close · d = discard & close · Esc cancel` |
| `OMNIBOX_KEYS` | 1 | `path_omnibox.rs:107` | n/a |
| `PANE_VIEWER_KEYS` (`PaneViewerMsg`) | 21 | `pane_viewer.rs:384`; help `:593` | all aliases; viewer draws its own footer |
| `VIEWER_KEYS` (`ViewerMsg`) | 24 | `overlay_keys.rs:594`, `viewer_help.rs:91` | all aliases (ADR 0052 boundary) |
| `VIEWER_FIND_KEYS` (`ViewerFindMsg`) | 10 | `overlay_keys.rs:587`, `pane_viewer.rs:377` | `Enter find · Esc cancel` |

Total: roughly 210 rows over 20 tables.

**Deviations for the "same" key across surfaces** (each is a behavior choice for
the dedup phase; counts and locations):

| Key | Divergence |
| --- | --- |
| `g` / `G` | `ch('G')` = `AnyMod(Char('G'))` in `HELP_KEYS` :456, `VALUE_MODAL_KEYS` :478, `MESSAGES_KEYS` :511 (Alt-G or Ctrl-Shift-G also jump to the bottom); `UpperChar('G')` in `PANE_VIEWER_KEYS` :653 and `VIEWER_KEYS` :677 (plain or Shift only; Ctrl+`G` is ignored, pinned by `overlay_tests.rs:223`). `ch('g')` is `AnyMod` everywhere, so **Ctrl-G jumps to the top of every pager and viewer** while Ctrl-G is also the global "cycle grouping" chord (`keybindings.rs:180`) |
| `y` | `ch('y')` = AnyMod in `VALUE_MODAL_KEYS` :465 and `CONFIRM_KEYS` :543 (Ctrl-Y copies/confirms); `AnyModExceptCtrl('y')` in `MESSAGES_KEYS` :519 |
| `j`/`k` | alias-only in menus/controls/multi-select/search-options; **shown** in pagers (`j/k to scroll`) and messages (`j/k select`); absent in text-bearing overlays by design |
| arrows in footer | shown (`↑/↓ move`) in menu, controls, forms, search; **alias-only in multi-select** (its footer is never drawn anyway) |
| `Ctrl-N`/`Ctrl-P` | bound in `SEARCH_KEYS`, `SEARCH_CONTENT_KEYS`, `SEARCH_OPTION_KEYS`, `PICKER_KEYS`; **not** in menus, controls, multi-select, forms |
| `Space` | toggles (controls, multi-select, search-options), steps option (`MUX_LAUNCH`, `PIN_EDIT`, shown as `←/→/Space option`), pages (pagers, aliased), types (text overlays) |
| `Enter` | wording: `pick`, `open hit`, `apply & close`, `confirm`, `to confirm`, `next/commit`, `launch`, `save`, `find` (nine verbs). Semantics: `NEW_MUX` advances fields until the last, `MUX_LAUNCH`/`PIN_EDIT` submit from any field |
| `Esc` wording | `close` (menu, controls, search, messages), `to close` (help, value modal), `cancel` (multi-select, text, forms, close-down, find), `to cancel` (confirm), `back` (search options). Partly principled (cancel = something pending), but three footer dialects exist: `<keys> <verb>`, `<keys> to <verb>`, `<key> = <verb>` |
| Close feedback | six overlays set `"<x>: cancelled"` in the status bar on `Close` (rename, worktree, new_mux, mux_menu, mux_launch, pin_edit: `overlay_keys.rs:88,119,147,180,290,624`); eight close silently (row_menu, clean_up, picker, value_modal, messages, help, controls; search sends `DiscardContentPreview`) |
| Matcher default | global chars use `AnyModExceptCtrl`; overlay chars use `AnyMod` (Ctrl-J, Alt-K also navigate) |

### 1.6 Key notation exists in four dialects

1. Footer/help labels: `Ctrl-R`, `S-Tab`, `↑`, `Space` (`Hint::Shown.label`).
2. `key_label()` (`keybindings.rs:566`): `Ctrl-r`, `Up`, `PgUp`.
3. `viewer_help::key_notation` (`:72`): re-maps to `↑`, upper-cases chords.
4. The snapshot key DSL: `<C-r>`, `<S-Tab>`, `<Up>` (`snapshot.rs:481-567`).

There is no single `KeySpec` that parses and formats; the framework should have
one (`KeySpec::parse("<C-r>")`, `.label()`, `.matches(KeyEvent)`).

### 1.7 Verdict, key tables

- **Name**: `keys`. **Extract after refactor.**
- Move out: the three `use crate::` lines at `overlay.rs:634-636`
  (`PaneViewerMsg`, `ViewerFindMsg`, `ViewerMsg`; class b) and the
  viewer/search/worktree/mux tables (class b).
- Keep in framework: `KeyMatcher`, `KeyRow`, `Hint`, `resolve`, `footer`,
  `mislabeled`, `typed_char`, `key_label`, builders (made `pub`), and the
  *standard* tables that go with standard controls: menu (`MENU_KEYS`),
  confirm (`CONFIRM_KEYS`), pager (`HELP_KEYS`, `VALUE_MODAL_KEYS`), text
  input (`TEXT_INPUT_KEYS`), multi-select (`MULTI_SELECT_KEYS`), form
  (`NEW_MUX_KEYS`, `MUX_LAUNCH_KEYS`, `PIN_EDIT_KEYS` share `FormKey` and differ
  only in rows and footer wording). That is 9 tables over 6 enums (`MenuKey`,
  `ConfirmKey`, `PagerKey`, `TextInputKey`, `MultiSelectKey`, `FormKey`).
- Add: a `KeyTable` registry trait so `mislabeled`, "Ctrl-C first", and
  "Esc leaves" run over all registered tables; shadow detection; `KeySpec`.
- Fix on the way: `AnyMod` vs `UpperChar` for the same letter, `Ctrl-<c>`
  case, `Hint` verb dialects (one grammar), `footer` for tables whose rows are
  all aliases.

---

## 2. The Overlay / Modal Contract

### 2.1 Surface

`src/tui/modal.rs` (202 LOC):

```rust
pub enum Modal { Help(..), Controls(..), Rename(RenameOverlayState), Search(..),
  ValueModal(..), Messages(..), Viewer(ViewerState), PaneViewer(Box<..>),
  WorktreeMenu(Box<..>), NewMux(..), MuxMenu(..), MuxLaunch(Box<..>),
  PinEdit(Box<..>), RowMenu(Box<..>), CleanUp(Box<..>), Picker(Box<..>) }   // :32-96, 16 variants

pub enum OverlayOutcome { Consumed, Commit(Box<Msg>), CommitAndStay(Box<Msg>), Close }  // :107

pub trait Overlay { type Ctx<'a>;
    fn handle(&mut self, ctx: Self::Ctx<'_>, key: KeyEvent) -> OverlayOutcome; }          // :137
```

`App::modal_stack: Vec<Modal>` (`app.rs:267`). Lifecycle:

- **Open**: a per-overlay `open_*` that pushes (`app/overlays.rs`, 71 `pub fn`s
  for 16 variants: peek `x()`, peek-mut `x_mut()`, `open_x`, `close_x` per
  variant; `close_x` pops only when the top matches, tested by
  `app_tests.rs:2054`).
- **Key routing**: `overlay_key_from_event` (`runtime.rs:883-937`) is a
  16-branch `if app.x().is_some()` ladder; each accessor peeks only
  `modal_stack.last()`, so only the top routes. It emits one of 16
  `Action::<X>Key(KeyEvent)` variants (`keymap.rs:43-117`), and `LiveMode::dispatch`
  (`runtime.rs:520-595`) / `StaticMode::dispatch` (:675-804) / `snapshot::apply_action`
  (`snapshot.rs:289-304`) each carry their own arm per variant.
- **Handle -> outcome**: one handler per overlay in `runtime/overlay_keys.rs`
  (16 fns: rename :72, worktree :103, new_mux :131, mux_menu :164, row_menu :194,
  clean_up :221, picker :248, mux_launch :274, value_modal :305, messages :347,
  help :383, search :423, controls :495, pane_viewer :558, viewer :573, pin_edit
  :608). Fourteen share one skeleton: `match outcome { Consumed => {}, Close =>
  close_x(), Commit(msg) => { close_x(); dispatch(msg) }, CommitAndStay(msg) =>
  dispatch(msg) }`. The only differences are the dispatch function
  (`dispatch_live` with terminal/tmux handle for nine, pure `dispatch` for
  value_modal/messages/help/controls) and whether `Close` also writes a
  `"<x>: cancelled"` status (6 of 14).
- **Draw**: `ui::draw` calls 15 `draw_*` helpers in a fixed order
  (`ui.rs:90-104`), each `let Some(state) = app.x() else { return }` (so only
  the top modal draws). Ten bodies are the identical
  `frame.render_widget(XWidget::new(state, app.theme()), area)`
  (`mux_menu`, `row_menu`, `picker`, `clean_up`, `mux_launch_form`,
  `pin_edit_form`, `worktree_menu`, `new_mux_form`, `value_modal`, `help`);
  four are bespoke (`search` builds items + outcomes, `controls` builds context,
  `messages` passes the log and an epoch, `rename` uses `TextInputWidget`).
  Full-screen modals (`Viewer`, `PaneViewer`) bypass `ui::draw` entirely in
  `draw_screen` (`runtime.rs:843-861`), so toasts and the status bar are not
  drawn over them.
- **Close**: pop by variant (`close_x`), or the reducer pops after
  `Commit`.

### 2.2 What the docs promise vs what exists

| Promise | Reality |
| --- | --- |
| ADR 0085 c.3 / `modal.rs:1-9`: "`draw` renders the stack in order (bottom-to-top)" | Only the top modal draws. The stacking tests (`app_tests.rs:2030-2094`) assert accessor results, never frames. A modal under another is invisible, and `ui.rs:90-104` has a fixed call order unrelated to the stack |
| ADR 0085 c.3 / CSP-497: trait has `render(&self, frame, area, &Theme)` | `Overlay` has **no** `render`. Widgets are separate `Widget` structs constructed in `ui.rs` |
| `Commit(Msg)` | `Commit(Box<Msg>)` plus a fourth outcome `CommitAndStay(Box<Msg>)` used only by Controls (`controls.rs:1333`) and Search regroup (`search.rs:1498`) |
| "Adding a modal: struct + impl + variant, zero new loop/keymap/draw branches" (`modal.rs:6-9`) | A modal added today (Picker is the newest; grep of its names outside `widgets/picker.rs`) touches 9 production sites besides its openers: `modal.rs` (variant), `keymap.rs:66` (`PickerKey`), `runtime.rs` (dispatch arm :542, StaticMode disabled-list entry :797, key-ladder branch :918), `overlay_keys.rs` (handler :248), `app/overlays.rs` (4 methods), `ui.rs` (draw fn :125 + call :99), `keybindings/overlay.rs` (table), plus its openers (`app.rs:1958`, `app/content.rs:1129`) and, in tests, the registries (`layout.rs` `builders` + `name_of` + `press`, `keys.rs` `tables`, `MODAL_CONVENTION` row). The compiler checks some (exhaustive `name_of`, `press`) but **nothing checks that the key ladder routes the new modal** (routing tests exist only for Help and Controls, `runtime_tests.rs:900-909`). A missing ladder branch silently sends the modal's keys to the global keymap |
| ADR 0085 c.1: the four update layers collapse into `Msg` | `Action` still has 40 variants; 16 are `*Key` forwarders, 9 are `Open*`, the rest effectful or view actions. `Msg` has roughly 110 variants (`app/msg.rs`, 500 LOC) |
| `Overlay` implemented by every modal | 14 of 16 variants: `Viewer` (nested reducer via `Msg::Viewer(ViewerMsg)`, `app.rs:1643-1667`) and `PaneViewer` (`handle_key -> PaneViewerOutcome`) are not |

### 2.3 The `Ctx` problem

CSP-497 left Controls and Pins (and Search) off the trait because their
handlers "need widget-side context". The fix was a GAT, `type Ctx<'a>`. Current
implementors (14, `grep "Overlay for"`):

| Ctx | Implementors |
| --- | --- |
| `()` (11) | Help, ValueModal, Rename (`modal.rs:189`), WorktreeMenu, NewMux, MuxMenu, MuxLaunch, PinEdit, RowMenu, CleanUp, Picker |
| `&'a ControlsContext<'a>` | Controls (`controls.rs:1326`): 11 domain fields (view, grouping, filter, sort, mux_recency, preview_wrap, session_preview, content, search_kinds, content_group, search_history) |
| `SearchContext<'a>` | Search (`search.rs:1448`): `items`, `content` rows, optional `EntityIndex` |
| `&'a MessageLog` | Messages (`messages.rs:57`) |

Why the GAT does not make dispatch generic: the host still needs a `match` per
`Modal` variant to build that variant's `Ctx`, so the 16 handlers remain.
Practical costs visible in the code:

- Borrow conflicts: `modal_stack` is a field of `App`, so `&mut modal` and
  `&App` cannot coexist. `handle_controls_overlay_key`
  (`overlay_keys.rs:495-531`) clones the view/grouping/filter/content state into
  locals to build an owned context before touching `app.controls_overlay_mut()`;
  `handle_search_overlay_key_with` (:423-491) builds owned `items`/`contents`
  vectors per key; `messages_overlay_mut` (`overlays.rs:759-769`) hand-splits
  disjoint fields to return `(state, &MessageLog)`.
- `Modal` is a closed enum in the app crate, so the stack cannot be generic
  over it without trait objects, and trait objects cannot carry per-variant
  GATs.

### 2.4 `Msg` / `App` leaks and how to invert them

| Leak | Where | Class | Inversion |
| --- | --- | --- | --- |
| `OverlayOutcome::Commit(Box<Msg>)` | `modal.rs:113,118` | (a) generic | `Outcome<M>`; framework never names `Msg` |
| Widgets construct `Msg::*` directly | `worktree_menu` 6, `mux_menu` 6, `picker` 2, `mux_launch` 2, `new_mux` 2, `clean_up` 1, `pin_edit` 1, `search` 7, `controls` 24 | (a)/(b) | Generic widgets return their own result (`String`, `usize`, `Vec<usize>`, `bool`, selected entry); host maps with `Outcome::map`. `RenameOverlayState` (`modal.rs:166-202`) is already this wrapper pattern, generalized |
| `RowMenuEntry.msg: Msg` | `row_menu.rs:28` | (a) | `MenuEntry<M>` generic over payload; a Conspectus-side `row_menu_for(app)` (`actions.rs:679`) builds `Vec<MenuEntry<Msg>>` |
| `Ctx` = host types | Controls, Search, Messages | (b)/(c) | These three overlays are Conspectus-specific (views, filters, content search, op log) and stay in the app, implementing the framework trait with the app's own `Cx` |
| `Action::<X>Key` wrappers | `keymap.rs` | (b) | Disappear: host calls `stack.route(key, &mut host_cx) -> Option<Outcome<M>>` and handles the outcome itself (it needs `terminal` only to run `dispatch_live`, which it can do after `route` returns) |
| `Theme` | all widgets | L1 token | Framework theme crate (inventory 01) |
| `crate::viewer::render::word_wrap` | `help.rs:183,559` | (b) | Framework text util |

### 2.5 Proposed framework API (shape, not final)

```rust
pub enum Outcome<M> { Consumed, Close, Commit(M), CommitAndStay(M) }
impl<M> Outcome<M> { pub fn map<N>(self, f: impl FnOnce(M) -> N) -> Outcome<N>; }

/// Cx is chosen by the host, usually a read-only view of its model.
pub trait Overlay<M, Cx: ?Sized> {
    fn handle(&mut self, cx: &Cx, key: KeyEvent) -> Outcome<M>;
    fn render(&self, cx: &Cx, theme: &Theme, area: Rect, buf: &mut Buffer);
    fn footer(&self) -> Option<String> { None }      // table-generated hint
    fn convention(&self) -> &'static ModalConvention;
}
pub struct ModalStack<M, Cx> { entries: Vec<Box<dyn Overlay<M, Cx>>> }
// push, pop_if::<T>(), top(), route(key, &Cx) -> Option<Outcome<M>>, draw(&Cx, ..) bottom-to-top
```

Two ownership options to settle the borrow problem (open question 1): the
stack lives beside the model (not in it), or `route` uses take-handle-put (the
pattern `Msg::Viewer` already uses, `app.rs:1646-1667`). Generic overlays
(`Help`/`ValueModal` pager, text input, menu, confirm, multi-select, picker,
form) implement `Overlay<M, Cx>` for **any** `Cx`; Conspectus's Controls,
Search, Messages, and domain forms implement it for `Cx = App`'s read view.

### 2.6 Per-overlay variant notes (what differs between instances)

| Overlay | Result type it should return | Differs from siblings by |
| --- | --- | --- |
| RowMenu | `MenuEntry<M>` (+ confirm arming) | `confirm` second-Enter arming (`row_menu.rs:106-112`); width 72, min height 8 |
| MuxMenu | enum of 2 | **near-copy of RowMenu** with 2 hard-coded entries, width 62, min height 10, no arming, title as first body line (`mux_menu.rs:52,117-153`) |
| WorktreeMenu | menu / branch input / confirm / close-down state machine | switches among `MENU_KEYS`, `CONFIRM_KEYS` (3 states), `CLOSE_DOWN_KEYS`, `TEXT_INPUT_KEYS` inside one overlay |
| CleanUp | `Vec<String>` ids | wraps `MultiSelectWidget`; own empty-state text (`clean_up.rs:110-125`) with a hand-written `Esc/Enter close` hint |
| Picker | selected value | type-to-filter, no footer, all aliases |
| Rename / TextInput | `String` | wrapper type exists only for `Msg::CommitRename` mapping |
| NewMux, MuxLaunch, PinEdit | form result | three near-copy `handle` bodies (see 9.1 item 7) |
| Help, ValueModal | none | pagers; ValueModal adds `y` copy handled in the runtime (`overlay_keys.rs:310-321`) |
| Messages | none (copy handled in runtime, :353-364) | list-detail pager, takes `&MessageLog` |

Verdict, modal contract: **name `modal`. Split.** Extract `Outcome<M>`,
`Overlay<M, Cx>`, `ModalStack`, the convention table, and the generic
overlays; keep the `Modal` enum's domain variants and handlers as Conspectus
adapters until the generic stack removes the need for 16 handler fns and 16
`Action` wrappers.

---

## 3. Focus Modality

Rules as they exist, with the evidence:

| Rule | Evidence |
| --- | --- |
| Two panes only: `Focus { Left, Right }`; `Tab` toggles (`Msg::CycleFocus`, hand-matched in `translate`) | `app.rs:360`, `app.rs:1517`, `keymap.rs:254` |
| Navigation keys follow focus: `j/k/PgUp/PgDn/g/G/Home/End` become explorer msgs, `Enter` becomes `ExplorerEnter`, `F` becomes `ExplorerToggleFullDetail`, `e` becomes group toggle; `h/l/<-/->` are dropped | `keymap.rs:158-207` |
| Some keys ignore focus on purpose: `J/K` always scroll the preview; `Tab` is the toggle | `keymap.rs:155-157,185-187,255-256` |
| An unfocused pane shows no highlighted row | `consistency_tests/rendering.rs` `an_unfocused_pane_shows_no_highlighted_row`; focus marker `► ` / two spaces keeps title width constant (`ui.rs:331-335`) |
| Accelerators act on the focused pane's entity, not always the selected row | `actions.rs:79-105` `Subject`/`action_subject`; `consistency_tests/focus.rs` (4 tests), `actions.rs` |
| An open overlay owns all keys; background focus is untouched; `Tab` is swallowed in text overlays | `runtime.rs:883`; `TextInputKey::Tab` (`overlay.rs:317,329`, handled `input.rs:77`) |
| `Backspace` pops a drill-down hop; with an empty stack, press twice to return focus left | `explorer_nav.rs:324-348` (`explorer_back_armed`) |
| Status bar names the focus scope `[left]/[right]` and changes its hints | `ui/header.rs:201-209,481-490` |

Classification: the **rules** are generic UI conventions (focus follows the
accent marker, navigation follows focus, unfocused panes are dimmed, one gesture
acts on the focused entity, overlays steal all input). The **wiring** is
Conspectus-specific: `Focus` is a two-pane enum, `remap_for_focus` rewrites
`Msg` variants, `action_subject` resolves graph nodes. Framework verdict
(**name `focus`**): ship a `PaneFocus<P>` helper (ring over named panes, `next`/`prev`, `is_focused(pane)`),
the accent-marker/dimming style tokens (L1), and the testable rule
"unfocused pane draws strictly fewer reversed cells". Keep `remap_for_focus` in
the app; replace its hard-coded `match` with a per-pane `Keymap` layer once the
global keymap is redesigned. `selected_default_action` (`keymap.rs:282-337`)
and `SelectedDefault` stay (domain).

---

## 4. Effects, Undo, Clipboard, Feedback

### 4.1 Effect lifecycle (`src/tui/effect.rs`, 387 LOC)

`Effect { SpawnRefresh, Toast, Persist, Quit, Exec(ExecSpec), RunMux(MuxOp),
WriteStore(StoreOp) }` (:17), `ExecSpec` 8 variants (:62), `StoreOp` 18 variants
(:137). `Lifecycle { attaches, freezes_row, refreshes, follows_entity }` (:228)
plus `lifecycle(&Effect) -> Lifecycle` (:295): an exhaustive `match` so "a new
effect cannot ship without deciding". `RefreshRequest { classes }` (:262) names
provider classes to re-probe. `UndoClass` + `undo_class(&StoreOp)` (:354-387)
is the same pattern for reversibility, with a mandatory reason string.

Everything in it is domain (mux attach, pin store, worktrees, provider
classes). The only generic content is the *pattern*:

1. effects are data, 2. every effect has a total classifier function with no
wildcard arm, 3. a test iterates all variants (`consistency_tests/lifecycle.rs`
`every_store_write_refreshes_the_views`, `freshness.rs`
`every_refreshing_effect_forces_a_live_rescan`), 4. a classifier outcome that
needs justification carries a reason string (`NotUndoable(&'static str)`).

**Verdict: stays in Conspectus (L3 / domain).** The framework documents the
pattern. `Lifecycle` field names (`attaches`, `freezes_row`, `follows_entity`)
are not reusable; a generic `EffectPolicy` trait would be speculative. L3
boundary note: `Effect`, `ExecSpec`, `execute_effects(_live)`
(`runtime.rs:1975-2020`), `ExecutorKind`/`live_executor_for`/
`pure_executor_runs` (:2028-2050), `LoopMode`/`run_loop` (:81,124),
`WorkerLedger`, and terminal suspend/resume (`runtime/launch.rs:521-642`:
`ratatui::restore()` / `ratatui::init()` around child processes) are all L3.
The L2 contract that must survive without L3: overlays return `Outcome<M>`,
the host's reducer owns `M`, and `Cx` is a host type.

One executor-level UI convention is generic and worth stating: a gesture the
current executor cannot run must say so instead of vanishing
(`consistency_tests/lifecycle.rs:54`
`the_plain_executor_says_so_when_handed_what_it_cannot_run` checks that a dropped
store write produces a toast).

### 4.2 Undo (`src/tui/undo.rs`, 98 LOC)

`UNDO_DEPTH = 20`; `Undo { Unpin, RestorePin }`; `UndoStack` is a bounded
`VecDeque` with `push/pop/len/is_empty`; `Undo::into_op() -> StoreOp`;
`Undo::focus(in_pinned_view) -> Option<RestoreFocus>`. The key is
`UNDO_KEY = 'u'` (`keybindings.rs:57`) bound as `AnyModExceptCtrl`; toasts that
offer an undo name it from that constant. Executor records the inverse **after**
the write succeeds (`Msg::UndoRecorded`), reducer pops on `Msg::Undo`, empty
stack yields toast `nothing to undo`, undo never switches views
(`consistency_tests/undo.rs:229`).

Generic: bounded history, "undo key is bound once and globally"
(`consistency_tests/undo.rs:84`), "every irreversible op states why", "undo restores the cursor
without leaving the view". Domain: both `Undo` variants, `RestoreFocus`
(`RowId`, `NodeId`). **Verdict: stays**; the framework may offer a
`BoundedStack<T>`, but `MessageLog` (VecDeque cap 200) and `UndoStack`
(VecDeque cap 20) are the only two copies in `src/tui` (the third, `SearchHistory`,
is a `Vec` in `app/content_history.rs:56`), so the saving is small.

### 4.3 Clipboard (`src/tui/clipboard.rs`, 137 LOC)

`copy_to_clipboard(&str) -> io::Result<()>` writes OSC 52 to **real stdout**
(:19-23); `write_osc52<W: Write>` (:28) is private and test-only; `base64_encode`
is hand-rolled (ADR 0056). Zero `crate::` imports; 9 unit tests with RFC 4648
vectors. Callers: Value modal `y` (`overlay_keys.rs:316`), Messages `y` (:359),
`i` copy-session-id and `Enter` on a Node field (`runtime`). Snapshot mode
replaces the call with a closure `|_| {}` (`snapshot.rs:315`) because the OSC
bytes would land in the frame output.

**Verdict: extract as-is, then add a `ClipboardSink` (writer or trait) so the
host/test-kit can route it.** Unchanged hazards: no tmux passthrough wrapping, fire-and-forget (the
doc says callers "should still surface a copied toast", and they do via
`Effect::Toast`/`post_toast`), and an empty payload clears the clipboard
(`clipboard.rs:122-128`).

### 4.4 Three feedback channels (status line, toast, message log)

| Channel | API | Lifetime | Call sites (non-test) |
| --- | --- | --- | --- |
| Status line | `Msg::SetStatus(Option<String>)` -> `App::status_message` (`app.rs:1540`), drawn by `draw_status_bar` (`header.rs:188-197`) replacing the key hints; cleared by navigation (31 `status_message =` assignments) | until next selection/focus change | 49 `Msg::SetStatus(` |
| Toast | `Effect::Toast(String)` -> `App::post_toast` (`overlays.rs:922`): drains the engine, shows a bottom-centered bordered toast for `TOAST_DURATION = 1500 ms`; `prepare_toast_for_render` ticks and re-areas each draw (:933) | 1.5 s | 40 `Effect::Toast(` + 12 `post_toast(` |
| Message log | `Msg::Report(Box<LogEntry>)` -> status summary (+ ` · ! details` if warning/error) and `MessageLog::push` (`app.rs:1543-1550`); `!` opens the overlay (ADR 0105); unseen chip `▲ N · ! messages` leads the status bar (`header.rs:232-241`) | 200 entries, session only, never persisted (ADR 0086) | 36 `app.report(` |

Toast shim (`widgets/toast.rs`): wraps `ratatui_comfy_toaster::ToastEngine<()>`;
fixed `ToastType::Success`, `Color::Reset` background, `ToastBorderMode::Full`,
position `Center` + offset `(0, i16::MAX)` to pin to the bottom edge
(:57-65); `engine_dismiss_all` implements replace-not-queue (:71); the
`ToastEngineHolder` newtype (:81-103) exists only so `App` can derive `Debug`.
Known defect, documented in the file header: the border color is upstream's
hard-coded `Success -> Green`, so operator `[tui.theme] success` overrides do
not reach toasts.

`MessageLog` (`messages.rs`): `LogLevel {Info, Warning, Error}` with glyphs
`√ ▲ ×` and `needs_attention()`; `LogEntry` with `at_epoch`, `summary`,
optional `target: LogTarget {Pin, Mux}`, optional `CommandRecord {argv,
exit_code, stdout, stderr}`, optional `detail`, `repeats`; `push` collapses an
entry identical to the newest apart from time into a repeat count and does not
count it as a new unseen failure (:221-240); `latest_failure_for(targets)`
lets a later success hide an older failure (:267).

Classification:

| Piece | Class |
| --- | --- |
| `MessageLog` (bounded, repeat-collapse, unseen counter), `LogLevel` + glyph/attention rule, `clock()` | (a)/generic: extract. `LogEntry::new` calls `crate::discovery::current_epoch()` (`messages.rs:88`): inject the clock |
| `LogTarget::{Pin, Mux}`, `CommandRecord::from_output(&Output)`, `full_text()` text ("target: pin ...") | (b): make `target: Option<T>` / `Box<dyn Any>`, keep `CommandRecord` as a generic "subprocess record" or app-side |
| Messages overlay widget | L1 (inventory of widgets) |
| `Msg::Report` hint text `" · ! details"` hard-codes the `!` key | (b): derive from the keybinding that opens the log |
| Choosing channel per event (toast vs status vs log) | convention: **navigation hint -> status; transient confirmation -> toast; outcome of an operation -> log (+ status summary)** (ADR 0105 decision 1) |

Framework API sketch: `Feedback { status: Option<String>, toasts: Toasts,
log: MessageLog<T> }` with `status()`, `toast()`, `report()`; host maps
effects onto it. **Verdicts**: toast: extract after refactor; log: split; status
line: convention only (the message string is data).

---

## 5. The Modal Key Convention

`MODAL_CONVENTION` (`overlay.rs:782-830`) is a table of 17 rows
(`modal: &'static str`, `enter: EnterMeans`, `space: SpaceMeans`,
`exception: &'static str`):

- Esc leaves the modal without applying anything pending (Controls applies as
  you go, so what it already applied stays; exception stated).
- Enter applies/submits (`Submits`), activates the focused item
  (`Activates`), or does nothing special (`Nothing`: viewers/pagers).
- Space changes the focused item (`Toggles`), pages (`Pages`, pagers), or types
  (`Types`).

Enforced by `consistency_tests/keys.rs`: `every_modal_has_a_row_in_the_convention`
(:72), `esc_leaves_every_modal` (:101), `enter_applies_or_activates_where_the_convention_says_it_does`
(:113), `space_changes_the_focused_item_where_the_convention_says_it_does` (:137),
`a_departure_from_the_convention_gives_its_reason` (:161).

The tests classify an action by the **`Debug` name of the enum variant**
(`LEAVES = ["Cancel","Close","No","Back"]`, `APPLIES = ["Confirm","Submit","Pick","Yes","Done","Merge","Discard"]`,
`CHANGES = ["Toggle","OptionNext"]`, :65-69) and "Page" by substring. That is
brittle (a renamed variant silently stops being checked) and is the thing to
replace with `trait KeyAction { fn role(&self) -> KeyRole }`
(`Leave | Apply | Change | Page | Move | Other`). The `tables(modal)` name->tables
map (`keys.rs:26-51`) is a fourth hand-kept registry of modals (see 9.2).

**Verdict: extract after refactor** into `convention` with `KeyRole`, with the
17 rows moving to the app as data.

---

## 6. Test And Preview Harnesses

### 6.1 Snapshot dev mode (`src/tui/snapshot.rs`, 571 LOC; ADR 0067/0068/0113)

Pipeline (`snapshot.rs:101-175`): build `App` -> load fixture or run live
discovery (`refresh`) -> parse key script -> for each key run
`runtime::action_for_event` then `apply_action` -> `app.search_overlay_tick`
and `run_pending_content_scans` after each key -> load the selected preview ->
draw once to `TestBackend` via the same `runtime::draw_screen` the live loop
uses -> slice a region (`SnapshotPane::{All, Header, Left, Right, Status}`) ->
`buffer_to_ansi` -> stdout. Flags (`cli/tui.rs:85-150`): `--snapshot`,
`--snapshot-width/height` (default 160x40), `--snapshot-keys`,
`--snapshot-pane`, `--snapshot-fixture`, `--snapshot-fixture-root`,
`--snapshot-now`, `--snapshot-export-fixture`. Gated by Cargo feature
`snapshot` (`Cargo.toml:76-82`).

| Component | Lines | Generic? |
| --- | --- | --- |
| Key DSL `parse_key_script`/`parse_named_key`/`literal_to_key` | 481-567 | **Yes.** Literals; uppercase letter implies SHIFT; `<Up><Down><Left><Right><Enter><Esc><Tab><BackTab><Space><Backspace><Delete><Insert><Home><End><PageUp><PageDown><F1..F12>`, `<C-x>`, `<A-x>`, `<S-x>` nestable; unterminated or unknown names are errors |
| `buffer_to_ansi` + `ansi_style_seq` + `color_to_seq` | 382-464 | **Yes.** Minimal SGR transitions per cell, reset at row end, 16-color, indexed, RGB; handles BOLD DIM ITALIC UNDERLINED REVERSED. Missing: CROSSED_OUT, SLOW/RAPID_BLINK, HIDDEN, underline color |
| `TestBackend` one-frame driver | 156-175 | **Yes** |
| `pane_rect` | 343-376 | Pattern generic; **duplicates the layout** (`Length(1)/Min(3)/Length(1)`, 50/50, narrow threshold) instead of sharing it with `ui::draw`/`draw_body` (`ui.rs:85,312-321`). The comment claims constants are shared via `pub(super)`; they are not |
| `apply_action` | 256-323 | Conspectus. A *second* copy of the "safe subset" dispatch next to `LiveMode::dispatch` and `StaticMode::dispatch`. Handles 6 of the 16 overlay key actions (Messages, Controls, Search, Help, Viewer, PaneViewer); the other 10 (Rename, WorktreeMenu, NewMux, MuxMenu, RowMenu, CleanUp, Picker, MuxLaunch, PinEdit, ValueModal) are skipped with a stderr line, so `--snapshot-keys ".jjj"` opens the row menu but cannot move its cursor |
| Fixture load/export, `freeze_clock`, `FIXTURE_PATH_ROOT` rebasing | 52-96,177-196,221-250 | Conspectus (graph data) |
| `SnapshotConfig` | 55 | Mixed: width/height/keys/pane generic; fixture fields domain |

Related automation: `tests/showcase_screenshots.rs` and `tests/tui_columns_doc.rs`
re-run `conspectus tui --snapshot` as a subprocess (`assert_cmd`), each with its
own copy of the `conspectus(scratch)` env-cleared command builder and a
`strip_ansi`. `just tui-columns` / `UPDATE_TUI_COLUMNS=1` regenerates
`<!-- frame: keys="..." width=N height=M -->` fenced blocks in
`docs/tui-columns.md` (`tests/tui_columns_doc.rs:105-251`). `tests/tui_tmux_e2e.rs`
(1,455 LOC) is the real-PTY rung: private tmux servers, `send-keys`,
`capture-pane`, convergence waits.

Goldens: 43 `insta::assert_snapshot!` calls in 7 files, 37 `.snap` files
(`src/tui/snapshots` 19, `src/tui/widgets/snapshots` 18). They store
`buffer_to_string` output: **plain characters, styles dropped**
(`ui.rs:376-391`). The ANSI-preserving path is covered only by README
screenshot tests; style regressions rely on ad-hoc cell asserts such as
`consistency_tests/styling.rs` and `help.rs`.

Test-kit evaluation: the three generic components total about 250 LOC and have
unit tests (`snapshot_tests.rs`, 15 tests: 5 key-script parse tests, 1
`buffer_to_ansi`, 5 `pane_rect`, 2 fixture, 2 end-to-end key-prelude drivers).
The change needed to extract:
split `snapshot::run` into `testkit::Harness<M>` where the host supplies
`build() -> M`, `route(&mut M, KeyEvent) -> Applied`, `draw(&mut M, &mut Frame)`,
and `regions(Rect) -> BTreeMap<&str, Rect>`; `Applied::Unsupported(&str)` is
the stderr note. The 10 skipped overlay actions disappear if the framework's
`ModalStack::route` is what both the live loop and the harness call.

### 6.2 Common render helpers (duplicated)

| Helper | Copies |
| --- | --- |
| `render_to_buffer(app, area)` + `buffer_to_string` | `ui.rs:365,380` (cfg(test)), `viewer/widget.rs:728,744`; per-widget `render`/`rendered`/`render_text` in `pane_viewer_tests.rs:34`, `mux_launch_tests.rs:374`, `pin_edit_tests.rs:80`, `viewer_help_tests.rs:8`, `search_content_tests.rs:408`; 13 files call `Buffer::empty` |
| `strip_ansi` | `discovery/tmux/mod.rs:1080`, `tests/showcase_screenshots.rs:64`, `tests/tui_columns_doc.rs:42`, `tests/tui_session_preview.rs:140` (4) |
| `fn key(code)->KeyEvent` / `fn press(..)` | 23 files define one (29 definitions): 13 `widgets/*_tests.rs`, `mux_menu.rs:162` inline, `consistency_tests/{forms,keys,layout}.rs`, `runtime_tests.rs` (3), `overlay_tests.rs`, others |
| Verbose `KeyEvent { code, modifiers, kind, state }` literal | 18 sites in 9 files (8 `widgets/*_tests.rs` + `examples/pantry.rs`) |
| `string_literals` scanner | `consistency_tests/text.rs:215` (handles raw strings, comments, char literals, `#[cfg(test)]` cutoff) and `tests/overlay_keymap_hygiene.rs:58` (naive: no raw strings, no comments) |
| `conspectus(scratch)` subprocess builder | `tests/tui_columns_doc.rs`, `tests/showcase_screenshots.rs` |

### 6.3 Consistency suite, file by file

Root `src/tui/consistency_tests.rs` (140 LOC, 1 test): states six rules (one
gesture one meaning; every change ends the same way; menus depend on the entity;
accelerators act on what you look at; rows look alike; user-visible text speaks
the operator's language), a defect-to-root-cause table, and
`all_ignored_tests_are_owned` (a repo-policy test: an `#[ignore]` must read
`backlog CSP-692.0N: ...`; zero tests are ignored today). All files use the shared
world in `fixtures.rs` (296 LOC): `world()`/`world_with(extra)` (live, gone,
planned, pinned, unpinned entities), `app_over`, `buffer_of`, `frame`,
`select`, `enter`, `menu_labels`.

| File | Tests | LOC | Invariant asserted | Generic UI rule? | Reusable helper |
| --- | --- | --- | --- | --- | --- |
| `keys.rs` | 6 | 217 | Esc leaves, Enter applies, Space changes per modal; every Modal has a row; both search modals share Tab semantics; footer generated | **Yes** (section 5) | `assert_modal_convention(registry)` |
| `layout.rs` | 7 | 534 | Every Modal variant is built or skipped with a reason (parses `modal.rs` text for `pub enum Modal`); each builder opens the modal it names; **nothing a modal draws escapes its frame** (diff vs bare frame, corner-glyph rect); frame does not move or resize when selection moves; **no word clipped at the modal edge** (compare against a 240-wide render); quick-select title width stable across toggles; Search modal sizing rule at 10 sizes | **Yes** for the first 5; last 2 domain | `assert_popup_contained`, `assert_selection_moves_no_text`, `assert_no_clipped_words`, modal registry |
| `help.rs` | 4 | 158 | The two help pages draw the same frame, headings, key column, footer; scroll behavior matches | **Yes** (two pages of one component look alike) | `assert_same_chrome(a, b, sizes)` over `Buffer` cells |
| `text.rs` | 8 | 376 | No ADR/`CSP-` id or non-user doc path on any screen (12 overlays x 8 views, menus, composed toasts/hints), in `--help` for all clap subcommands, or in any string literal of `src/tui` and `src/cli`; "pin", never "slot"; the detector and scanner are themselves tested | **Mechanism yes, rule set policy** | `forbid_in_ui_text(screens, pred)`, `forbid_in_literals(dir, pred)`, `string_literals(src)` |
| `docs.rs` | 5 | 220 | `docs/tui-columns.md` glyph table = `icons::all_glyphs()` (set equality); state-word table = `rows::words::ALL`; every view and row kind has a heading (exhaustive `match` forces new ones); doc speaks the operator's language; doc linked from 3 entry points | **Mechanism yes** (doc table <-> code set) | `assert_doc_table_matches(doc, heading, set)`, `assert_doc_headings(doc, names)` |
| `rendering.rs` | 13 | 338 | Live and gone rows share one column grid (label start, age end); Pinned keeps columns aligned across kinds; chip look for agent name is the same across 4 views (bg or REVERSED); unfocused pane shows no highlighted row (count REVERSED cells); regrouping resets scroll | **Yes** for grid/chip/focus; rest domain | `assert_columns_aligned(lines, labels)`, `reversed_cells(buf, half)`, `token_is_chip` |
| `state.rs` | 6 | 214 | State is a glyph in single-kind views and a word from a fixed vocabulary in mixed views; same pin marker pair, same age string for one entity in every view; unknown age is a dash; the word "gone" never appears | Pattern yes (shared vocabulary across surfaces); words domain | `assert_vocabulary(lines, allowed, forbidden)` |
| `styling.rs` | 4 | 156 | Not-running entities wear the ghost style (theme color + ITALIC); hollow pin glyph in ghost color; `p` has an immediate inverse on the same key in every view | Pattern yes (style by state, same everywhere); rest domain | `cell_style_at(buf, text)` |
| `forms.rs` | 7 | 207 | Forms keep arrow movement between fields, Tab path completion in cwd, harness options listed, all editable fields listed, command row is read-only (never takes focus, dim italic, indented) | Partly (read-only rows never take focus) | `walk_focus_ring(form, keys)` |
| `menus.rs` | 7 | 157 | Menu depends on entity not view; hides entries with nothing to say; pin/unpin always offered | Rule generic, content domain | `assert_menu_parity(a, b)` |
| `focus.rs` | 4 | 85 | Accelerators act on the right pane's entity when it has focus | Rule generic, subject domain | `assert_subject_follows_focus` |
| `parity.rs` | 9 | 209 | Enter / `v` / `p` do the same thing to the same entity in every view; `p` equals the menu's Pin entry | Rule generic ("one gesture, one meaning"), probes domain | `for_each_view_and_row(app, f)` |
| `lifecycle.rs` | 11 | 254 | Every key-bound message runs on an executor that can run what it emits; every store write refreshes; attach freezes row; rebuild follows entity; refresh requests are taken once | L3/domain | none (pattern only) |
| `freshness.rs` | 4 | 121 | Every refreshing effect forces a live rescan; never asks forges; runtime honors the request | L3/domain | none |
| `undo.rs` | 8 | 273 | Bounded stack; newest-first; undo key bound once; every store op classified; round trip of `p` + undo returns cursor in 5 views | Pattern (undo key unique, irreversible ops give reasons) | `assert_key_bound_once(table, key)` |
| `actions.rs` | 30 | 516 | Action subject per row kind, Enter effect per entity kind, focus follows entity | Domain | none |
| `content_filter.rs` | 14 | 388 | Content search narrows lists the same way in every view and composes with filters | Domain | none |
| `content_interactions.rs` | 10 | 350 | Content search survives pins, filters, refresh, rescan, resize | Domain | none |
| `search_flat.rs` | 15 | 426 | Flat message listing semantics | Domain | none |
| `search_quick.rs` | 18 | 483 | Quick select reach/outcome words | Domain | none |
| `search_view.rs` | 13 | 316 | Search view lists and acts on sessions | Domain | none |
| `fixtures.rs` | 0 | 296 | Shared world and drivers | Domain; pattern `app_over(view, snapshot)` generic | `Scenario` builder |

Counts (204 tests): directly liftable generic mechanisms, about 30 (`keys` 6,
`layout` 5, `help` 4, `text` 8, `docs` 5, plus the grid/chip/focus checks in
`rendering`); Conspectus-world instances of generic rules whose *shape* becomes a
helper, about 55 (`menus` 7, `parity` 9, `state` 6, `styling` 4, `forms` 7,
`focus` 4, `undo` 8, the rest of `rendering` 13); pure domain behavior, about
115 (`actions` 30, `content_filter` 14, `content_interactions` 10,
`search_flat` 15, `search_quick` 18, `search_view` 13, `lifecycle` 11,
`freshness` 4). These stay as Conspectus tests.

Gaps in the suite relative to the conventions it states:

1. **No behavioral "Esc closes every modal" test**: `keys.rs` checks the table
   returns a leave-named action, not that the real handler pops the stack
   (`layout.rs::press` only moves selection).
2. No test that **every `Modal` is key-routed** in `overlay_key_from_event` (see
   2.2).
3. The registry of modals exists **four times**: `layout.rs::builders()` (16
   entries), `text.rs::screens()` (about 12 overlay entries, inline,
   `text.rs:98-137`), `keys.rs::tables()` (17 names), and `examples/pantry.rs`
   (6 groups). A single `ModalCase { name, view, open }` registry would drive
   layout, text-leak, convention, and gallery.
4. Tests that need a binary subprocess (`tui_columns_doc`) are
   `cfg(all(feature = "snapshot", debug_assertions))` gated.
5. `Ctrl-C first` and shadowed-binding checks are missing (1.4).

### 6.4 Pantry preview harness (`examples/pantry.rs`, 1,030 LOC; CSP-424 / CSP-437)

`tui_pantry::run!(ingredients)` over `Vec<Box<dyn Ingredient>>`; `Ingredient`
has `group`, `name`, `source`, `description`, `render(&self, area, buf)` and a
`Send` bound. Because `ratatui_cheese`'s multi-select validator is `!Send`, every
variant holds plain config and rebuilds state inside `render` (the
"config on the ingredient, build state per render" shim; CSP-437 says it
generalized).

| Struct | Line | Instances in `main` | What it varies |
| --- | --- | --- | --- |
| `MultiSelectVariant` | 52 | 3 | items, initial selection, cursor advance (via `Down` presses) |
| `ControlsVariant` | 133 | 4 | view/grouping/filter/sort, sub-editor state reached by driving keys (`ControlsOpenState`) |
| `ValueModalVariant` | 224 | 4 | label, value, scroll |
| `HelpVariant` | 260 | 3 | scroll |
| `SearchVariant` | 304 | 4 | item corpus, typed query, cursor advance |
| `TextInputVariant` | 377 | 3 | title, initial, cursor-left count |
| `ThemeHarnessIngredient` | 485 | 1 | renders every `[tui.theme]` key as a labeled sample in 10 sections (`theme_sections`, :741) |

22 ingredients across 7 groups today. The task record (CSP-437) says 28
ingredients across 8 groups including 6 Pins sub-modals; those were removed with
the old pin model. **Coverage**: 5 of 16 `Modal` variants (Controls,
ValueModal, Help, Search quick mode only, Rename via TextInput) plus MultiSelect
and the theme; no gallery entry for Messages, Viewer, PaneViewer,
WorktreeMenu, NewMux, MuxMenu, MuxLaunch, PinEdit, RowMenu, CleanUp, Picker,
badge/chip, toast, path omnibox, forms.

Observations: every variant struct re-implements the same four metadata
methods (`group`, `name`, `source`, `description`; roughly 15-20 LOC of pure
boilerplate each, 6 times) around a `render`; key-driving uses verbose
`KeyEvent { code, modifiers, kind, state }` literals five times in the file
(`:67-72,145-150,315-320,327-332,409-414`); `theme_sections` hard-codes theme
fields by name and still lists `harness.aider` (the aider removal workstream
`m-24` is filed); `search_mock_items` fakes `RowId::Synthetic` keys so it needs no
graph; there is no `pantry.toml` (the migration to the proc-macro convention was
deferred); the harness is a dev-dependency (`Cargo.toml:151-155`).

**Verdict**: extract the *pattern*: a generic `Gallery` helper with
`Case<S> { group, name, description, make_state: fn() -> S, render: fn(&S, Rect, &mut Buffer), keys: &str }` where `keys` uses the same key DSL
as the snapshot harness (it would replace the hand-driven `cursor_advance` /
`ControlsOpenState`); move the theme harness into the framework (it is
inherently about L1 tokens); share the modal registry with the consistency
suite. Conspectus's six variant structs stay as fixtures.

### 6.5 Proposed `testkit` surface

```
testkit::keys      parse("j<Tab>jjj") -> Vec<KeyEvent>;  press/ctrl/alt/shift ctors;  KeySpec
testkit::render    render_widget(w, (w,h)) -> Buffer;  to_text;  to_ansi(region);  strip_ansi;  normalize_frame
testkit::golden    assert_frame!(name, buf, Plain|Ansi);   doc_frames(path, renderer) (the <!-- frame --> sync)
testkit::invariant popup_contained;  selection_moves_no_text;  no_clipped_words;  same_chrome;  columns_aligned;  focus_dims_unfocused
testkit::keytables registry + mislabeled;  convention;  ctrl_c_first;  no_shadowed_bindings;  esc_closes_all (behavioral)
testkit::lint      string_literals;  forbid_in_literals;  forbid_in_text;  doc_table_matches;  no_keycode_in(dir)
testkit::registry  ModalCase { name, setup }  shared by layout / text / convention / gallery
testkit::harness   Harness<M>{ build, route, draw, regions }  -> snapshot CLI + tests + tmux-free scripting
```

---

## 7. What Changed Since The Architecture Documents

### 7.1 `docs/tui-architecture-review.md` (2026-07-01) findings F1-F7

| Finding | Status now | Evidence |
| --- | --- | --- |
| F1 effects interleaved with input | **Mostly fixed** (CSP-496). Reducer returns `Vec<Effect>`; pure and live executors. Residue: overlay handlers still thread `(terminal, app, config, tmux)` only to reach `dispatch_live` | `effect.rs`, `runtime.rs:1975-2090`, `overlay_keys.rs` |
| F2 projection changes re-fetch the world | **Fixed** (CSP-495) | task Done; out of this slice |
| F3 `RunConfig` as live UI state | **Fixed** (CSP-495) | out of this slice |
| F4 implicit modality | **Partly fixed**: `Vec<Modal>` exists, but routing and drawing are still per-variant ladders; stack draw order not implemented | 2.2 |
| F5 renderer mutates state | **Partly**: `Cell` scroll fields are gone, but `draw(&mut App)` still `update`s `Msg::LeftViewportChanged`/`ExplorerViewportChanged` mid-frame (`ui.rs:70-82`, `left_panel.rs:130`, `right_panel.rs:138`); ADR 0085 wanted strict `&App -> pixels` | CSP-499 Done |
| F6 loop duplication | **Fixed**: `LoopMode` + `run_loop` (`runtime.rs:81,124`); snapshot mode is a third driver but shares `action_for_event` and `draw_screen` | CSP-498 Done |
| F7 overlays as proto-components | **Half fixed**: `Overlay::handle` unifies input; `render`, centered-rect math, and runtime handlers are still per overlay (`popup_frame` shares frames) | 2.1 |
| Review R7 "declarative keymap on top" | Overlay half done (ADR 0123); global half partial (1.1) | |

### 7.2 ADR 0085 contracts

1. One Msg/update/App: `Action` layer persists (40 variants).
2. Effects as data: done; extra `Persist` and `Quit` variants as foreseen.
3. Modal stack + one Overlay contract: handle-only trait, GAT `Ctx`, extra
   `CommitAndStay`; draw/routing still per-variant (2.2).
4. Derived view-models: done.
5. One loop, strict `draw`: loop done; draw not strictly pure (F5).

### 7.3 ADR 0024

`src/tui/cmd.rs` and `src/tui/data.rs` do not exist now (`Cmd` became
`Effect` in `effect.rs`; the discovery adapter lives in `runtime.rs`); no
`run_with_events` seam exists (the testing seams are `App::update` plus
`TestBackend` plus the `LoopMode` trait); `crossterm` is consumed through
`ratatui::crossterm`; the TUI dependency set has grown beyond the ADR's two
crates (`tui-popup`, `tui-input`, `ratatui-cheese`, `ratatui-comfy-toaster`,
`ansi-to-tui`, `tui-markdown`, dev: `tui-pantry`, `insta`); whether each has its
own ADR was not audited here. ADR 0024's rule "any retained-mode TUI framework
requires an ADR" does not apply to an internal widget crate, but the plan
README already schedules ADRs for the workspace conversion and the framework
boundary, and ADR 0024's dependency policy is the template for the crate's own
allowed-dependency list.

### 7.4 ADR 0067

Stands, with additions: fixtures (ADR 0068), interactive fixture mode (0069),
fixture-root path rebasing and `--snapshot-now` (0113). The "side-effect-free
subset" has not grown to cover pure overlays.

---

## 8. Dependency And Coupling Summary

| Item | `crate::` imports (non-test) | External crates |
| --- | --- | --- |
| `KeyMatcher` + `key_label` (`keybindings.rs:56-103,560-605`) | none | `ratatui::crossterm` |
| `overlay.rs` generic part (:1-183, 706-830) | `super::KeyMatcher`, `super::key_label` | `ratatui::crossterm` |
| `overlay.rs` concrete tables | 3 (`PaneViewerMsg`, `ViewerFindMsg`, `ViewerMsg`) | same |
| `keybindings.rs` global table | `View`, `Action`, `Msg` | same |
| `keymap.rs` | `App`, `Focus`, `Msg`, `View`, `widgets::controls::VIEW_OPTIONS`, `cursor::wrap_step`, `rows::*`, `model::NodeId`, `actions::*` | same |
| `modal.rs` | `Msg`, 14 widget state types | same |
| `clipboard.rs` | none | `std` only |
| `messages.rs` | `model::MuxSessionId`, `discovery::current_epoch` | `std` |
| `undo.rs` | `model::{NodeId, PinSlot}`, `effect::StoreOp`, `rows::RowId` | `std` |
| `effect.rs` | `model`, `actions::AttachTarget`, `resume`, `widgets::mux_launch`, `viewer`, `content_search` | none |
| `toast.rs` | none | `ratatui-comfy-toaster` |
| `snapshot.rs` | `App`, `Msg`, `RunConfig`, `runtime::*` (9 fns), `model::GraphSnapshot`, `viewer_bridge` | `ratatui`, `anyhow`, `serde_json` |
| `help.rs` | `Theme`, `icons`, `keybindings`, `viewer::render::word_wrap` | `ratatui-cheese::help::Binding`, `tui-popup` |
| `consistency_tests/*` | the whole TUI + `cli::Cli` (text.rs) | `clap`, `tempfile` |
| `examples/pantry.rs` | `conspectus::tui::widgets::*`, `filter`, `Theme` | `tui-pantry` |

---

## 9. Duplications and inconsistencies

### 9.1 Overlay plumbing

1. **Per-variant accessor quartets**: 16 `Modal` variants x (peek, peek_mut,
   open, close) in `app/overlays.rs` (71 `pub fn`). Identical bodies differing
   in type name.
2. **Outcome handlers**: 16 fns in `runtime/overlay_keys.rs`, 14 sharing one
   skeleton; two dispatch flavors; 6 write `"<x>: cancelled"`, 8 do not.
3. **`Action` forwarders**: 16 of 40 `Action` variants only wrap a `KeyEvent`
   (`keymap.rs`), each needing arms in `LiveMode::dispatch`,
   `StaticMode::dispatch` (9 answered with the "scenario TUI keeps mutating
   actions disabled" hint, `runtime.rs:785-804`), and `snapshot::apply_action`
   (10 unhandled, `snapshot.rs:316-321`).
4. **Key routing ladder**: 16 `if app.x().is_some()` branches
   (`runtime.rs:883-937`); tested for 2 modals.
5. **Draw ladder**: 15 `draw_*` fns (`ui.rs:107-274`), 10 identical bodies.
6. **Three Action dispatchers**: `LiveMode::dispatch` (`runtime.rs:467`),
   `StaticMode::dispatch` (:665), `snapshot::apply_action`
   (`snapshot.rs:256`), covering different subsets. In `StaticMode` an opened
   row menu / mux menu / forms route to `*Key` actions that the static
   dispatcher answers with a status hint only *(from reading, not run)*, so the
   `tui --fixture` interactive dev mode appears unable to dismiss a menu it
   can open with `.`; flagged for the operator (open question 6).
7. **Near-copy overlays**: `MuxMenuState` vs `RowMenuState` (same table,
   different constants); `NewMux`/`MuxLaunch`/`PinEdit` `handle` bodies (Abort|
   Cancel->Close, Up/Down/Back -> focus, Tab -> path complete else advance,
   Submit -> `try_commit`, Option keys -> `Some(delta)`), `new_mux.rs:122-164`,
   `mux_launch.rs:358-420`, `pin_edit.rs:564-615`.
8. **Two help-section generators** (`viewer_help.rs:39`, `pane_viewer.rs:563`)
   and a third, hand-written, source (`help.rs:307`).

### 9.2 Hand-kept registries (must be one)

| Registry | File | Entries |
| --- | --- | --- |
| Modals to build | `layout.rs:73` `builders()` | 16 (+ 2 skipped with reasons) |
| Modal names exhaustive | `layout.rs:34` `name_of` | 16 |
| Modals to screenshot for leak scan | `text.rs:98-137` | about 12 |
| Modal -> key tables | `keys.rs:26` `tables()` | 17 names |
| Modal conventions | `overlay.rs:782` | 17 rows |
| Tables to label-check | `overlay_tests.rs:138` | 15 of 20 tables |
| Gallery groups | `pantry.rs:528` | 7 |

### 9.3 Key hints and tables vs docs

- 4 hand-written overlay status hints outside the hygiene guard
  (`runtime.rs:1514,1548,1587,1789`), two of which drift from the table
  (1.3). Header hints (`ui/header.rs:481-660`) are hand-written, ~10 strings.
- `KeyBinding.help_text` unused in production (58 strings) while 48 prose rows
  are hand-maintained in `help.rs`; 7 keys outside the table.
- `MULTI_SELECT_KEYS` and `PICKER_KEYS` footers are generated or empty but never
  rendered; picker has no key hint at all.
- Key-label dialects: 4 (1.6); `key_label` lower-case `Ctrl-c` vs displayed
  `Ctrl-C`.
- `Msg::Report` hard-codes `" · ! details"` (`app.rs:1544`) instead of deriving
  the key from the binding for `OpenMessages`.

### 9.4 Doc vs code drift in this slice

`modal.rs` header and ADR 0085 contract 3 (stack draw order, `render` method,
zero new branches); `keybindings.rs:1-17,105-116` ("wave 2 folds ... wave 4
rewires keymap_sections"; the mode column) and CSP-468's "2 arms remain";
ADR 0123 consequence "its status hint ... follow from that row" (4 do not);
ADR 0067 "no fixture mode in V1" (fixtures arrived with ADR 0068/0113);
`snapshot.rs` `pane_rect` doc "constants pulled in via `pub(super)`" (they are
re-declared).

### 9.5 Test-support duplication

23 files x local `key()/press()` helpers; 18 verbose `KeyEvent{..}` literals;
4 `strip_ansi`; 2 `render_to_buffer`/`buffer_to_string` pairs; 2
`string_literals` scanners; 2 `conspectus(scratch)` builders; 13 files with
their own `Buffer::empty` render dance.

---

## 10. Open questions for the operator

1. **Stack ownership and draw order.** Should the framework `ModalStack` (a)
   draw every entry bottom-to-top (what ADR 0085 promised; needs each popup to
   `Clear` its own area and a convention for dimming what is underneath), or
   (b) keep today's actual behavior (only the top draws)? And does it live
   outside the model (passes `&Cx` while holding `&mut` entries) or use
   take-handle-put? This determines whether Controls/Search keep a 11-field
   owned context snapshot.
2. **Global keymap scope.** Do we design a scoped keymap in the first cut
   (modes = global / pane focus / overlay; rows carry help text that actually
   renders), or ship only the overlay-table model now and leave the global
   table and `remap_for_focus` in Conspectus until a later story? The current
   global table has one mode and unused help text.
3. **Key-semantics unification (consistency-first).** Pick one rule for: chars
   in overlays (`AnyMod` vs `AnyModExceptCtrl` vs `UpperChar`; today Alt-G
   jumps to the bottom of Help but not of the viewer, and Ctrl-G jumps to the
   top of every pager); Ctrl-N/P everywhere or
   only in search; arrows shown or aliased in footers; the footer verb grammar
   (`close`/`to close`/`= merge & close`); `Enter` in forms (advance-then-commit
   vs always submit).
4. **Close feedback.** Six overlays say `"<x>: cancelled"` in the status bar on
   Esc, eight stay silent. Which is the convention?
5. **Generated main help.** Is it acceptable that the framework generates the
   `?` page from key tables plus per-action descriptions (as the two viewer
   helps already do), giving up the 48 long hand-written prose rows in
   `help.rs`? Or should prose rows stay as an override?
6. **`tui --fixture` interactive mode.** From reading, `StaticMode::dispatch`
   answers every `*Key` forwarder for menus and forms with a status hint, so a
   menu opened with `.` may not be dismissable. Is the dev mode used enough that
   the extraction should treat this as a bug to fix, or is it dead?
7. **Where does the consistency suite live.** Generic invariants move to the
   framework's `testkit`, but the Conspectus world and domain tests stay.
   Should framework invariants also be exposed as a one-call
   `assert_conventions(&Registry)` that fails the *host's* test run, so a new
   modal cannot ship without a convention row, a table, a layout case, and a
   gallery entry?
8. **Clipboard.** OSC 52 writes to process stdout. Should the framework take a
   `Write` sink (host decides, tests capture) as the only API, dropping the
   `copy_to_clipboard(&str)` convenience?
9. **`Success` toast color.** Toast border ignores `[tui.theme] success`.
   Fix by forking/wrapping the engine in the framework, or accept the
   documented limitation?
10. **Gallery coverage policy.** Pantry covers 5 of 16 modals. Is "every
    framework control has a gallery case" a hard rule enforced by the registry
    test (open question 7), or best effort?

## Verification (CSP-716)

Open question 6 and item 6 of 9.1 (D12 in the inventory's suspected
defects) were run before the extraction. The defect reproduced, worse than
suspected, and is fixed; interactive fixture mode was kept because it is
used.

- **Is it used?** Yes. `just demo` runs `tui --fixture
  tests/fixtures/showcase.json`; the README and `docs/dev-scenarios.md`
  document it, ADRs 0069 and 0070 introduced it, and `dev scenario tui`
  runs the same `StaticMode` loop. Removing it was not an option.
- **Reproduction.** A pseudo-terminal run of `conspectus tui --fixture
  tests/fixtures/showcase.json` (tmux and the state directories isolated):
  `j` then `.` opened the `session showcase` row menu; Esc drew "scenario
  TUI keeps mutating actions disabled" and left it open; Ctrl-C and `q` did
  nothing, because an open overlay owns every key, so the process had to be
  killed. Any overlay reached another way was a trap too, since
  `Action::Msg` reaches the reducer in static mode: the launch form `N`
  opens, the pin menu `p` opens on an entity several pins point at, and
  whatever a menu entry opens.
- **Fix.** The nine menu, prompt, and form handlers in
  `runtime/overlay_keys.rs` take a `send` callback for what the overlay
  commits instead of the live loop's terminal, mux, and config.
  `LiveMode` passes the live executor and `StaticMode` the pure one, so the
  overlays move, edit, and close the same way in both modes, and a pick
  that needs a terminal, a mux, or a store is refused with the pure
  executor's toast ("that isn't available in this mode"), as `p` already
  was. The direct openers that static mode refused (`R`, `w`, `X`, `n`,
  `m`) stay refused with the old hint. The same run after the fix: `.`,
  `j` moved to `Show rebuild plan`, Esc closed the menu, `.` and Ctrl-C
  closed it again, a second Ctrl-C quit with status 0.
- **Tests.** `src/tui/consistency_tests/fixture_mode.rs` drives keys
  through `press_in_fixture_mode` (the loop's `action_for_event`, then the
  static dispatch, with no terminal): every modal the layout suite builds
  closes on Esc and on Ctrl-C without quitting, `.` then `j` and Esc move
  and close the row menu, and a Pin pick and `p` are refused out loud. The
  viewer and pane viewer, which the layout suite cannot build, already took
  their keys in static mode.
- **Left as is.** `snapshot::apply_action` still skips menu picks and the
  form and picker keys; CSP-732 extends the snapshot driver to every
  overlay key. With the handlers taking `send`, that driver can pass its
  own. The three dispatchers remain three.

## Verification (CSP-759)

The status bar's hints are generated from key tables (ADR 0191). Checked
with `conspectus tui --snapshot` on the showcase fixture: the status bar
in every view, both focus states, on each row kind whose hints differ,
and with each reachable overlay open, at 160, 100, 60, and 40 columns.

- **The four hand-written status hints (1.3, 9.3).** Gone. The worktree
  and mux menus, Controls, and both searches no longer post a hint when
  they open; the status bar names the top overlay's keys from its own
  table (`Overlay::hints`). `tests/overlay_keymap_hygiene.rs` now scans
  `runtime.rs`, `runtime/executor.rs`, `app/overlays.rs`, and
  `ui/header.rs` for any hand-written hint, by a stricter rule.
- **The main view's hints (1.3 item 3).** Generated from
  `MAIN_VIEW_KEYS`, a key table of the main view's keys, with the
  contextual wording as annotations on its rows; a consistency test
  presses every key the bar names, in every view, row, and focus, through
  the loops' routing. They are not yet the `KEYBINDINGS` rows themselves:
  the scoped keymap (CSP-760) merges the two.
- **`Msg::Report`'s ` · ! details` (9.3).** The `!` comes from its row of
  `MAIN_VIEW_KEYS`, as the unseen chip's does.
