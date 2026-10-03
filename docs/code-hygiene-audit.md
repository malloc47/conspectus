# Code Hygiene And Simplification Audit

Status: draft audit, 2026-07-01. Companion to
`docs/extensibility-assessment.md` (which covers provider seams; this
document deliberately does not repeat those findings). Goal: identify
simplicity, redundancy, and Rust-hygiene work worth doing before humans
invest sustained time in the codebase.

Method: `cargo clippy --all-targets --all-features` at the project's default
lint level (clean), the same run with `-W clippy::pedantic -W clippy::nursery`
(1,873 warnings, inventoried below), duplicate-definition analysis across
`src/`, per-file production-vs-test line measurement, and targeted reads of
the largest production files.

## Headline Numbers

| Metric | Value |
| --- | --- |
| `src/` total | ~104k lines |
| … production | ~59k lines |
| … in-file `mod tests` | ~45k lines (43% of `src/`) |
| `tests/` integration | ~9.3k lines |
| Default-level clippy | clean (`-D warnings` in `just check`) |
| Pedantic+nursery warnings | 1,873 |
| `panic!`/`unreachable!` in production paths | 6 |
| `.unwrap()` in production paths | 19 |
| Duplicated named helpers (verified byte-identical clusters) | 7 families |

Overall verdict: **the codebase is in much better shape than "vibecoded"
implies** — error handling is disciplined, the module boundaries match the
ADRs, dependencies are curated and individually justified, and provider
identity flows as data rather than as scattered branches. The debt is
concentrated in three shapes: (1) copy-paste helper duplication across the
per-view/per-projection modules, (2) five TUI monolith files, and (3) test
scaffolding that was grown by repetition instead of by builders. None of it
is architectural; almost all of it is mechanical to fix.

## A. Verified Copy-Paste Duplication

Each item below was confirmed by diff/hash, not just name collision.

- **A1. `collect_agent_mux_candidate_counts` ×6.** Two identical clusters:
  `src/output/{prs,forks}.rs` (md5-identical pair) and
  `src/tui/rows/{union,prs,forks,mux}.rs` (md5-identical quadruple). Every
  view/projection rebuilds the same session→mux-candidate-count `HashMap`
  from a full `candidate_links` scan. One shared query helper (see C2)
  removes all six.
- **A2. `agent_row` / `mux_indicator` / `session_matches_filter` ×3–4.**
  Byte-identical across `src/tui/rows/{union,prs,forks}.rs` (and a near-twin
  in `rows/mux.rs`). The row-model assembly for "an agent session rendered
  as a child row" is copy-pasted per view. The `RowFilter` predicate itself
  is properly shared in `src/filter.rs` — only the per-view glue was cloned.
  Extract into `src/tui/rows/mod.rs`.
- **A3. `snapshot_fragment` ×7.** The trivial `GraphSnapshot → GraphFragment`
  field-mover is redefined in `discovery/mod.rs`, `harness/mod.rs`,
  `forge/mod.rs`, `agent_deck.rs`, `atelier.rs`, `workspace.rs`, and
  `git.rs`. Should be `impl From<GraphSnapshot> for GraphFragment` in
  `discovery/mod.rs` (or a method on `GraphFragment`), used everywhere.
- **A4. `current_epoch` ×5.** `discovery/mod.rs`, `hook.rs`,
  `codex_log.rs`, `hook_sidecar.rs`, `output/render.rs` each define the same
  wall-clock-to-epoch helper. One `util` (or `model::time`) function; the
  call sites that take `now` as a parameter for testability stay unchanged.
- **A5. `path_string` ×5** (`git.rs`, `atelier.rs`, `agent_deck.rs`,
  `workspace.rs`, `dev_scenarios.rs`) — same lossy path→String conversion.
- **A6. `centered_modal_rect` ×6.** `widgets/{help,controls,search,input,
  multi_select,pins}.rs` each hand-roll centered-rect math differing only in
  width cap / height policy. Parameterize once (`fn centered_rect(area,
  MaxWidth, HeightPolicy)`) in the widgets module — `tui-popup` is already a
  dependency and `popup_frame.rs` exists, so this may collapse further into
  the existing frame helper.
