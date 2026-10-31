# ADR 0150: The TUI Framework Is A Workspace Member Crate

## Status

Accepted. Narrows ADR 0024 (the TUI's module layout) and ADR 0052 (the
"extractable viewer" boundary): generic widgets and conventions move to the
framework crate; the transcript model and parsers stay in Conspectus.

## Context

The TUI has grown to about 86,000 lines (tests included) with 25 widget
modules, a theme, a glyph slate, key tables that generate footers and help, an
overlay contract, and a consistency suite. The inventory in
`docs/plans/tui-framework/inventory.md` found that most of that is not
Conspectus-specific, and that the generic parts have drifted apart: 13
truncation helpers, 6 word-wrap helpers, 4 selection-highlight looks, 4
confirmation idioms, 3 single-line editors, and 7 scrolling readers that are one
shape.

The operator wants the look and feel, controls, and widgets to live in a
reusable "design framework" crate the way a component library sits apart from
the apps that use it, ready to move to its own repository when that becomes
useful. Today `conspectus` is a single Cargo package, so nothing but review
keeps a widget from importing `crate::tui::Msg` or a harness color.

## Decision

1. **Boundary.** The framework is a **workspace member crate** at
   `crates/tuikit/` (package `tuikit`, a working name; see ADR 0152).
   The repository becomes a Cargo workspace whose root keeps the `conspectus`
   package. `conspectus` depends on `tuikit` by path. `tuikit` depends on
   nothing from `conspectus`.

2. **Scope.** The first cut covers two layers:
   - **Layer 1, components and design tokens:** widgets, theme and palette,
     glyphs, chips, text and scroll helpers, layout frames.
   - **Layer 2, interaction conventions:** key tables that generate footers and
     help, the overlay and modal-stack contract, focus rules, a feedback model,
     and a test kit (key DSL, ANSI-preserving frame driver, convention
     conformance harness).

   **Layer 3, the application runtime** (the MVU event loop, effect lifecycle,
   terminal suspend and resume for subprocess hand-off), stays in Conspectus.
   The plan carries one optional story near the end that evaluates extracting
   it.

3. **Internal reuse only.** The crate is `publish = false`. It carries no
   semver promise and has no release pipeline. Its documentation still has to
   stand alone (README, architecture, per-control reference, contribution
   rules), because the goal is that it can leave the repository at any time.

4. **Independence rules.** Each is enforced by a test, not by convention:
   - The framework's `Cargo.toml` names no `conspectus` crate, and its
     dependency set is an allowlist.
   - No source, doc, or test file in the framework refers to Conspectus
     vocabulary (sessions, muxes, harnesses, pins, worktrees, PRs, graph
     nodes). Domain meaning enters through type parameters, closures, traits,
     and a named-style registry.
   - The framework starts no processes and writes no files. Hosts supply sinks
     (the clipboard takes a `Write`; path completion takes a source). The
     capability manifest (ADR 0117) therefore needs no framework entries.
   - Each control is generic over the host's message type `M` and, where it
     needs host data, a context type `Cx`; none reads global state.
   - The repo's source-scanning tests cover the framework's tree, with a
     tripwire that fails if a scan visits no files.

5. **Behavior change policy.** Consistency first. A story that extracts a control
   unifies it on the best variant in the same change, and catalogs every visible
   difference as a regression or a consistency win (ADR 0151).

6. **Workspace mechanics.** Shared dependency versions move to
   `[workspace.dependencies]` and the curated clippy levels to
   `[workspace.lints]`. CI, the `justfile`, the dev-shell banner, and the
   Pre-Main Invariants run `--workspace`, so the framework's tests cannot go
   unrun without failing.

## Consequences

- Moving the framework to its own repository later is a directory lift plus a
  separation rehearsal story that proves it builds, tests, and documents itself
  alone. ADR 0199 makes the rehearsal a CI job that runs on every change.
- Each extraction is a sequence of small, reviewable stories rather than one
  rewrite. Each leaves `main` releasable.
- The compiler rejects `Msg`, `App`, `RowKind`, and domain theme fields inside
  the framework, so the work of making a control generic is visible as type
  parameters and traits.
- The root `Cargo.toml`, CI, `justfile`, `flake.nix` banner, and `CLAUDE.md`
  and `AGENTS.md` invariants change once, in the workspace-conversion story.
- Five source-scanning tests (`test_module_hygiene`, `comment_hygiene`,
  `overlay_keymap_hygiene`, `capabilities_static`, and the consistency text
  scan) walk fixed `src/` paths and would pass vacuously after a move. A
  guardrail story extends them and adds tripwires before any code moves.
- CSP-342 (lift `src/viewer/` into a workspace member crate) lands on the same
  workspace; its shape changes because the generic pager moves into the
  framework and the transcript model stays.
- Conspectus's conventions in `CLAUDE.md` for new views, menus, and keys now
  describe framework controls; a closing story updates them.
- The framework keeps its own decision log in `crates/tuikit/docs/decisions/`:
  one short record per framework decision, written without Conspectus
  vocabulary, each naming the ADRs it came from. The ADR index marks those
  ADRs framework-owned, wholly or in part. **Before separation**, while the
  framework lives in this repository, the ADRs here are canonical for every
  decision that originated here; a framework record restates the decision and
  points at its ADR, and when they disagree the ADR wins and the record is
  corrected. A decision that also changes Conspectus gets an ADR and a
  framework record in the same change; a decision about the framework alone
  (its documentation, tests, or internal conventions) may be a framework
  record alone. **After separation**, the framework's log is canonical: the
  ADR numbers in its records become history, and these ADRs keep pointers to
  the framework's records. The framework's doc-sync test holds the log, its
  index, and the marks here to one another while the two share a repository.

## Alternatives Considered

- **A module boundary with a static guard** (`src/tui/kit/` plus a test that
  forbids `crate::` imports). Less churn at the start, but enforcement is a
  scan, and a leak is a test failure someone can allowlist away. The compiler
  is a stronger check. Rejected by the operator.
- **A separate repository now.** Premature: the controls still import app
  types, and every change would need a two-repo release until the seams are
  clean. The workspace crate keeps atomic changes while the API settles.
- **Independence by copy** (the `ratcn` model, where components are copied
  into each app with a fixture that proves they stand alone). Gives each app
  its own divergent copy, which is the drift this work removes.
- **Adopt an existing framework.** `rat-widget` was evaluated and rejected
  (CSP-433: mandatory focus, event, and scroll traits; all-or-nothing per
  surface). `tui-realm` and the component-tree frameworks fit poorly: the
  architecture review (`docs/tui-architecture-review.md`) concluded the UI is
  two panes plus modals over one shared graph. The young design kits
  (`tuika`, `tui-lipan`, `ratatui-themekit`) churn too fast to depend on and
  are better read for vocabulary.
- **Include the runtime (layer 3) in the first cut.** It is the most
  entangled part (effects, undo, and the reducer know the graph model), and
  the layers below are useful without it. Deferred to an optional evaluation.

## Open Questions Answered

- *Where does the generic transcript pager live?* In the framework as `Pager`;
  the transcript model, parsers, and gutter chips stay in Conspectus as a
  `PagerContent` adapter.
- *Do the five hygiene tests move?* Their mechanisms are reusable helpers in
  the framework's test kit; their rule sets are repo policy and stay.
- *What about `docs/capabilities.toml`?* The framework spawns nothing and writes
  nothing, so it stays out of the manifest; a guard fails the build if that
  changes.
