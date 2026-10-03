---
id: CSP-216
title: >-
  External full-transcript viewer launch (moved ahead of the inline-widget track
  per the amended ADR 0019…
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 204000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
External full-transcript viewer launch (moved ahead of the inline-widget track per the amended ADR 0019, which treats inline preview and external launch as parallel surfaces).

- Known limitations (filed as `CSP-330` and
  `CSP-331`): recall doesn't scan opencode at the
  storage layout the user's machine actually uses (it hard-codes
  `~/.local/share/opencode/storage/session` while the real path
  is `session_diff/` + SQLite); and recall has no modal focus,
  so `j`/`k` after deep-link entry type into the search box
  rather than navigating the preview. The stderr-hold makes
  both failure modes legible from the TUI.
- **Repurposed by ADR 0052**: external viewer launch is no
  longer the default `T` target. The native in-tree viewer
  (`H-VIEWER-NATIVE-*`) takes over the default. The code
  shipped under this story stays as the *escape-hatch* path
  operators reach via `[viewers.<harness>]` config
  (`CSP-332`). The recall-specific surface
  (`RecallViewer`, `supports_flag` capability probe,
  `required_flags` trait method) was ripped out alongside
  `CSP-341`; only `ClaudeHistoryViewer` remains
  as the hardcoded escape-hatch backend. Operators who want
  recall back configure it via `CSP-332` once that
  lands.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/tui/viewer.rs` defines `SessionViewerAction`,
`LaunchPlan`, and `ViewerDisabled` per the amended ADR 0019.
Two backends ship: `ClaudeHistoryViewer` (resolves the on-disk
`<state_scope>/projects/*/<session_key>.jsonl` via std
`read_dir` and passes it as the positional arg —
`claude-history --show-id` *prints* the id, the interactive
viewer takes a file path) and `RecallViewer`
(`recall --session <session_key>`, multi-harness: claude-code,
codex, opencode, factory, droid). `resolve_viewer_target`
prefers harness-specific over multi-harness. PATH discovery
uses a std-only `BinaryProbe` walker (no new dep). The probe
also feature-detects `recall --help` for `--session` because
upstream `recall 0.5.0` lacks the flag; the conspectus Nix
overlay carries a patched `recall 0.5.0-conspectus-session`
until upstream lands a PR. `plan()` is fallible
(`Result<LaunchPlan, String>`) so claude-history can return
a `ViewerDisabled::TranscriptNotFound` hint when the JSONL
file isn't on disk; the resolver falls through to recall
when one is configured. The `T` keybind routes through
`Action::View` → `view_action`. `run_viewer_launch` does an
explicit `Clear(All) + MoveTo(0,0)` after `ratatui::restore`
so mosh / nested muxers stop intertangling the child's
first writes with the dropped TUI buffer, and holds for
Enter on non-zero exit so the operator can read the
viewer's stderr before the alt screen reclaims the
terminal. Help overlay registers the new binding. Fourteen
unit tests cover supported / unsupported harnesses, the
"no binary on PATH" path, the preference order, the
capability-probe gating for unpatched recall, and the
transcript-not-found + recall-fallback paths.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TRANSCRIPT-012`
