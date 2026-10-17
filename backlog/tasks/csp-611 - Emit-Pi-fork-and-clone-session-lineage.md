---
id: CSP-611
title: Emit Pi fork and clone session lineage
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 13:44'
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
- [x] #1 A child whose `parentSession` points at a discovered session emits one `parent_session` candidate with a `LinkEndpoint::Node` target, carrying `harness_key` and the `lineage_kind` the Pi ADR fixed
- [x] #2 A child whose parent file is missing or outside the scan emits a `LinkEndpoint::Unresolved` target carrying the parent key recovered from the filename and the path hint, and the resolver reports `UnresolvedEndpoint` without inventing a node
- [x] #3 A parent in a different cwd directory resolves to the right node
- [x] #4 A self-referencing or empty `parentSession`, and a path whose filename does not match the session-file pattern, emit no lineage candidate
- [x] #5 Fixture and resolver tests cover the resolved, unresolved, cross-cwd, and malformed cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Parse the header `parentSession` into the cached `PiSession`. Recover the parent key from the parent's header when it is readable, otherwise from its `<ts>_<id>.jsonl` file name. Drop non-`.jsonl` paths, names without the pattern, empty values, and self-references.
2. Emit one ADR 0018 `parent_session` candidate per child: a `Node` target when the parent key was discovered under the same root, otherwise an `Unresolved` endpoint with the key and path. The metadata carries `harness_key=pi`, `lineage_kind=fork`, and `lineage_fidelity=native`.
3. Add fixtures under `tests/fixtures/pi/lineage`, unit and resolver tests, and a corpus snapshot.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pi forks and clones now emit ADR 0018 `parent_session` candidates from the header `parentSession` path. The metadata carries `harness_key=pi`, `lineage_kind=fork`, `lineage_fidelity=native`, `parent_session_id`, and `parent_session_path`.
- The parent key comes from the parent file's header when it is readable, otherwise from its file name.
- A parent discovered under the same root gets a `Node` target, whatever cwd directory it sits in.
- Otherwise the target is `Unresolved`, with the key and the path as a hint.
- Empty values, self-references, non-`.jsonl` paths, and names without `<ts>_<id>` emit nothing.

Verification:
- `just check`: 2348 tests, clippy, fmt, rustdoc, and the diff check.
- New `pi_tests` cover the resolved cross-cwd parent, the unresolved missing parent with key and path, the four no-lineage cases, the resolver (one `ParentSession` relationship plus one `UnresolvedEndpoint` diagnostic, with no invented node), a readable parent outside the scan keyed by its header id, and no lineage for the grouped and flat fixtures.
- The `fixture_corpus` snapshot `pi_lineage_candidates_snapshot` records the candidates.
- Manual: `conspectus table sessions --harness pi --columns agent,cwd,lineage` against the lineage fixture shows the cross-cwd child pointing at its parent and the missing parent as `?…0000dead`.

Pi is not installed on this machine, so the live `/fork` and `/clone` check was not done.
<!-- SECTION:FINAL_SUMMARY:END -->
