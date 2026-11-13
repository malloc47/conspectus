---
id: CSP-821
title: Write the operator guide for live state
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 15:35'
labels:
  - live-state
  - ls-docs
milestone: m-26
dependencies:
  - CSP-811
  - CSP-813
documentation:
  - docs/plans/live-state/README.md
priority: medium
type: docs
ordinal: 907000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Hooks are opt-in and have known gaps, so the operator needs one page that says what live state is, how to turn it on per harness, what each state means, what is inaccurate by nature, and how to find out why nothing shows.

Scope: A "Live state" section in `docs/operations.md`: enabling it per harness (`hook init`, the Codex `/hooks` review, the OpenCode and Pi plugins), what each state and detail means, the quiet and unknown rules, the known gaps (an Esc interrupt, an approval granted, a killed process) and how each is repaired, the store file and its privacy guarantee, and a troubleshooting list keyed to the `hook status` health line. Update `docs/library-api.md` for the model types, and the README feature list if it lists capabilities.

TUI changes: none.

Tests and checks: `git diff --check`; the doc-sync tests that scan the docs still pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `docs/operations.md` explains enabling, states, gaps, privacy, and troubleshooting
- [x] #2 `docs/library-api.md` lists the live-state types
- [x] #3 The known gaps match ADR 0206
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the per-harness live-state subsections in docs/operations.md (CSP-811, CSP-815, CSP-816, CSP-817), ADRs 0203 to 0207, and the code they describe (model::live time rules, the store, the binder, hook_health labels, the CLI surfaces).
2. Reorganize docs/operations.md into one "Live agent state" section: what it is (three buckets), turning it on per harness, what each state and detail means, how a state ages (quiet, unknown, 15 minutes), where it shows (table, node show, graph JSON, hook status; not the TUI), a known-gaps table matching ADR 0206, the four per-harness subsections without the repeated store and privacy text, the store and its privacy guarantee, the health table, and troubleshooting keyed to each health label. Keep every existing anchor.
3. Update docs/library-api.md (missing live-state entry points), README.md (discovery table, limits), CHANGELOG.md.
4. Check ADRs 0203 to 0207 against the code and fix factual drift without changing decisions.
5. Update docs/plans/live-state/README.md: status, story table with task IDs and the follow-ups, section 7 mapped to tasks, an as-built section.
6. git diff --check, doc tests, checks.sh, run-tests.sh, snapshot frames; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reorganized rather than appended: the four per-harness subsections (CSP-811, CSP-815, CSP-816, CSP-817) moved from under "Hook sidecar state" into a top-level "Live agent state" section; their repeated store path, binding, and privacy sentences and the cross-harness gaps (Esc, approval, killed process, trust) now live once in shared subsections. The node show paragraph's live-state field list moved into "Where live state shows". All 21 in-page anchors resolve (checked by script).
ADR review against the code: three factual corrections (0203 quiet rule with unknown liveness, 0204 socket use on the attribution path, 0205 --fresh flag never built; --refresh serves). No decision changed. 0206 and 0207 matched.
Section 7 of the plan: decision 8 (subagent display) had no story; filed CSP-854 (needs-decision, depends on CSP-823). CSP-835 is not a live-state follow-up (m-23), so the follow-up table lists CSP-836 to CSP-853.
Validation: git diff --check clean; search_docs, comment_hygiene, capabilities_static, tui_columns_doc, doc_sync, independence pass; checks.sh exit 0; run-tests.sh exit 0, 5398 passed, 0 failed; snapshot frames (9 views) byte-identical to the CSP-819 set.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`docs/operations.md` now has one "Live agent state" section for the operator: what live state is (the three buckets), turning it on per harness (`hook init` for Claude Code, Codex with its `/hooks` trust step, and Pi with its version floor; the opencode plugin rebuild, its 2.x `opencode.jsonc` entry, and the typecheck note), what each state and detail means, how a state turns quiet or `unknown:stale` (the 15-minute rule) in plain words, where it shows today (`table --columns +live`, `node show`, `graph --format json` with an example, `hook status`; not the TUI), a known-gaps table matching ADR 0206 with what repairs each, the store file and what it never holds, and troubleshooting keyed to each health label (`off`, `no-events`, `silent`, `receiving`, `unsupported`).

- The per-harness subsections were moved under it and de-duplicated, not copied; every anchor still resolves.
- `docs/library-api.md` lists the read-rule constants, `LiveCounts::dominant`, and where `bind_live`'s file and probe types come from; README's discovery table and limits mention live state; CHANGELOG has an entry.
- ADRs 0203, 0204, 0205 corrected for three factual drifts without changing decisions.
- Plan README: stories marked done, follow-ups CSP-836 to CSP-853 tabled, section 7 mapped to tasks (filed CSP-854 for the unowned subagent-display decision), decision D9, and a section 9 "As built".

Verified: git diff --check, doc-sync and doc-scanning tests, checks.sh, full suite (5398 passed, 0 failed), and byte-identical snapshot frames.
<!-- SECTION:FINAL_SUMMARY:END -->
