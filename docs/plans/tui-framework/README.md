# TUI Framework Extraction: Plan

Status: planning complete on branch `worktree-tui-framework`, uncommitted.
The operator confirmed the control set and house rules; ADRs 0150 to 0152
record the decisions; milestone m-25 holds 57 stories (CSP-710 to CSP-766),
listed at the end of this file. Implementation has not started.

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
renumber on merge.

Execution note: CSP-721 originally waited on CSP-700 (aider removal, m-24). That
dependency was dropped when execution began, so this milestone does not remove
an adapter the operator's separate workstream may be removing. The harness
palette adapter keeps aider's entries until m-24 lands and deletes them then.

### Cleanups and suspected defects (first)

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-710 | Delete the superseded sectioned detail model | none |
| CSP-711 | Retire the hidden Union and Forks views | none |
| CSP-712 | Reproduce and fix the suspected scroll and paging defects | none |
| CSP-713 | Reproduce and fix Enter-to-copy and `o` on expanded node fields | none |
| CSP-714 | Reproduce and fix the left-pane alignment, draft-PR, and selected-chip defects | none |
| CSP-715 | Reproduce and fix the suspected text-entry defects | none |
| CSP-716 | Reproduce and fix the suspected chrome and fixture-mode defects | none |

### Workspace conversion and guardrails

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-717 | Convert the repository to a Cargo workspace with an empty `tuikit` crate | none |
| CSP-718 | Guard the framework's independence from Conspectus | CSP-717 |
| CSP-719 | Extend the source-scanning tests to the framework crate | CSP-717 |
| CSP-720 | Stand up the framework documentation skeleton and contribution rules | CSP-717 |

### Foundations

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-721 | Move design tokens to a generic `theme` module with an app-registered style map | CSP-718, CSP-720 |
| CSP-722 | Move the vetted glyph table to `glyphs` and retire duplicate glyph literals | CSP-718, CSP-720 |
| CSP-723 | Merge the truncation, padding, wrapping, and snippet helpers into one `text` module | CSP-718, CSP-720 |
| CSP-724 | Extract `scroll`: cursor stepping, follow offset, and the scrollbar gutter | CSP-718, CSP-720, CSP-712 |
| CSP-725 | Extract `chip` and `age` | CSP-721, CSP-723, CSP-722 |
| CSP-726 | Extract `ModalFrame` and unify the popup sizing rules | CSP-721, CSP-723 |
| CSP-727 | Extract `selection`: one list cursor, highlight, and wrap-or-clamp rule | CSP-721, CSP-722, CSP-724 |

### Interaction conventions (layer 2)

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-728 | Extract the key-table model to `keys` | CSP-718, CSP-720 |
| CSP-729 | Extract the `Overlay` contract and a host-owned `ModalStack` | CSP-728, CSP-726 |
| CSP-730 | Extract `HelpPage` and generate help from key tables | CSP-728, CSP-723, CSP-724, CSP-726 |
| CSP-731 | Extract `feedback`: severity, `Toasts`, the message log, and the clipboard sink | CSP-721, CSP-722, CSP-726 |
| CSP-732 | Extract the `testkit`: key DSL, frame driver, and convention conformance harness | CSP-719, CSP-720 |

### Text entry

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-733 | Extract `LineEdit` and `TextField` | CSP-723, CSP-721, CSP-728, CSP-715 |
| CSP-734 | Extract `TextPrompt<M>` | CSP-733, CSP-729, CSP-726 |
| CSP-735 | Extract `PathField` | CSP-733, CSP-715 |
| CSP-736 | Extract the `Form<M>` kit and collapse the new mux form onto it | CSP-734, CSP-735, CSP-729, CSP-727 |
| CSP-737 | Collapse the mux launch form onto `Form<M>` and retire `LaunchSpecFormState` | CSP-736 |
| CSP-738 | Collapse the pin edit form onto `Form<M>` with an override row | CSP-736 |

