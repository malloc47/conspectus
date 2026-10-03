---
id: CSP-263
title: Add a sanitized real-state fixture corpus
status: Done
assignee: []
created_date: '2026-05-24 20:27'
labels:
  - test
milestone: m-11
dependencies:
  - CSP-262
ordinal: 274000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: create checked-in fixture directories for representative
  real provider shapes that synthetic builders have historically
  missed: Codex rollout JSONL files, Claude Code transcript
  envelopes and lineage variants, hook payloads, tmux discovery
  rows, and `/proc/<pid>/fd` target strings. Add a small sanitizer
  script or documented command sequence that strips usernames,
  absolute private paths, tokens, prompt contents, and host-specific
  IDs while preserving schema shape and edge-case fields.
- Tests: fixture-load tests that parse every corpus file and assert
  the expected node/link or payload shape; no test reads the user's
  real `~/.codex`, `~/.claude`, tmux server, or `/proc`.
- Manual checks: run the sanitizer against a known live failure and
  confirm the resulting fixture is reviewable, deterministic, and
  free of private transcript text.
- Blockers: `CSP-262` for replay integration; the corpus can start
  with parser-only tests before the replay harness is complete.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/fixtures/` with sanitized corpus files for
Codex (full, minimal, forked rollouts), Claude Code (basic,
resume, fork transcripts), hook payloads (Claude SessionStart,
Claude ephemeral, Codex SessionStart), tmux discovery rows (multi-
session, paths-with-spaces), and `/proc/<pid>/fd` targets
(rollout, task, opencode paths plus socket/pipe/anon-inode
descriptors). Added `tests/fixture_corpus.rs` with 13 fixture-load
tests that parse every corpus file through the actual adapter,
hook-parser, and row-parser paths and assert expected node/link
shapes and edge-case field handling. A 7-step sanitization
workflow is documented in the test file header.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-002`
