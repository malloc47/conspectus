---
id: CSP-342
title: '(later) Extraction prep: lift `src/viewer/` into a workspace member crate'
status: To Do
assignee: []
created_date: '2026-06-02 18:59'
updated_date: '2026-10-31 08:16'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 224000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope (re-scoped 2026-10-31 by the TUI framework's backlog review, CSP-766; the original scope is kept below): the workspace already exists (CSP-717, ADR 0150), and the transcript viewer is the framework's `tuikit::pager::Pager` in Screen mode over `src/viewer`'s `Transcript<M>` content (CSP-748, ADR 0180). The generic pager moved to the framework; `src/viewer/` keeps only the transcript: the model, the parsers, rendering, and the pager content with its find unit, hidden-match count, toggles, and opening at a search result. It imports nothing from `crate::tui` and lists `tuikit` in `docs/transcript-viewer-deps.md`. Lifting it into a member crate (`crates/conspectus-transcript-viewer/`) therefore carries `tuikit` (with its `richtext` feature) as a dependency and no reader of its own. A thin clap `bin` that takes a `SessionLocator` from argv would draw `Pager<Transcript>` in a loop of its own (about 30 lines; the application runtime stays in Conspectus, ADR 0200, `docs/plans/tui-framework/runtime-evaluation.md`). Conspectus depends on the crate as a workspace member; its intra-doc links to `crate::discovery` and `crate::model` become plain text.
- Tests: the viewer tests run from the new crate; the TUI still opens transcripts through `src/tui/viewer_bridge.rs` unchanged; the repository's source-scanning tests are extended to the new crate, as CSP-719 did for the framework.
- Blockers: a reason to ship the viewer on its own (no consumer is scheduled). The dependency surface the original blocker waited on is settled: the viewer depends on `ratatui`, `tuikit`, and its parsers' crates, as `docs/transcript-viewer-deps.md` lists.

Original scope (2026-06, kept for history):
- Scope: when the viewer module's import surface has been stable for ≥ N stories, create a `crates/` workspace, move `src/viewer/` to `crates/conspectus-transcript-viewer/`, add a thin `bin` target with clap that takes a `SessionLocator` from argv. Conspectus depends on it as a workspace member.
- Tests: existing viewer tests run from the new crate location; conspectus TUI still routes through it unchanged.
- Blockers: stability of the dep surface (see `docs/transcript-viewer-deps.md` history).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-010`