### Choice and command

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-739 | Extract `ActionMenu<M>` | CSP-729, CSP-727 |
| CSP-740 | Extract `Dialog<M>` and rebuild the worktree menu on menu-to-dialog flows | CSP-739, CSP-734 |
| CSP-741 | Extract `Checklist` and drop the `ratatui-cheese` dependency | CSP-729, CSP-727, CSP-730 |
| CSP-742 | Extract `Palette<R>` and move the Picker onto it | CSP-733, CSP-729, CSP-727, CSP-723 |
| CSP-743 | Move quick select and content search onto `Palette` | CSP-742, CSP-723 |
| CSP-744 | Extract `SettingsPanel<M>` from the Controls overlay | CSP-741, CSP-734, CSP-739 |

### Reading surfaces

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-745 | Extract `Pager` with `Find` and port the pane scrollback viewer onto it | CSP-733, CSP-730, CSP-724, CSP-723, CSP-729, CSP-725 |
| CSP-746 | Move the value modal and the help page body onto `Pager` | CSP-745, CSP-712 |
| CSP-747 | Extract `LogList` and move the Messages overlay onto it | CSP-745, CSP-731 |
| CSP-748 | Move the transcript viewer onto `Pager` and move its generic helpers out | CSP-745 |
| CSP-749 | Extract `richtext` and the preview-pane scroll region | CSP-745, CSP-723 |

### Data display

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-750 | Extract `TreeState<Id>` and `TreeView<R>` with app-rendered lines | CSP-724, CSP-727, CSP-725, CSP-723, CSP-714 |
| CSP-751 | Add `ColumnSchema` and migrate the Sessions and Mux views | CSP-750, CSP-723, CSP-725, CSP-721, CSP-711 |
| CSP-752 | Migrate the PRs, Pinned, and History views to `ColumnSchema` | CSP-751 |
| CSP-753 | Migrate the Search view and hit rows to `ColumnSchema` | CSP-752, CSP-743 |
| CSP-754 | Extract `PropertyGrid`: property, link, and disclosure rows | CSP-723, CSP-727, CSP-713, CSP-746, CSP-710 |
| CSP-755 | Design the generic `Explorer`: decouple the zoned cursor and drill stack from `App` | CSP-754, CSP-750 |
| CSP-789 | Keep the right pane's drill across a refresh (found by CSP-755) | none |
| CSP-790 | Extract `Breadcrumb` and one rule for pieces of a line that give way (split from CSP-756) | CSP-755 |
| CSP-791 | Extract `StackedMasterDetail` and the section rule from the right pane's layout (split from CSP-756) | CSP-755 |
| CSP-756 | Extract `Explorer`, `Breadcrumb`, and `StackedMasterDetail` per the design | CSP-755, CSP-729, CSP-789, CSP-790, CSP-791 |

### Chrome

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-757 | Extract `AppFrame`, `SplitPane`, and `PaneFrame` with the focus cue | CSP-726, CSP-721, CSP-729 |
| CSP-758 | Extract `TabBar`, `PriorityLine`, and `EmptyState` | CSP-723, CSP-725, CSP-757, CSP-716, CSP-790 |
| CSP-759 | Extract `StatusBar` and generate its hints from the key tables | CSP-728, CSP-730, CSP-723, CSP-731 |
| CSP-760 | Design and extract the scoped global keymap and generate the main help from it | CSP-759, CSP-730, CSP-710 |

### Separation readiness

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-761 | Complete the framework documentation | CSP-738, CSP-737, CSP-740, CSP-743, CSP-744, CSP-748, CSP-747, CSP-749, CSP-753, CSP-756, CSP-758, CSP-760, CSP-732, CSP-720 |
| CSP-762 | Create the framework's own decision log | CSP-761 |
| CSP-763 | Rehearse separating the framework into its own repository | CSP-761, CSP-718 |
| CSP-764 | Update Conspectus docs and agent instructions for the framework | CSP-761 |

### Close-out

| Task | Story | Depends on |
| --- | --- | --- |
| CSP-765 | Optional stretch: evaluate extracting the application runtime (layer 3) | CSP-729, CSP-731, CSP-732, CSP-757 |
| CSP-766 | Assess the milestone's impact on the existing backlog | CSP-764, CSP-762, CSP-763 |
