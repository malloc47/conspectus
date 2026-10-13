---
id: CSP-599
title: Reconcile ADR 0087's harness-state prohibition with hook installation
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 03:24'
updated_date: '2026-10-13 04:00'
labels:
  - h-caps
milestone: m-11
dependencies: []
priority: medium
ordinal: 648000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Writing the capability manifest (ADR 0117) surfaced a conflict. ADR 0087's first prohibition lists `~/.claude/` as harness-owned state Conspectus never writes, yet ADR 0028's `conspectus hook init|remove` edits `~/.claude/settings.json` and `~/.codex/config.toml`, with `--scope project` variants.

The manifest records this as its own `hook` context so the tests pass. Decide one of:
- amend ADR 0087 to sanction operator-initiated hook installation as a category;
- narrow hook install, for example to print-only.

Update the manifest and docs to match.

Blockers: an operator decision.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision: amend ADR 0087 rather than narrow hook install to print-only. Hook install predates ADR 0087 (it comes from ADR 0028), so the amendment records an existing sanctioned write rather than opening a new one. It extends the ADR instead of superseding it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Amended ADR 0087, now "Accepted (amended)".

Changes to the ADR itself:
- Hook installation the operator starts (`hook init` / `hook remove`, ADR 0028) is sanctioned category 5: one idempotent hook entry in the harness settings file, which keeps every other setting, is written through a temporary file, and has a `--dry-run`.
- Prohibition 1 now excludes exactly that entry.
- An amendment section records why this extends rather than supersedes the ADR, and why print-only was rejected.
- The code-review `grep` advice now points at the ADR 0117 manifest.

Matching updates:
- ADR 0117's consequence now reads as resolved, and the manifest's `hook` context cites ADR 0087 category 5.
- The envelope restatements in AGENTS.md (and so CLAUDE.md), `docs/design.md`, and the README now list five categories.

Verified with `just check` (2,314 tests, including both capability tests) and `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
