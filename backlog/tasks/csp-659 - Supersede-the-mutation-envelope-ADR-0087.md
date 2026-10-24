---
id: CSP-659
title: Supersede the mutation envelope (ADR 0087)
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 02:34'
labels:
  - history-pins
milestone: m-23
dependencies: []
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0087-mutation-envelope.md
  - docs/adr/0117-capability-manifest-and-traced-backstop.md
  - docs/capabilities.toml
  - CLAUDE.md
priority: high
type: docs
ordinal: 717000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The plan adds write paths and one new kind of mutation: a history store the daemon writes, a pin store in `$XDG_DATA_HOME`, and cloning a repo during a rebuild. ADR 0087 says removing a prohibition takes supersession, and the operator chose to supersede it with a new envelope ADR rather than amend it again. Cloning goes through `git clone`; `gh repo clone` was considered and rejected because it only covers GitHub remotes.

- Scope: a new envelope ADR restating every sanctioned category and prohibition from ADR 0087 and its 2026-10-12 amendment, plus: a narrow `git clone <remote> <path>` carve-out from the git prohibition (act context only, destination must not exist, no other git command); an explicit rule that the daemon may write category 2 observations (`graph.bin`, history) and never categories 1, 3, 4, or 5, resolving ADR 0090's stricter reading; `$XDG_DATA_HOME/conspectus/pins.toml` as a category 1 store; placement of the history store and every rebuild step. Every live citation of ADR 0087 moves to the new ADR.
- Tests: `git diff --check`; `tests/capabilities_static.rs` and `tests/comment_hygiene.rs` still pass after citations move.
- Manual checks: none.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A new ADR supersedes ADR 0087, and ADR 0087's status names it
- [x] #2 The new ADR keeps every existing category and prohibition and adds the clone carve-out, the daemon category 2 rule, the `pins.toml` store, and the history store, with the rationale and rejected alternatives (amending again, `gh repo clone`)
- [x] #3 `CLAUDE.md`, `docs/capabilities.toml`, `docs/design.md`, and ADRs that list ADR 0087 as their envelope cite the new ADR
- [x] #4 Capability manifest tests pass
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write ADR 0143 restating ADR 0087's categories and prohibitions under the same numbers, adding the history store, pins.toml, the daemon category 2 rule, and a git clone carve-out (category 6).
2. Move live 0087 citations (AGENTS.md, capabilities.toml, design.md, code comments, docs) to 0143; add 0143 pointers to ADRs whose Related ADRs list 0087.
3. Run the capability and comment-hygiene tests.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded ADR 0143, superseding ADR 0087: every category and prohibition keeps its number, plus pins.toml in category 1, history in category 2, an explicit daemon rule (category 2 only), and category 6 for `git clone` into an absent path, with placement tables for history, slots, and rebuild steps and the rejected alternatives (amending again, `gh repo clone`). `AGENTS.md`/`CLAUDE.md`, `docs/capabilities.toml`, `docs/design.md`, code comments, and envelope-citing ADRs now cite 0143. `capabilities_static`, `capabilities` (traced), and `comment_hygiene` pass; `git diff --check` clean.
<!-- SECTION:FINAL_SUMMARY:END -->
