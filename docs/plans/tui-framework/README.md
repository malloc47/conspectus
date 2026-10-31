# TUI Framework Extraction: Plan

Status: executed on branch `worktree-tui-framework` (2026-10-31). The
operator confirmed the control set and house rules, and ADRs 0150 to 0152
record those decisions. Milestone m-25 held 57 planned stories (CSP-710 to
CSP-766); three were split out of CSP-756 (CSP-789 to CSP-791), and 33
follow-ups were filed while the stories ran. Every story is Done; of the
follow-ups CSP-770 (a test flake), CSP-776 (dimming actions that cannot act),
CSP-802 (folding the keymap's `Action` layer into `Msg`), and CSP-803 (the
TUI's color off switch and color depth, filed by the close-out review) are
open; the tables at the end of this file have each one's status. The
close-out's review of every open story is
[`backlog-impact.md`](backlog-impact.md). ADRs 0153 to 0200 record what the stories decided, and the
framework's own log,
`crates/tuikit/docs/decisions/`, restates the framework's part. The
framework's documentation starts at `crates/tuikit/README.md`.

To review the branch, start with [`review-guide.md`](review-guide.md): the
milestone in numbers, the 25 decisions most likely to need the operator's
call, the remaining review rows by theme, the open follow-ups, merge notes,
and how the work was verified.

Two files ran the milestone and stay with it:

- [`execution-brief.md`](execution-brief.md): the contract every story
  followed (the per-story backlog workflow, commits, checks, the tmux-safe
  test runner, disk hygiene, framework independence, and stop conditions).
- [`operator-review.md`](operator-review.md): every judgment call the
  stories made where the operator may want a different answer, with the
  default that shipped and where to change it. Nothing in it blocks the
  milestone.

Conspectus has grown enough TUI components that its look, controls, and
widgets are worth separating into a reusable TUI "design framework" crate,
the way a React component library sits apart from the apps that use it.
This directory holds the planning artifacts for that extraction. It ends in
a backlog milestone (see Deliverables); nothing here changes code.

## Scope Decisions (confirmed with the operator)

| Question | Decision |
| --- | --- |
| How is independence enforced? | A **workspace crate**: the repo becomes a Cargo workspace and the framework is a member crate that conspectus depends on by path. The compiler rejects conspectus-specific references; moving it to its own repo is a directory lift. |
| Which layers are in? | **Layer 1**, components and design tokens (widgets, theme/palette, glyphs, chips, layout helpers), and **layer 2**, interaction conventions (key tables that generate help, the modal/overlay contract and focus rules, the snapshot and cross-control consistency harness). **Layer 3**, the app runtime (MVU loop, effect lifecycle, terminal suspend/resume), is out of the first cut; the milestone carries one optional stretch story to evaluate it near the end. |
| End state? | **Internal reuse only.** Consumed by path (later possibly git) dependency. Documentation still has to stand on its own so the crate can move to a separate repo at any time, but there is no semver promise, release pipeline, or public gallery. |
| Behavior changes while deduplicating? | **Consistency-first.** Unify on the best variant in the story that extracts the control. Every visible delta is cataloged in that story as a *regression* or a *consistency win*. |

Working name: "the framework" / "the kit". The crate name is a decision for
the control-set checkpoint (and a story in the milestone), not baked into
this plan.

## Phases

1. **Scope** — the table above.
2. **Inventory** — every widget and opinionated composite, with its variants
   and customizations: [`inventory.md`](inventory.md), built from the
   per-area notes in [`inventory/`](inventory/).
3. **Deduplicate and confirm** — the smaller base control set and the house
   rules, reviewed with the operator: [`control-set.md`](control-set.md).
   Workspace-conversion findings: [`workspace-impact.md`](workspace-impact.md).
4. **Gap analysis** — controls other frameworks ship that Conspectus does not
   use yet: [`gap-analysis.md`](gap-analysis.md), built from the surveys in
   [`research/`](research/).
5. **Milestone and backlog** — milestone, stories, and the ADR(s) recording
   the decisions.

Design notes written during execution:

- [`explorer-design.md`](explorer-design.md) — the generic `Explorer`
  (CSP-755): the measured code, the `ExplorerSource` seam, the state and
  outcome type, `Breadcrumb` and the stacked layout, the right pane's key
  table, the migration steps, and the go decision (ADR 0188).
- [`runtime-evaluation.md`](runtime-evaluation.md) — extracting the
  application runtime, layer 3 (CSP-765): the runtime's lines classified
  generic or domain by file, a `Runtime<M, Fx>` seam checked against the
  loops and the layer 2 contracts, the stories and risks, the benefit for a
  second program, and the recommendation, later (ADR 0200).
- [`backlog-impact.md`](backlog-impact.md) — the milestone's effect on the
  rest of the backlog (CSP-766): every story open at the close classified
  (unaffected, path or terminology update, re-scoping, newly blocked, or
  superseded) with the action taken, the outcome of each known overlap, each
  Tier 1 and Tier 2 gap mapped to a story or marked not planned, and the
  follow-ups still open.

## Deliverables

- This directory (inventory, control set, gap analysis).
- ADR(s) under `docs/adr/` for the workspace conversion, the framework
  boundary, and the control set.
- A backlog milestone with stories that extract each control, document the
  framework, guard its independence, prepare it to leave the repo, and (last)
  assess the impact on the existing backlog.

## Prior Art In The Repo

- CSP-433: spike on `rat-widget` as a cohesive widget kit — no-go (mandatory
  focus/event/scroll trifecta, all-or-nothing adoption per surface).
- CSP-424 / CSP-437: `tui-pantry` preview harness (`examples/pantry.rs`).
- CSP-497: modal stack and the shared `Overlay` contract (`src/tui/modal.rs`).
- CSP-468: declarative keybinding table feeding dispatch, overlays, and help.
- CSP-435 / CSP-430 / CSP-432: deferred adoptions (`tui-tree-widget`,
  `tui-textarea`, `throbber-widgets-tui`).
- `docs/tui-architecture-review.md`: findings F1–F7 on the TUI's shape.
- `docs/tui-columns.md`: living reference for the left panel's columns.

## Milestone m-25: Stories

Created from this plan. Every task carries the `tui-framework` label plus one
`tuifw-<area>` label, a **TUI changes** section (consistency wins and
regressions), acceptance criteria, and dependencies. `backlog doctor` reports
no cycles. IDs were assigned from `ca668a3`; if `main` files tasks first,
renumber on merge. The Status column reads the board on 2026-10-31; each
Done story's final summary lists its visible changes as regressions and
consistency wins.

Execution note: CSP-721 originally waited on CSP-700 (aider removal, m-24). That
dependency was dropped when execution began, so this milestone does not remove
an adapter the operator's separate workstream may be removing. The harness
palette adapter keeps aider's entries until m-24 lands and deletes them then.

### Cleanups and suspected defects (first)

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-710 | Delete the superseded sectioned detail model | none | Done |
| CSP-711 | Retire the hidden Union and Forks views | none | Done |
| CSP-712 | Reproduce and fix the suspected scroll and paging defects | none | Done |
| CSP-713 | Reproduce and fix Enter-to-copy and `o` on expanded node fields | none | Done |
| CSP-714 | Reproduce and fix the left-pane alignment, draft-PR, and selected-chip defects | none | Done |
| CSP-715 | Reproduce and fix the suspected text-entry defects | none | Done |
| CSP-716 | Reproduce and fix the suspected chrome and fixture-mode defects | none | Done |

### Workspace conversion and guardrails

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-717 | Convert the repository to a Cargo workspace with an empty `tuikit` crate | none | Done |
| CSP-718 | Guard the framework's independence from Conspectus | CSP-717 | Done |
| CSP-719 | Extend the source-scanning tests to the framework crate | CSP-717 | Done |
| CSP-720 | Stand up the framework documentation skeleton and contribution rules | CSP-717 | Done |

### Foundations

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-721 | Move design tokens to a generic `theme` module with an app-registered style map | CSP-718, CSP-720 | Done |
| CSP-722 | Move the vetted glyph table to `glyphs` and retire duplicate glyph literals | CSP-718, CSP-720 | Done |
| CSP-723 | Merge the truncation, padding, wrapping, and snippet helpers into one `text` module | CSP-718, CSP-720 | Done |
| CSP-724 | Extract `scroll`: cursor stepping, follow offset, and the scrollbar gutter | CSP-718, CSP-720, CSP-712 | Done |
| CSP-725 | Extract `chip` and `age` | CSP-721, CSP-723, CSP-722 | Done |
| CSP-726 | Extract `ModalFrame` and unify the popup sizing rules | CSP-721, CSP-723 | Done |
| CSP-727 | Extract `selection`: one list cursor, highlight, and wrap-or-clamp rule | CSP-721, CSP-722, CSP-724 | Done |

### Interaction conventions (layer 2)

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-728 | Extract the key-table model to `keys` | CSP-718, CSP-720 | Done |
| CSP-729 | Extract the `Overlay` contract and a host-owned `ModalStack` | CSP-728, CSP-726 | Done |
| CSP-730 | Extract `HelpPage` and generate help from key tables | CSP-728, CSP-723, CSP-724, CSP-726 | Done |
| CSP-731 | Extract `feedback`: severity, `Toasts`, the message log, and the clipboard sink | CSP-721, CSP-722, CSP-726 | Done |
| CSP-732 | Extract the `testkit`: key DSL, frame driver, and convention conformance harness | CSP-719, CSP-720 | Done |

### Text entry

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-733 | Extract `LineEdit` and `TextField` | CSP-723, CSP-721, CSP-728, CSP-715 | Done |
| CSP-734 | Extract `TextPrompt<M>` | CSP-733, CSP-729, CSP-726 | Done |
| CSP-735 | Extract `PathField` | CSP-733, CSP-715 | Done |
| CSP-736 | Extract the `Form<M>` kit and collapse the new mux form onto it | CSP-734, CSP-735, CSP-729, CSP-727 | Done |
| CSP-737 | Collapse the mux launch form onto `Form<M>` and retire `LaunchSpecFormState` | CSP-736 | Done |
| CSP-738 | Collapse the pin edit form onto `Form<M>` with an override row | CSP-736 | Done |

### Choice and command

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-739 | Extract `ActionMenu<M>` | CSP-729, CSP-727 | Done |
| CSP-740 | Extract `Dialog<M>` and rebuild the worktree menu on menu-to-dialog flows | CSP-739, CSP-734 | Done |
| CSP-741 | Extract `Checklist` and drop the `ratatui-cheese` dependency | CSP-729, CSP-727, CSP-730 | Done |
| CSP-742 | Extract `Palette<R>` and move the Picker onto it | CSP-733, CSP-729, CSP-727, CSP-723 | Done |
| CSP-743 | Move quick select and content search onto `Palette` | CSP-742, CSP-723 | Done |
| CSP-744 | Extract `SettingsPanel<M>` from the Controls overlay | CSP-741, CSP-734, CSP-739 | Done |

### Reading surfaces

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-745 | Extract `Pager` with `Find` and port the pane scrollback viewer onto it | CSP-733, CSP-730, CSP-724, CSP-723, CSP-729, CSP-725 | Done |
| CSP-746 | Move the value modal and the help page body onto `Pager` | CSP-745, CSP-712 | Done |
| CSP-747 | Extract `LogList` and move the Messages overlay onto it | CSP-745, CSP-731 | Done |
| CSP-748 | Move the transcript viewer onto `Pager` and move its generic helpers out | CSP-745 | Done |
| CSP-749 | Extract `richtext` and the preview-pane scroll region | CSP-745, CSP-723 | Done |

### Data display

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-750 | Extract `TreeState<Id>` and `TreeView<R>` with app-rendered lines | CSP-724, CSP-727, CSP-725, CSP-723, CSP-714 | Done |
| CSP-751 | Add `ColumnSchema` and migrate the Sessions and Mux views | CSP-750, CSP-723, CSP-725, CSP-721, CSP-711 | Done |
| CSP-752 | Migrate the PRs, Pinned, and History views to `ColumnSchema` | CSP-751 | Done |
| CSP-753 | Migrate the Search view and hit rows to `ColumnSchema` | CSP-752, CSP-743 | Done |
| CSP-754 | Extract `PropertyGrid`: property, link, and disclosure rows | CSP-723, CSP-727, CSP-713, CSP-746, CSP-710 | Done |
| CSP-755 | Design the generic `Explorer`: decouple the zoned cursor and drill stack from `App` | CSP-754, CSP-750 | Done |
| CSP-789 | Keep the right pane's drill across a refresh (found by CSP-755) | none | Done |
| CSP-790 | Extract `Breadcrumb` and one rule for pieces of a line that give way (split from CSP-756) | CSP-755 | Done |
| CSP-791 | Extract `StackedMasterDetail` and the section rule from the right pane's layout (split from CSP-756) | CSP-755 | Done |
| CSP-756 | Extract `Explorer`, `Breadcrumb`, and `StackedMasterDetail` per the design | CSP-755, CSP-729, CSP-789, CSP-790, CSP-791 | Done |

### Chrome

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-757 | Extract `AppFrame`, `SplitPane`, and `PaneFrame` with the focus cue | CSP-726, CSP-721, CSP-729 | Done |
| CSP-758 | Extract `TabBar`, `PriorityLine`, and `EmptyState` | CSP-723, CSP-725, CSP-757, CSP-716, CSP-790 | Done |
| CSP-759 | Extract `StatusBar` and generate its hints from the key tables | CSP-728, CSP-730, CSP-723, CSP-731 | Done |
| CSP-760 | Design and extract the scoped global keymap and generate the main help from it | CSP-759, CSP-730, CSP-710 | Done |

### Separation readiness

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-761 | Complete the framework documentation | CSP-738, CSP-737, CSP-740, CSP-743, CSP-744, CSP-748, CSP-747, CSP-749, CSP-753, CSP-756, CSP-758, CSP-760, CSP-732, CSP-720 | Done |
| CSP-762 | Create the framework's own decision log | CSP-761 | Done |
| CSP-763 | Rehearse separating the framework into its own repository | CSP-761, CSP-718 | Done |
| CSP-764 | Update Conspectus docs and agent instructions for the framework | CSP-761 | Done |

### Close-out

| Task | Story | Depends on | Status |
| --- | --- | --- | --- |
| CSP-765 | Optional stretch: evaluate extracting the application runtime (layer 3) | CSP-729, CSP-731, CSP-732, CSP-757 | Done |
| CSP-766 | Assess the milestone's impact on the existing backlog | CSP-764, CSP-762, CSP-763 | Done |

### Follow-ups filed during execution

Filed by the story named, or by its review, and run in milestone m-25 unless
the Status says otherwise. CSP-789 to CSP-791 are in the Data display table
above.

| Task | Story | Filed by | Status |
| --- | --- | --- | --- |
| CSP-767 | Extend the working-directory guard to the framework crate | CSP-719 | Done |
| CSP-768 | Group-row attach hint points at a detail pane that lists no muxes | CSP-710 | Done |
| CSP-769 | Decide how quick select and Reveal treat fork entities now that no view lists them | CSP-711 | Done |
| CSP-770 | Stop fake-binary tests from racing on a busy executable | a test flake | To Do (no milestone) |
| CSP-771 | Make `o` open truncated fields in Expanded Node Detail | CSP-712 | Done (no milestone; closed with CSP-713) |
| CSP-772 | Line up the path and count columns of nested group headers with their parents | CSP-714 | Done |
| CSP-773 | Guard the host TUI against raw colors and dim | CSP-721 | Done |
| CSP-774 | Crop CLI search snippets with `tuikit::text::fit_snippet` | CSP-723 | Done |
| CSP-775 | Give the left pane tree the list cursor marker | CSP-727 | Done |
| CSP-776 | Dim overlay actions that cannot act now | CSP-728 | To Do (no milestone) |
| CSP-777 | Compute the launch form's default command off the draw path | CSP-735 | Done (covered by CSP-737) |
| CSP-778 | Check a pin's new name in the Rename prompt before anything is renamed | CSP-738 | Done |
| CSP-779 | Skip the scrollbar gutter where its track cannot draw | CSP-739 | Done |
| CSP-780 | Find a stacked step's frame in the conformance suite when it covers the entry under it exactly | CSP-740 | Done |
| CSP-781 | Give every modal footer the same look | CSP-742 | Done |
| CSP-782 | Page the search palettes with PgUp and PgDn | CSP-743 | Done |
| CSP-783 | Draw an action menu's cursor inactive under its own steps | CSP-744 | Done |
| CSP-784 | Fit a footer too tall for its frame one way in every framed control | CSP-746 | Done |
| CSP-785 | Check the framework with its cargo features off in CI and `just check` | CSP-749 | Done |
| CSP-786 | Keep the left pane's row width when its list overflows | CSP-753 | Done |
| CSP-787 | Lay out the quick-select entity rows from the shared column plan | CSP-753 | Done |
| CSP-788 | Make the right pane's hint for an unresolved row say what is true | CSP-754 | Done |
| CSP-792 | A refresh returns the right pane's Preview to its anchor | CSP-789 | Done |
| CSP-793 | Name a non-bare repo by its directory in the breadcrumb, not `.git` | CSP-790 | Done |
| CSP-794 | Let `SectionRule` give way when its chip and summary do not fit | CSP-791 | Done |
| CSP-795 | Draw the framework controls' empty notes through `EmptyState` | CSP-758 | Done |
| CSP-796 | Name the keys in status and refusal messages from the key tables | CSP-759 | Done |
| CSP-797 | Decide whether the main screen's letters follow the one character rule | CSP-760 | Done |
| CSP-798 | Name the keys in empty-state hints and Preview notes from the keymap | CSP-796 | Done |
| CSP-799 | Check a pin's new name in `conspectus pin rename` before the tmux session is renamed | CSP-778 | Done |
| CSP-800 | Keep the context beside a match when a snippet's text has no space near the cut | CSP-774 | Done |
| CSP-801 | Cut a snippet inside a word only beside a token, not beside ordinary prose | CSP-800 | Done |
| CSP-802 | Fold the keymap's `Action` layer into `Msg` so every loop routes a key one way | CSP-765 | To Do (no milestone) |
| CSP-803 | Draw the TUI without color when color is off, and within the terminal's color depth | CSP-766 | To Do (no milestone) |
