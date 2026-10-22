---
id: CSP-659
title: Supersede the mutation envelope (ADR 0087)
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
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
- [ ] #1 A new ADR supersedes ADR 0087, and ADR 0087's status names it
- [ ] #2 The new ADR keeps every existing category and prohibition and adds the clone carve-out, the daemon category 2 rule, the `pins.toml` store, and the history store, with the rationale and rejected alternatives (amending again, `gh repo clone`)
- [ ] #3 `CLAUDE.md`, `docs/capabilities.toml`, `docs/design.md`, and ADRs that list ADR 0087 as their envelope cite the new ADR
- [ ] #4 Capability manifest tests pass
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