- **A7. Dual event loops in `src/tui/runtime.rs`.** `event_loop` (:105,
  ~240 lines) and `static_event_loop` (:344, ~205 lines) share the same
  body skeleton (toast tick, poll, key dispatch, render) and differ only in
  refresh strategy (live discovery channel vs fixture reload). Extract one
  loop driver parameterized by a refresh source; the ADR 0069 fixture mode
  keeps its distinct entry point.
- **A8. Per-projection `cell(key, ctx)` dispatchers ×5**
  (`output/{agent,mux,union,prs,forks}.rs`). Structurally parallel rather
  than identical — each matches column keys to formatters, and many arms
  (mux cell, provenance cell, age cell) repeat. A shared column-formatter
  registry keyed by column id, with per-projection extras, would shrink
  these and make new columns single-site. Lower priority than A1/A2 since
  the drift risk is bounded by snapshot tests.
- **A9. Already-tracked codec/tier duplication.** `CSP-083` (declared
  endpoint codec), `CSP-084` (relation-kind codec), `CSP-085` (resolver
  provenance tiers) remain valid and are not repeated here.

## B. Test Scaffolding Hygiene

- **B1. The `is_cargo_test_process` production hack (×3).**
  `discovery/harness/{aider,claude_code,codex}.rs` each carry a
  `#[cfg(not(test))] file_modified_epoch` that *sniffs argv for
  `/target/debug/deps/`* to detect "running under an integration test" and
  then fabricates mtime `1_700_000_000` for temp-dir paths, plus a
  `#[cfg(test)]` twin. This is a test-environment backdoor compiled into
  release binaries (any binary whose argv matches the pattern gets fake
  timestamps), duplicated three times. Replace with an injected mtime
  source on the adapter (defaulting to real `fs::metadata`), or set real
  mtimes on fixture files (`filetime` crate or `File::set_modified`);
  integration tests then need no production-side cooperation. Highest
  hygiene priority in this audit.
- **B2. Fixture-literal boilerplate.** 83 `AgentSessionNode { … }` and 53
  `MuxSessionNode { … }` struct literals, most in tests, each spelling out
  five-plus `None` fields. Every model-field addition (e.g. the recent
  `session_kind`) touches dozens of sites. Add `#[cfg(test)]`-visible
  builders (or `new(id)` constructors + `with_*` setters mirroring
  `RepoNode::new` / `CheckoutNode::new`, which already exist) and migrate
  incrementally. `tests/support/fixtures.rs` and
  `discovery/harness/fixtures.rs` are already good shared-fixture homes.
- **B3. In-file test volume.** ~45k test lines inside `src/` files; the
  extremes invert the ratio entirely — `output/table.rs` is 147 production
  lines under 2,678 test lines; `tui/ui.rs` 3.2k/3.4k; `rows/sessions.rs`
  1.3k/3.0k. Coverage is an asset; the *placement* hurts navigation. Move
  each large `mod tests` to a sibling file (`#[cfg(test)] mod tests;` with
  `foo/tests.rs` or `#[path]`) — purely mechanical, zero behavior change,
  and it makes the production surface of a file visible at a glance.
- **B4. Stale metrics in backlog stories.** `CSP-088` says `cli.rs` is
  961 lines (now 5,982); `CSP-087` says `declared.rs` is 1,338 (now
  1,370). Refresh when filing new work so sizing is honest.

## C. Rust-Specific Hygiene

