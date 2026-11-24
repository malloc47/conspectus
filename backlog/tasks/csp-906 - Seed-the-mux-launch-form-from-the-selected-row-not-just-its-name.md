---
id: CSP-906
title: 'Seed the mux launch form from the selected row, not just its name'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-24 16:34'
updated_date: '2026-11-24 17:06'
labels:
  - tui
  - launch
milestone: m-23
dependencies: []
priority: medium
type: enhancement
ordinal: 981000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`N` (or `m`, then Launch harness in new mux…) opens the launch form seeded from the selected row, but only the name and, in part, the cwd follow it. The name is the next free one in the row's series (`worker-1` gives `worker-2`); the harness is always the first registered one, the argv is empty, and the pin choice is always `don't pin`, whatever the row is. So launching another mux like the one under the cursor means retyping its harness, its flags (skip-permissions and the like), and whether it is pinned. The operator reported this on 2026-11-24 and wants the form to start as a copy of the selected row, which the name already does.

The pieces exist: the form derives its option checkboxes from the argv it holds, and a row's harness, argv, cwd, and pin are already known to rebuild, pin edit, and `src/resolve/slots.rs`. What is missing is the seed. The cwd is also taken from the row without checking that the directory still exists, which a stopped mux, a removed worktree, or a pin on a deleted checkout can fail.

Out of scope: the new tmux session form (`OpenNewMuxForm`), which has no harness or pin.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The form opens with the harness of the selected row (a session, a mux, a pin, or a stopped or remembered mux), and the first registered harness only when the row has none or its harness is not one the form can pick
- [x] #2 The harness's launch options and any other arguments follow the row's own launch: skip-permissions and the like start checked when the row's argv carries them, and the `command` row shows that argv
- [x] #3 The cwd is the row's while that directory exists; when it does not, the form seeds the cwd it would for a row with none
- [x] #4 The pin choice follows the row: `don't pin` when the row is not pinned, `launch, then pin` when it is pinned and running, `pin, don't launch` when it is pinned and not running
- [x] #5 The same entity seeds the same form whichever view the row is in, and a row that names no harness, cwd, or pin (a group header, a PR) leaves the form with today's defaults; the name seed is unchanged
- [x] #6 Tests cover each seeded field per row kind, including the missing-cwd and unknown-harness cases; the operator guide's description of the form says what it is seeded from
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. One seed, from the same entity the cwd seed already follows. New `src/tui/launch_seed.rs`: `launch_seed(app) -> LaunchSeed { harness, argv, cwd, pin }`, built from `action_subject(app)` (the selected row, or the right pane's entity while it has focus), so every view seeds the same entity alike.
2. Harness, argv, cwd: resolve the subject as an unsaved mux slot through `resolve_slot` (`src/resolve/slots.rs`), which already layers a pin's overrides, then the live mux's start command, then the history recipe, then the harness default. So launching like a row reads what a pin of that row would. A pin subject uses its own `PinNode` effective fields. An agent-session subject takes the session's harness, and the argv of the mux hosting it only when that mux's start command runs the same harness. A subject with none of these (group header, PR, checkout, repo) contributes nothing, and the form keeps today's defaults.
3. Harness is used only if the form can pick it (`known_harness_keys`), else the first registered as today. The argv goes in as the form's own field text (`launch_argv::field_text`, empty when it equals the harness default), and the existing `follow()` turns it into checked options and the `command` row.
4. cwd: keep `derive_mux_form_cwd`'s choice, then use it only if it is an existing directory; otherwise the cwd the form seeds for a row with none (`$HOME`). Only the launch form: the new-session form (`OpenNewMuxForm`) keeps its seed.
5. Pin: for a mux or agent-session subject, `pins_for` finds the pins pointing at it; none gives `don't pin`, one whose status is live gives `launch, then pin`, any other status (stopped, planned, unknown) gives `pin, don't launch`. Other kinds of subject (a pinned PR, checkout, repo) stay `don't pin`, since a new mux pinned because a PR is pinned would be a guess; I will flag this.
6. Form: `MuxLaunchForm::with_argv` and `with_pin`; `Msg::OpenMuxLaunchForm` in `app.rs` calls the seed instead of deriving each field inline.
7. Tests: the seed per subject (live and stopped mux, pinned and not; session in and out of a mux; pin rows live, stopped, planned; History row; group header; PR), the missing-cwd and unknown-harness cases, the form builders (options follow the argv, pin choice selected), an end-to-end open-the-form-and-read-its-request test, and one per view for the same entity. Red-check against today's behavior.
8. Docs: the operator guide's paragraph on the form says what it is seeded from; ADR 0254 (amends ADR 0096's cwd seeding; 0253 is held by CSP-905 on another branch) with the ADR index and design.md if it states the seed; CHANGELOG.
9. `just check`; a real frame of the opened form via `tui --snapshot`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Where it came from: `Msg::OpenMuxLaunchForm` derived the name and, in part, the cwd from the row, took `known_harness_keys().first()` as the harness, and never set the argv or the pin choice. Fix: `src/tui/runtime/launch_seed.rs` builds one `LaunchSeed` from `action_subject` (the entity the name seed already follows, so every view agrees), and the form gains `with_argv` and `with_pin`. A mux pin or a mux's recipe is read through `resolve_slot`, so launching like a row reads what a pin of that row would (overrides, then live, then history); a session takes its harness and the start command of the live mux hosting it when that runs the same harness.
Decisions, none the operator was asked about: (1) the pin choice follows muxes and sessions only, so a pinned PR, checkout, or repo leaves `don't pin`; (2) a session row counts as pinned only when the session itself is, as its row's marker does, not when its mux is; (3) an argv that is only the harness default is not carried, so the form keeps deciding its default, which can be a launch profile's; (4) the cwd existence check replaces the old blanket rule that never used a gone node's directory, so a stopped mux whose directory still exists now seeds it, and the row's current directory is tried before its recipe's. Only the launch form changed: the new tmux session form keeps its seed. ADR 0254 records this and amends ADR 0096; 0253 is held by CSP-905 on another branch, so the two ADRs and their README rows will want merging in order.
Verification: `just check` passed on the second run (5990 tests, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; `tuikit-features` skipped, nothing under `crates/tuikit` changed). The first full run stopped at clippy (one pedantic lint in a test, fixed), and the second stopped on `cli_history::rebuild_plan_of_a_running_mux_is_a_pasteable_script` with `server exited unexpectedly` from its private tmux server. That test passed alone 3 of 3 and in the next full run; it does not touch the launch form. Seen once, not diagnosed. With the seed reduced to the old behavior, 6 of the new behavior tests failed (the others guard unchanged cases). A real frame via `tui --snapshot` on the showcase fixture: `N` on the pinned running `project` mux opened `claude-code`, `[x] skip permissions`, `claude --dangerously-skip-permissions`, and `launch, then pin`; on the plain `ambiguous` mux, skip permissions unchecked and `don't pin`; the showcase's `/fixture/...` directories do not exist, so the cwd took the fallback. Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The launch form (`N`, or `m` then Launch harness in new mux) now starts as a copy of the entity you are looking at, not just its name: the row's harness, its launch options and other arguments (a mux started skipping permissions opens with the box checked), its directory while that still exists (else the cwd the form seeds with no row), and its pin (`don't pin`, `launch, then pin` for a pinned running row, `pin, don't launch` for a pinned stopped or planned one). It follows the same entity in every view, and a row that names none of these leaves the form as it was. Pin matching covers muxes and sessions only. New `launch_seed` module, `MuxLaunchForm::with_argv` and `with_pin`; ADR 0254 (amends 0096), the operator guide, design.md, and the CHANGELOG updated.
Verified with 14 new tests (6 fail against the old behavior), `just check` (5990 passed, after one unrelated intermittent tmux failure that passed alone and on rerun), and a rendered form over two showcase rows. Work is on branch `feat/new-mux-launch-prefill` in `.claude/worktrees/new-mux-launch-prefill`, uncommitted.
<!-- SECTION:FINAL_SUMMARY:END -->
