# Conspectus Backlog

Work tracking moved to [Backlog.md](https://backlog.md) task files under
[`backlog/`](../backlog/) (ADR 0109). Each `##` section of this file became a
milestone in `backlog/milestones/`, with its prose kept in the milestone
description, and each story, open or done, became a task file in
`backlog/tasks/`. A story's place in this file is its task `ordinal`.

Stories were renumbered from per-workstream IDs (`P8-014`, `H-PIN-TUI-011`)
to `CSP-NNN`. [`backlog-legacy-ids.md`](backlog-legacy-ids.md) maps them, and
each task file keeps its legacy ID on a `Legacy ID:` line. Use the `backlog`
CLI to read and change tasks; `git log -- docs/backlog.md` shows this file's
history.

## Before The Migration

The preamble this file carried before the migration, kept for the record:

This file is the interim work tracker for Conspectus. Keep it concise,
reviewable, and aligned with `docs/design.md` and `docs/adr/`.

Backlog.md is the preferred next-phase tool once the project needs structured
CLI queries, dependency operations, or agent/MCP integration.

### Conventions

- Use short, stable IDs so tasks can be referenced from commits and PRs.
- Keep work grouped by phase, ordered by dependency where practical.
- Record blockers explicitly.
- Move completed work to the bottom of the relevant phase instead of deleting it
  when the history is useful.
- Promote significant design decisions into ADRs before implementation relies
  on them.

### Later

- [ ] Evaluate Backlog.md migration once task count, dependencies, or
  multi-agent coordination make manual tracking cumbersome.
