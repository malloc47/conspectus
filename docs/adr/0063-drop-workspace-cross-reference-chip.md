# ADR 0063: Drop The Workspace Cross-Reference Chip

## Status

Accepted. The chip removal remains in force; the workspace surfaces
this ADR references were later reshaped by ADR 0064 and ADR 0065
(the dedicated Workspaces view is gone; `SessionsGrouping::Workspace`
is the successor surface).

## Context

`CSP-405` paired two changes to the Sessions / Graph view: a
**strict-nesting** fix (only nest a session under a workspace
header when the session has a direct `AgentSession --
AssociatedWith → Workspace` edge — the (A) case) and a
**cross-reference chip**, `[ws-name]` or `[N ws]`, that surfaced
on (B)-class rows — sessions whose cwd lives in a repo a
workspace claims as a member but which carry no workspace edge.

The chip's purpose was to preserve the cross-reference signal
that the fallback nesting rule was overproducing. ADR-less
follow-ups added two refinements:

- **Activeness gate.** A workspace's chip fires only when at least
  one (A)-class session lives in its tree — dormant workspaces
  were suppressed to avoid wall-of-chips noise on shared utility
  repos.
- **Cardinality threshold.** `WEAK_WORKSPACE_CHIP_MAX = 3`:
  1 weak membership → `[ws-name]`, 2..=3 → `[N ws]`, 4+ →
  suppressed.

The chip rendered only in Sessions / Graph; CSP-407 confirmed the
other four views (Mux/Prs/Forks/Union) had no analog to surface
and that the Workspaces view's CSP-406 polish (ADR 0062) had
already narrowed the (A)/(B) decision to "Workspaces shows only
(A); (B) lives only as the Sessions chip."

After running the chip for several weeks the operator reported
that the (B) signal was not telling them anything actionable.
Knowing that a session in `~/src/conspectus` happens to live in a
repo some workspace claims as a member did not help them choose
what to do next — the only useful workspace relationship is "the
session's cwd is in the workspace," which the strict nesting
already conveys clearly. The chip's "fyi cross-reference" was
fyi-only and added a column of dim noise to every (B)-class row
in the daily-driver setup.

This is the third place the (A)/(B) distinction has been narrowed
in the UI. The first was CSP-406 → ADR 0062 (drop `related`
from the Workspaces view). The second was CSP-407 → ADR 0061
(drop unimplemented `Workspace` grouping from the four
non-Workspaces views). This ADR is the third: drop the chip from
the one view it still rendered in. (A)/(B) remains load-bearing
at the data-model layer — `cross_link::infer` still emits
`AssociatedWith Workspace` only for (A) and the strict-nesting in
Sessions / Graph still depends on it — but the UI no longer
surfaces (B) as a chip, subgroup, or grouping option anywhere.

## Decision

Remove the workspace cross-reference chip from the Sessions /
Graph view.

- Delete `WEAK_WORKSPACE_CHIP_MAX`, `workspaces_for_repo`,
  `active_workspaces`, `workspace_display`, and
  `weak_workspace_chip` from `src/tui/rows/sessions.rs`.
- Delete the chip-emit branch in `emit_session` and the
  `workspace_chip` field on `AgentSessionRow` in
  `src/tui/rows/mod.rs`.
- Delete the chip-rendering block in `render_session_spans`
  (`src/tui/ui.rs`).
- Drop the `workspace_chip: None,` placeholders from the
  Mux/Prs/Forks/Union/Workspaces row builders, both search
  backends, and the six `ui.rs` test row constructors.
- Delete the four chip-specific Sessions tests
  (`repo_shared_session_stays_at_repo_level_with_chip`,
  `dormant_workspace_does_not_chip_repo_shared_sessions`,
  `repo_in_multiple_workspaces_renders_n_ws_chip`,
  `repo_in_many_workspaces_suppresses_chip`). Refactor the two
  tests that mixed nesting + chip assertions
  (`workspace_rooted_session_has_no_chip_even_when_repo_is_in_other_workspaces`
  is dropped; `repo_shared_session_stays_at_repo_level_with_chip`
  becomes `repo_shared_session_stays_at_repo_level` and keeps
  only the strict-nesting assertions).

Keep the strict-nesting fix from CSP-405 in place — the
`workspace_for_session` and `workspace_for_repo` helpers it
depends on, the `cross_link::workspace_member_roots` fix to
require cwd inside the workspace's visible tree, and the
`symlinked_workspace_member_does_not_associate_session_at_canonical_path`
regression test all stay.

## Consequences

- (B)-class sessions in Sessions / Graph render identically to
  sessions whose repo has no workspace membership at all. The
  only visible workspace context in the TUI is the workspace
  header above (A)-class rows in Sessions / Graph and the
  workspace top-level row in the Workspaces view. The
  cross-reference signal is no longer exposed anywhere.
- `src/tui/rows/sessions.rs` loses ~95 lines of helpers and the
  `BTreeSet` import; the `WorkspaceId` import remains for the
  strict-nesting helpers.
- `AgentSessionRow` drops a field and its docstring; every row
  builder that constructed one loses a line; the six `ui.rs`
  test sites lose a line each.
- The `[ws-name]` / `[N ws]` vocabulary is gone from the UI
  entirely. Tests covering CSP-405's strict-nesting outcome
  remain; tests covering the chip are deleted.
- CSP-405's net contribution becomes: strict workspace nesting
  in Sessions / Graph, plus the cross-link inference fix
  (`workspace_member_roots` no longer indexes
  `canonical_checkout_root`, fixing the symlinked-member leak).
  The chip is reverted in full.

## Alternatives Considered

### A. Make the chip opt-in via a config flag

Keep the helpers but hide the chip behind a setting like
`[tui.views.sessions].show_workspace_chip = true` so operators
who want the cross-reference can turn it on. Rejected because the
operator who exercised the chip in the daily-driver setup is the
same operator now requesting its removal; no use case has been
articulated by anyone else. Adding a config knob preserves the
maintenance cost of the helpers, the activeness gate, the
cardinality threshold, and three layers of UI plumbing for a
feature with no demonstrated consumer.

### B. Replace the chip with a less prominent indicator

Swap the `[ws-name]` chip for a smaller glyph (a colored dot, a
single-character marker) so the signal stays visible without
consuming column width. Rejected because the operator complaint
is not about the chip's visual cost but about the signal's
informativeness — a quieter chip pointing at the same uninformative
relationship would solve the wrong problem.

### C. Keep the chip but drop the activeness gate

Surface every workspace membership, regardless of whether the
workspace is live. Rejected unambiguously by the CSP-405
follow-up findings; ungated chips were the wall-of-chips
behavior the activeness gate was added to suppress.

## Open Questions Answered

- *Q: Is the (B)-class chip carrying its weight in the daily-driver
  view?* (open since the CSP-405 follow-ups)
  **A: No. The cross-reference signal is not actionable; the
  strict-nesting fix carries the useful (A) relationship without
  the chip's help.**
