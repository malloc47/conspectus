---
id: CSP-699
title: Record the aider removal decision
status: To Do
assignee: []
created_date: '2026-10-24 03:19'
updated_date: '2026-10-31 08:16'
labels:
  - h-aider-removal
milestone: m-24
dependencies: []
priority: medium
type: docs
ordinal: 775000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Removing aider changes the supported v1 harness set in `docs/design.md`. It also raises questions that are hard to recover from the code later: which seams were aider-only and should go with it, and what happens to an operator config that still sets `harness_aider`. Per the project conventions, a significant decision like this needs an ADR before code changes land.

Earlier ADRs that name aider (among them 0019, 0022, 0032, 0052, 0058, 0112) record history and stay as they are. The new ADR says what changed, and the ADR index marks any decision it narrows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new ADR under `docs/adr/` records the decision to remove aider support, with the reason (unused), and covers status, context, decision, consequences, and alternatives considered (for example: keep it, or keep it behind a feature flag)
- [ ] #2 The ADR decides, for each aider-only seam, whether it is removed or kept for future harnesses: the viewer external-launch escape hatch, the `cwd` argument on `HarnessAdapter::resume_argv`, and per-repo marker scanning that is not gated by a harness state root
- [ ] #3 The ADR decides how a config that still sets the `harness_aider` theme key is handled (for example: ignored with a warning, or rejected), and how a cached `graph.bin` or history store that holds aider nodes behaves after upgrade
- [ ] #4 `docs/design.md` drops aider from the supported harness set
- [ ] #5 `docs/adr/README.md` lists the new ADR and notes any earlier ADR it amends or supersedes
- [ ] #6 `git diff --check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): two seams the ADR decides have moved. (1) The `harness_aider` theme key is a row of `ALIASES` in `src/tui/theme.rs` for the named role `harness.aider`, which `HARNESS_COLORS` registers (a test requires one entry per registered adapter); `[tui.theme.harness] aider` sets the same role. The framework's override walk (`tuikit::theme`, ADR 0153) reports a key that is neither a token, a registered name, nor an alias as `OverrideProblem::UnknownKey`, which `src/config.rs` turns into an "unknown `[tui.theme]` key" warning. Dropping both rows therefore gives "ignored with a warning"; keeping the alias row with an undrawn role (as the retired `pin_placeholder` is kept) gives a silent ignore. (2) The viewer's external-launch escape hatch is `src/tui/viewer.rs` (`claude-history` only); ADR 0052's boundary was narrowed by ADR 0180, and the native viewer is the framework's `Pager` over the viewer's `Transcript`. Also: `crates/tuikit/tests/independence.rs` lists `aider` in `DOMAIN_TERMS`; keep it, since that list keeps the host's vocabulary out of the framework whatever the host supports (`crates/tuikit/docs/separation.md`, step 7).
<!-- SECTION:NOTES:END -->