- **C1. No `[lints]` table.** The crate relies on default clippy (clean)
  with `-D warnings` in `just check`, but has no curated stricter set. The
  pedantic+nursery inventory shows where signal hides among the noise —
  worth adopting selectively via `[lints.clippy]` in `Cargo.toml` rather
  than wholesale:
  - `redundant_clone` (63 hits; top files: `tui/explorer.rs` 9,
    `model/mod.rs` 8, `tui/app.rs` 5, `output/table.rs` 5, `cli.rs` 5) —
    real allocations, mechanical fixes.
  - `match_same_arms` (36; `runtime.rs` 6, `widgets/pins.rs` 5,
    `explorer.rs` 5, `detail.rs` 5, `ui.rs` 4) — often collapsible arms,
    occasionally a hint that two cases should share a helper.
  - `needless_pass_by_value` (60; `runtime.rs` 11, `widgets/pins.rs` 5,
    `server/mod.rs` 4) — clones forced on callers.
  - `wildcard match will also match future variants` (29) — these `_` arms
    silently swallow new `NodeKind`/`RelationKind`/action variants; several
    should be exhaustive so the compiler flags new-variant work. This one
    has correctness value for exactly the extension work planned in
    `H-EXT-*`.
  - `needless_collect` (12), `map_unwrap_or` (58 combined), `option_if_let_else`
    (48), `uninlined_format_args` (19) — cheap cleanups; adopt-and-fix or
    ignore consciously.
  - Cast-truncation warnings (~60, mostly `usize`→`u16`/`i32` in TUI layout
    math) — benign in context; consider `#[allow]` at the layout modules
    with a comment rather than crate-wide.
  - Doc lints (`missing # Errors`, long first paragraph, missing backticks,
    ~300 combined) — skip; the doc culture here is already unusually good
    and retrofitting `# Errors` sections is low-value.
- **C2. No snapshot index — linear scans everywhere.** `GraphSnapshot`
  stores `Vec<GraphNode>` / `Vec<GraphLink>`; production code does
  `nodes.iter().find(...)` at 35 sites and `candidate_links` scans at 25,
  and each view rebuilds ad-hoc `HashMap`s (A1) per render. At current
  graph sizes this is fine; at "47+ sessions per cwd" densities the
  per-keystroke TUI rebuild cost grows quadratically. Introduce a
  `SnapshotIndex` built once per snapshot publish (id→node,
  source-id→links-by-relation, session→preferred-mux, session→candidate
  counts) and pass it to row builders and detail/explorer. This also
  *deletes* the A1 duplication rather than merely centralizing it.
- **C3. Keybinding dispersion + help drift.** Key dispatch is hand-matched
  (`KeyCode::` ×128 in `runtime.rs`, ×137 in `widgets/pins.rs`, plus each
  overlay widget), an `Action` enum exists only inside `runtime.rs`, a
  202-line `remap_for_focus` re-maps actions per focus, and the help
  overlay's `keymap_sections()` (`widgets/help.rs:171`) is a hand-written
  parallel list of the same bindings. Nothing forces the help text, the
  dispatcher, and the overlays to agree. Introduce one declarative binding
  table — `(mode/focus, key, Action, help text)` — consumed by the
  dispatcher, `remap_for_focus` (becomes data), the help overlay, and the
  controls/pins overlays' hint footers. This also serves the project's
  menu-first discoverability preference: overlays render from the same
  table the dispatcher uses, so they cannot drift.
- **C4. Stringly-typed metadata** (`SourceMetadata.fields`, evidence
  strings like `codex_log_process_thread_match` scored at
  `resolve/mod.rs:741`) — already tracked as `CSP-090` and folded into
  the `CSP-476` scope; not repeated here.

## D. Monoliths And One-Off Logic Worth Splitting

Production-line counts (tests excluded):

| File | Prod lines | Note |
| --- | --- | --- |
| `src/tui/ui.rs` | ~3,245 | all views' render fns in one file; dispatch is centralized (3 `match view` sites) so per-view extraction is clean |
| `src/cli.rs` | ~2,760 | `CSP-088` already tracks the split; scope is stale (says 961 lines) |
| `src/tui/widgets/pins.rs` | ~2,757 | one widget owning menu model + per-action forms + rect math |
| `src/tui/app.rs` | ~2,676 | state + cursor reseating + per-view state maps |
| `src/tui/runtime.rs` | ~2,640 | dual event loops (A7), key dispatch (C3), rename/pins action plumbing |
| `src/tui/explorer.rs` | ~1,931 | row-tree assembly; `finalize_group` and friends are dense but cohesive |

Recommendations:

- **D1.** Split `ui.rs` by view (`ui/sessions.rs`, `ui/detail_panel.rs`,
  `ui/status_bar.rs`, …). Mechanical; the `--snapshot` harness (ADR 0067)
  makes regressions cheap to catch.
- **D2.** Split `runtime.rs` into loop driver / key dispatch / action
  handlers once A7 and C3 land (they remove most of its bulk).
