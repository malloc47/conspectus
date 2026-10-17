---
id: CSP-611
title: Emit Pi fork and clone session lineage
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-610
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/session-format.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/sessions.md
documentation:
  - docs/adr/0018-intra-harness-session-lineage.md
  - docs/adr/0005-session-lineage-endpoints.md
priority: medium
type: feature
ordinal: 660000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi's `/fork`, `/clone`, and `pi --fork` create a new session file whose header `parentSession` holds the parent session's file path. Without lineage those sessions appear as unrelated siblings in the sessions graph, which hides the branching history the operator built on purpose. ADR 0018 already defines direct `parent_session` candidates between `AgentSession` endpoints for intra-harness lineage; Pi is another producer.

The parent is identified by path, not id, so the adapter has to recover the parent's session key: from the parent file's header when it is readable, otherwise from the `<timestamp>_<session-id>.jsonl` filename. The parent can sit in a different cwd directory (a cross-project fork), be outside the scan, or be deleted.

In-file `/tree` branches are a single session and stay out of scope.

Manual checks: `/fork` and `/clone` a live Pi session and confirm the child shows under its parent in the sessions graph.

Blockers: the discovery adapter story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A child whose `parentSession` points at a discovered session emits one `parent_session` candidate with a `LinkEndpoint::Node` target, carrying `harness_key` and the `lineage_kind` the Pi ADR fixed
- [ ] #2 A child whose parent file is missing or outside the scan emits a `LinkEndpoint::Unresolved` target carrying the parent key recovered from the filename and the path hint, and the resolver reports `UnresolvedEndpoint` without inventing a node
- [ ] #3 A parent in a different cwd directory resolves to the right node
- [ ] #4 A self-referencing or empty `parentSession`, and a path whose filename does not match the session-file pattern, emit no lineage candidate
- [ ] #5 Fixture and resolver tests cover the resolved, unresolved, cross-cwd, and malformed cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
