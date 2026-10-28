# ADR 0151: The Framework Control Set And Its House Rules

## Status

Accepted. Extends [ADR 0150](0150-tui-framework-workspace-crate.md). When the
stories named below land they narrow ADR 0030 (text-input primitive, becomes
`LineEdit`), ADR 0031 (the Controls overlay becomes `SettingsPanel`), ADR 0085
(the overlay contract gains `render` and the stack draws every entry), ADR 0097
(the launch-spec form primitive dissolves into `Form`), and ADR 0123 (overlay
key tables move into the framework and generate the status-bar hints too).
The index in `docs/adr/README.md` records each amendment as its story lands.

## Context

The inventory (`docs/plans/tui-framework/inventory.md`) counted about 60
surfaces and shared primitives behind the TUI. Many are one control wearing
different clothes: 18 menu, dialog, prompt, list, and toast surfaces, 11
text-entry surfaces, 7 scrolling readers, and 8 tree views. They also disagree
on rules a user feels: four list-selection looks, four confirmation idioms,
three Enter and Tab behaviors in forms, three error displays plus four silent
rejections, and six overlays that say "cancelled" where eight stay silent.

Extracting the controls without settling those rules would copy the
disagreements into a library. The operator asked for consistency to win where
unifying changes visible behavior.

## Decision

### The control set

**Foundations:** `theme`, `glyphs`, `text`, `scroll`, `chip`, `age`, `frame`
(modal frame, pane frame, split pane, rule), `selection`.

**Interactive controls (16):**
- Text entry: `LineEdit` and `TextField`; `TextPrompt<M>`; `PathField`; `Form<M>`.
- Choice and command: `ActionMenu<M>`; `Dialog<M>`; `Checklist`; `Palette<R>`
  (the filter picker, quick select, content search, and a later command
  palette are instances); `SettingsPanel<M>`.
- Reading: `Pager` with `Find`; `HelpPage`; `LogList`; `richtext` (a cargo
  feature).
- Data display: `TreeView<R>` with `TreeState<Id>` and `ColumnSchema`;
  `PropertyGrid`; `Explorer`.
- Feedback: `Toasts`.

**Chrome (5):** `AppFrame`, `TabBar`, `StatusBar`, `EmptyState`, and the pane
frame with its focus cue.

**Layer-2 conventions:** `keys`, `overlay`, `help`, `focus`, `feedback`
(including the clipboard sink), `testkit`.

`docs/plans/tui-framework/control-set.md` lists what each control absorbs and
what stays in Conspectus as an adapter. The row builders, harness colors,
transcript model, preview sources, graph-querying menus, `Msg`, `Action`,
`App`, effects, and undo stay in Conspectus.

### Staging inside the control set

`TreeView` lands first with app-rendered lines and then migrates each view to a
`ColumnSchema`, one view at a time. The `Explorer` is extracted last, after a
design story, because about 13% of `explorer.rs` is domain-free and its state
machine is written as `impl App`.

### House rules

Every extraction story applies these. Each story records, in its final summary,
every visible change as a **regression** or a **consistency win**.