- **D3.** `widgets/pins.rs`: separate the pins menu *model* (actions,
  selection-aware defaults) from the form rendering; the model is
  unit-testable without ratatui types.
- **D4.** `output/table.rs` is now a re-export shim over `output/render.rs`
  whose own comment says "new code should prefer `output::render::*`
  directly." Finish that migration and delete the shim (or shrink it to
  the two genuinely table-specific fns).
- **D5.** `dev_scenarios.rs` (1,194 lines) plus the `dev scenario` CLI tree
  compile into release binaries unconditionally. Either gate behind a
  `dev-scenarios` feature (the `snapshot` feature, ADR 0067, is the
  precedent and is already on in dev shell/CI) or record the decision that
  shipping them is intentional.
- **D6.** Naming nits for a future tidy: `src/viewer/` (transcript viewer
  core) vs `src/tui/viewer.rs` + `src/tui/viewer_bridge.rs` (TUI glue) is
  a recurring confusion; `src/hook.rs` (hook *writer*) is far from
  `src/discovery/hook_sidecar.rs` (hook *reader*). Rename/move only when
  touching these anyway.

## E. What Not To Change

Explicitly defended, so cleanup enthusiasm doesn't erase good decisions:

- The **provenance-first resolver** and evidence-preserving link model —
  verbose but load-bearing (ADRs 0002/0006).
- The **runner seams** (`SystemX`/`FakeX`) — the offline-test story depends
  on them; CSP-480 will unify, not remove.
- The **id-type macro + node constructors** in `model/mod.rs` — right
  amount of abstraction already.
- The **doc-comment culture** (module headers citing ADRs and story ids) —
  unusually good; do not let doc lints bully it into churn.
- The **test volume** itself — 52% of the repo being tests is an asset;
  reorganize (B3), don't reduce.
- **Six real panics / 19 unwraps** in production is already excellent;
  no error-handling campaign is needed.

## Suggested Chunking

Filed in `docs/backlog.md` § Code Hygiene And Simplification as
`CSP-462` through `CSP-472`, matching HYG-1 … HYG-11 below in order.
The `H-HYG` entries are the tracking source of truth.

Quick wins first (each independently landable, no behavior change):

1. **HYG-1 (S):** dedupe `snapshot_fragment` (A3), `current_epoch` (A4),
   `path_string` (A5) into shared helpers.
2. **HYG-2 (S):** extract shared TUI row helpers (A2) + shared
   candidate-count helper (A1 interim home in `tui/rows/mod.rs` +
   `output/mod.rs`; superseded by HYG-6's index).
3. **HYG-3 (S):** single parameterized `centered_rect` (A6).
4. **HYG-4 (M):** kill `is_cargo_test_process` / fake-mtime backdoor (B1)
   via injected mtime source or real fixture mtimes.
5. **HYG-5 (M):** adopt `[lints.clippy]` with the curated set (C1) and fix
   the fallout (`redundant_clone`, `match_same_arms`,
   `needless_pass_by_value`, exhaustive-match on model enums).

Structural (bigger, still mechanical):

6. **HYG-6 (M):** `SnapshotIndex` + migrate row builders/detail/explorer
   linear scans (C2); deletes the A1 copies for good.
7. **HYG-7 (M):** declarative keybinding table feeding dispatcher +
   overlays + help (C3); absorbs `remap_for_focus`.
8. **HYG-8 (M):** unify the dual event loops (A7), then split `runtime.rs`
   (D2).
9. **HYG-9 (M):** split `ui.rs` by view (D1); split `widgets/pins.rs`
   model/render (D3).
10. **HYG-10 (S):** finish the `output::render` migration, delete the
    `table.rs` shim (D4); feature-gate or bless `dev_scenarios` (D5).
11. **HYG-11 (M, rolling):** test builders for node literals (B2) and
    sibling-file `mod tests` extraction for the worst offenders (B3);
    fine to do opportunistically per file touched.

Sequencing note: HYG-1/2/3 before the `H-EXT-*` extensibility work pays off
immediately (new views/providers would otherwise copy the copies); HYG-5's
exhaustive-match adoption is directly synergistic with `H-EXT-*` since new
enum variants then fail loudly at compile time; HYG-6 and HYG-7 are the two
chunks that remove whole classes of future duplication rather than instances.
