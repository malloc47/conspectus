---
id: CSP-884
title: >-
  Match open transcripts in a moved Claude Code or Codex directory as fd
  evidence
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 23:33'
updated_date: '2026-11-15 01:50'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 961000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Process-tree attribution reads a harness process's open files and matches them against fixed path fragments in each adapter's `RuntimeSignature::fd_path_patterns` (`/.claude/projects/`, `/.claude/tasks/`, `/.codex/sessions/`, `/.codex/tmp/`). Since CSP-838 and CSP-844, discovery reads Claude Code under `CLAUDE_CONFIG_DIR` and Codex under `CODEX_HOME` (ADR 0239), and `CONSPECTUS_<HARNESS>_STATE` could already move them, but a process holding a transcript in such a directory gives no fd evidence, so a session there loses that signal for session-to-pane binding (hooks, pane commands, and cwd still attribute it). The patterns are `&'static` and `session_key_evidence_from_fd_paths` (`src/discovery/cross_link.rs`) has no access to the configured state roots. Found while doing CSP-838.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A harness process with a transcript open under a moved directory (CLAUDE_CONFIG_DIR, CODEX_HOME, or the Conspectus override) yields the same fd session evidence as one under the home default
- [x] #2 Tests cover a moved directory for each harness and leave the home-default matching unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `fd_state_dirs` to `RuntimeSignature`: directories under the harness's state root whose open files identify it (Claude Code `projects`, `tasks`; Codex `sessions`, `tmp`; the others none, so opencode and Pi are unchanged).
2. Build the fd matcher from discovery's resolved state roots (`LocalDiscoveryConfig::harness_state_roots`, ADR 0239 rule: override, harness variable, home default), plus each root's canonical path since `/proc/<pid>/fd` links are resolved; a path matches an adapter by its fixed fragments (home default, unchanged) or by lying under one of those directories (component-wise), first registered adapter wins as before.
3. Thread the roots through `cross_link::infer`, `infer_without_process_tree`, and `infer_with_fd_paths` (discovery's mutator pass, the dev scenarios, the replay harness).
4. Tests first: a moved Claude Code and Codex directory (harness variable and Conspectus override, roots from `harness_state_roots_in` with a scratch HOME) yield the same fd evidence and link as the home default; without the roots they do not; the home default still matches; a sibling directory sharing the prefix does not; a symlinked root matches through its canonical path.
5. Amend ADR 0239's consequence that named this gap; update docs/operations.md and CHANGELOG.md.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Confirmed before starting: `fd_path_patterns` were fixed fragments and `session_key_evidence_from_fd_paths` had no roots.
Tests first: with the matcher stubbed to ignore the roots, the moved Claude Code, moved Codex, and symlinked-root tests failed; the home-default and sibling-prefix tests passed (unchanged behavior). All pass with the matcher.
Scope: opencode, Pi, and aider name no `fd_state_dirs`, so their fd matching is unchanged (opencode's `CONSPECTUS_OPENCODE_STATE` is not matched; the story covers Claude Code and Codex). `infer_with_process_snapshot`, a test seam, keeps matching by the home defaults alone.
Validation: `just check` (fmt, clippy, nextest 5,806 passed, doc, diff-check) via /tmp/csp-fu/just-check.sh; /tmp/csp-fu/checks.sh clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
An agent pane's open files now identify its session under a moved Claude Code or Codex directory. `RuntimeSignature` gains `fd_state_dirs` (Claude Code `projects`, `tasks`; Codex `sessions`, `tmp`; none for the others), and `cross_link::infer`, `infer_without_process_tree`, and `infer_with_fd_paths` take discovery's resolved state roots (`LocalDiscoveryConfig::harness_state_roots`: the Conspectus override, then `CLAUDE_CONFIG_DIR`/`CODEX_HOME`, then the home default, ADR 0239). A file matches when an adapter's fixed fragment names it, as before, or when it lies under one of those directories by whole path components; a symlinked root also matches by its canonical target, since `/proc/<pid>/fd` links are resolved. The first registered adapter still wins.

Tests (src/discovery/cross_link_tests.rs), roots from `harness_state_roots_in` with a scratch HOME: a moved Claude Code directory (`CLAUDE_CONFIG_DIR` and `CONSPECTUS_CLAUDE_CODE_STATE`) and a moved Codex home (`CODEX_HOME` and `CONSPECTUS_CODEX_STATE`) give `active_pane_fd_session_match` for the right harness only, and nothing without the variable; the home defaults still match with roots default, moved, or absent; sibling-prefix and other root files do not match; a symlinked root matches through its target. ADR 0239 amended (status, consequence, amendment section; index updated); docs/operations.md and CHANGELOG.md (Fixed) updated.

Verified: `just check` passed (nextest 5,806 passed, 0 failed), `checks.sh` clean. Not covered: a live harness process (tests inject fd paths; running `claude`/`codex` is off limits).
<!-- SECTION:FINAL_SUMMARY:END -->