| # | Topic | Rule |
| --- | --- | --- |
| 1 | List cursor and highlight | Full-row highlight from `selection_active` (`selection_inactive` unfocused) plus the `marks::POINTER` marker in a 2-cell gutter, in every list |
| 2 | List ends | Short choice lists (menus, pickers, checklists, palettes) wrap; scrollable content (tree, properties, pagers, logs) clamps |
| 3 | Confirmation | An inline arm (`Enter` again) on a menu entry when there is nothing more to say; a `Dialog` (`y`/`Enter` confirm, `n`/`Esc` cancel) when a body helps; reversible verbs need neither |
| 4 | Close keys | `Esc` steps back one level; `Ctrl-C` and `q` dismiss the overlay (`q` only where nothing is typed) |
| 5 | Close feedback | Cancelling is silent |
| 6 | Forms | `Enter` submits from any row; `Tab`/`Down` next and `S-Tab`/`Up` previous over one ring |
| 7 | Validation errors | One error line in the modal's footer slot, set by a failed confirm and cleared by the next edit; no silent rejection |
| 8 | Footers and hints | Every modal footer and the status-bar hints are generated from key tables; one grammar, a short imperative verb with no "to" |
| 9 | Focus cue | The focused pane's border and title marker take `panel_focus_accent` |
| 10 | Unfocused cursor | Every list shows the active or inactive selection pair |
| 11 | Feedback severity | `Feedback` has info, success, warning, and error; each channel (status line, toast, log) styles from the theme; errors stay longer |
| 12 | Paging | Viewport-sized `PgUp`/`PgDn`, `Home`/`End`/`g`/`G` on every list taller than a screen, half-page `Ctrl-D`/`Ctrl-U` in pagers, scroll offsets clamped |
| 13 | Theme | Every control takes `&Theme`; no raw `Color::`; the muted tokens collapse to `muted` and `inert`, with the old config keys kept as aliases |
| 14 | Width | All truncation and padding measure terminal cells; `…` ends, middle ellipsis for paths and ids |
| 15 | Text editing | Cell-width-correct editing, bracketed paste, working word motion |
| 16 | Path fields | A directory-or-file kind, `~` expanded on commit, completion behind a source and cached |
| 17 | Labels | One vocabulary: the shorter form (`name`, `socket`, `argv`, `branch`) |
| 18 | Pagers | `Esc` steps back (help, then find, then leave), `q` leaves, confirming a find scrolls to the match, a `[ n/m ]` chip in Screen mode and a scrollbar in Modal and Embedded modes |

### Cleanups that precede extraction

Dead and superseded code goes first so the extractions start from the code that
runs: the legacy sectioned detail model (`detail.rs`, `detail_header.rs`), the
hidden Union and Forks views, and reproduce-then-fix stories for the suspected
defects the inventory found by reading. A defect is fixed before the extraction
that would otherwise paper over it.

## Consequences

- The visible changes are real and deliberate: menus start wrapping, forms
  gain themed colors and an error line, the focused pane gains a colored border,
  the right pane gains an unfocused cursor, and several `cancelled` status
  messages disappear. Each story lists them.
- Operators who set `[tui.theme]` see forms follow it for the first time.
- `docs/tui-columns.md` and its tests change with the `ColumnSchema`
  migrations; the document and the code move together as the project already
  requires.
- The rules become the framework's own conformance checks, enforced by the test
  kit, not by review.
- The gap analysis (`docs/plans/tui-framework/gap-analysis.md`) lists controls
  other frameworks ship that this set does not include. Each affected story
  leaves the matching seam open (for example key rows that can carry
  availability, controls that can report their rendered area for mouse hit
  testing) without building the control.

## Alternatives Considered

- **Behavior-preserving extraction first, unification afterward.** Roughly twice
  the stories; each is a pure refactor, but the disagreements are copied into
  the library and then changed a second time. Rejected by the operator in favor
  of consistency first.
- **A smaller set that stops at `TreeView` with app-rendered lines.** Cheaper,
  but leaves the opinionated columnar look in app code with no consistency win
  across the ten row layouts. Rejected.
- **Keeping the Explorer in Conspectus.** Lower risk, but the drill stack and
  zoned cursor are reusable and the property grid still needs them. Kept in
  scope as the last extraction, with a design story ahead of it.
- **Per-overlay close semantics as today.** Preserves muscle memory but keeps the
  rule unlearnable; rejected for rule 4.

## Open Questions Answered

- *Do menus wrap or clamp?* Short choice lists wrap, scrollable content clamps.
- *Which confirmation idiom?* Two, chosen by whether a body helps; reversible
  verbs need none.
- *Does the focus cue change?* Yes: the border gains the accent, as ADR 0076
  already says it does.
- *One feedback type or three channels?* One type with severity, three
  channels.
- *Is multi-line input in the first cut?* No. It is a gap-analysis item;
  `tui-textarea` 0.7.0 does not resolve against ratatui 0.30 and the maintained
  continuation is `ratatui-textarea`.
