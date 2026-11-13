---
id: CSP-840
title: Parse only the whitelisted fields of a hook payload in hook write
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 06:06'
updated_date: '2026-11-13 19:28'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 925000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook write` parses the whole stdin payload into a `serde_json::Value` before any adapter sees it, and every `HarnessAdapter` hook method takes that `Value`. The Claude Code mapping then reads only a few named fields, but the full parse has already allocated every string, including a `PostToolUse` `tool_response` that can run to megabytes (a large file read, a long command output), on every tool call. Research note 01 (sections 0 and 4.2) asks for a typed whitelist parse that skips the rest; the adapter side is done, the writer side is not.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 hook write never builds a full Value of a payload; unknown fields are skipped during parsing
- [x] #2 Every harness adapter keeps its current mapping, attribution, and privacy tests green
- [x] #3 A test shows a payload with a multi-megabyte tool_response costs about what a small one does
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `crate::hook::payload`: `PayloadField` (a name and the fields read inside it) and `parse(input, fields)`, a serde `DeserializeSeed` over serde_json's slice deserializer that keeps each named field at the depth named and skips every other value with `IgnoredAny`, so it is never built. A field read as a scalar that holds an object or list, or a nested field that holds a scalar, reads as `null`, which every adapter treats as it treats the original (absent or the wrong type).
2. `HarnessAdapter::hook_payload_fields` (default: the session fields the default record reads); Claude Code, Codex, opencode, and Pi declare theirs beside their mapping tables.
3. `hook write` parses stdin with the adapter's fields instead of a full `serde_json::Value`; the adapter methods keep their signatures, so every mapping, attribution, and privacy test runs unchanged.
4. Tests: parser unit tests; a corpus test that every fixture payload of every adapter, and type-mangled variants of every field, map identically through the whitelist and through the full value; the sequence runner goes through the whitelist; an integration test with a counting allocator showing a multi-megabyte `tool_response` allocates what a small one does.
5. Docs: adapter module docs, CHANGELOG, plan README as-built note.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built `crate::hook::payload` (a serde `DeserializeSeed` over serde_json's slice deserializer; no new dependency or feature) and `HarnessAdapter::hook_payload_fields`; Claude Code, Codex, opencode, and Pi declare their fields beside their mapping tables. A field of the wrong kind reads as null, which every adapter reads as absent, as it read the wrong kind before. Residual: a string where an object is read (a bare-string `tool_response`) is still unescaped by serde_json into its scratch buffer before being dropped; objects and lists, the shapes Claude Code sends, are skipped without a copy.
Validation: run-some.sh --lib discovery::harness (327 passed, incl. the corpus equivalence tests; removing `is_interrupt` from the Claude Code list makes it fail); --lib hook::payload; --test hook_payload_cost (a 7 MB tool_response allocates no more than a small one, within 4 KiB); checks.sh exit 0; run-tests.sh exit 0, 5,643 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook write` no longer builds a full `serde_json::Value` of a payload. It parses stdin with `crate::hook::payload::parse` against the fields the adapter declares in the new `HarnessAdapter::hook_payload_fields` (default: the session fields; Claude Code, Codex, opencode, and Pi name theirs beside their mapping tables), keeping each named field at the depth named and skipping every other value with `IgnoredAny`, unbuilt. Adapter signatures are unchanged, so every mapping, attribution, and privacy test runs as before, and the sequence runner now goes through the whitelist. A corpus test checks every fixture payload of every adapter, and every single-point type swap or removal of its values, maps identically through the whitelist and the whole value; an integration test with a counting allocator shows a 7 MB `tool_response` allocates what a small one does. checks.sh and the full suite (5,643 passed) pass. CHANGELOG.md, the adapters' module docs, and the plan README's as-built notes updated.
<!-- SECTION:FINAL_SUMMARY:END -->
