---
id: CSP-904
title: A forge failure that has since recovered keeps showing in the forge chip
status: Done
assignee:
  - '@claude'
created_date: '2026-11-24 01:36'
updated_date: '2026-11-24 01:50'
labels:
  - forge
  - diagnostics
milestone: m-11
dependencies: []
priority: medium
type: bug
ordinal: 980000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI header's forge chip kept showing `gh pr list --repo git.drwholdings.com/btatnall/dek failed: HTTP 405 ...` for days after the server recovered. The failure itself was real (three forge refreshes during a `git.drwholdings.com` GraphQL blip, 405 once and 503 twice, for 14 remotes), but the `ProviderUnavailable` diagnostics it produced are never retired. Found on 2026-11-23: the daemon's snapshot held 42 of them, 14 exact duplicates; `gh pr list` worked by hand for every one of those remotes, and a cold in-process discovery with the same environment reported none.

Why they stick: `GraphSnapshot::evict_provider` (`src/model/graph.rs`) drops a provider's nodes, links, and provenance but documents that diagnostics "survive intact". The warm path then merges `GraphFragment::from(prior)` with the fresh fragment (`src/discovery/mod.rs`, `merge_fragments`), which appends diagnostics without dedupe or replacement, and the resolver keeps discovery diagnostics. `graph.bin` persists the result, so restarting the daemon or the TUI does not clear it; only a cold rebuild (`conspectus refresh`, empty prior) does.

Visible effects: the chip reports one arbitrary-looking repo (the first `github` diagnostic in sorted order, `forge_unavailable_reason` in `src/tui/runtime.rs`), and `gone_records` in `src/tui/rows/history.rs` treats the forge as unreadable, so the History view never marks a PR gone while a stale diagnostic lingers.

Out of scope: changing what the chip says when several remotes really fail (it still names only the first).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A forge refresh in which every query succeeds leaves the snapshot with no `ProviderUnavailable` diagnostic for `github`, whatever earlier refreshes reported, without a cold rebuild
- [x] #2 Repeated failed refreshes report only the current failures: the snapshot never holds the same forge diagnostic twice or one from an earlier refresh
- [x] #3 A stale forge diagnostic already stored in `graph.bin` clears on the first forge refresh after the fix, with no manual `conspectus refresh`
- [x] #4 A refresh that still fails keeps its diagnostics, so the forge chip still names a real, current failure
- [x] #5 Evicting one provider leaves other diagnostics alone: `Config`, `LegacyPins`, `Conflict`, `UnresolvedEndpoint`, and another provider `ProviderUnavailable`
- [x] #6 Tests cover recovery after failure, repeated failure without duplicates, a stale diagnostic loaded from a prior snapshot, and the History view marking a PR gone once the forge reads again
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: add `GraphSnapshot::clear_provider_diagnostics(provider)`, which drops the `ProviderUnavailable` diagnostics a provider reported, and call it from `evict_provider` so a provider's diagnostics go with its slice. Fix the `evict_provider` doc comment that says diagnostics survive intact. Every other diagnostic kind, and another provider's `ProviderUnavailable`, stays.
2. Daemon path: no code change needed beyond 1, since `try_class_cycle` already evicts every provider of the class and `forge_tick` runs a Forge cycle after any fetch. Cover it with a server test.
3. One-shot warm path (`discover_local_warm_with`): the freshness gate names `github` for eviction only when the prior holds a node or link stamped with it, which a run where every query failed never produced. So when `github` is not fresh, clear its diagnostics before the forge pass runs (or is skipped because the forge is off).
4. Tests: (a) model: eviction drops only the evicted provider's `ProviderUnavailable` and keeps the rest; (b) discovery: failure then recovery over a prior with no `github` node, repeated failure leaving one diagnostic, a stale diagnostic in a hand-built prior clearing, and a still-failing run keeping its diagnostic; (c) server: `SlowGh` gains a failing switch; two failing forge refreshes leave one diagnostic per failure, a recovering one leaves none, and a stale diagnostic seeded as if from `graph.bin` clears on the forge refresh; (d) History: once the stale diagnostic is evicted, the gone PR is listed.
5. Check each new test fails without the fix (stash the source change, run, restore). Then run `just check`, and `git diff --check`.
6. Docs: look for a statement in `docs/operations.md` or `docs/design.md` about forge diagnostics persisting or clearing, and correct it if there is one. No ADR (operator decision).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Two changes were needed, and each was shown necessary by disabling it alone. (1) `evict_provider` now also drops the `ProviderUnavailable` diagnostics the evicted provider reported, via the new `GraphSnapshot::clear_provider_diagnostics`; that alone fixes the daemon path, where `try_class_cycle` evicts every provider of the class. (2) The one-shot warm path (`discover_local_warm_with`) clears `github`'s diagnostics whenever the forge slice is not fresh: the freshness gate names a provider only if the prior holds a node or link stamped with it, which a run where every query failed never produced, so eviction never fired there. With only (1), five discovery tests fail; with neither, 11 of 12 new tests fail (the twelfth is a guard that a fresh slice keeps its report).
Scope kept narrow: only `ProviderUnavailable` is provider-scoped. `Config`, `LegacyPins`, `Conflict`, and `UnresolvedEndpoint` are untouched. Unverified: `Config` diagnostics pushed by `observed_cwd_git_fragment` and the atelier and pin loaders may leak the same way across warm merges.

Validation: `just check` passed (5977 tests, 1 skipped; fmt, clippy, rustdoc with -D warnings, `git diff --check`; `tuikit-features` skipped by the recipe because nothing under `crates/tuikit` changed). `cargo test --workspace` (CI's second run) was not run locally; the new tests share no process state.
Live check, 2026-11-23: a throwaway `conspectus serve` (private runtime, state, and data dirs, seeded with a copy of the operator's `graph.bin` and history) warm-started and did its first forge poll. The installed binary ended with 42 `provider_unavailable` diagnostics, 14 of them exact duplicates (`HTTP 405` x14, `HTTP 503` x28), the shape of the operator's live daemon, though live `gh` calls were succeeding (a cold in-process rebuild moments before reported none). The fixed build ended with 0, same input. The first version of that check read "0 before" from a client-side cold rebuild, not from the daemon, and was redone to wait for the daemon's forge scheduler. The operator's own daemon was not touched or restarted.
Not changed: the forge chip still names only the first failing remote when several really fail. `Config` diagnostics from `observed_cwd_git_fragment` and the atelier and pin loaders may leak across warm merges the same way; not verified, not in scope.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Stale forge failures no longer outlive the failure. `GraphSnapshot::evict_provider` now drops the `ProviderUnavailable` diagnostics the evicted provider reported (new `clear_provider_diagnostics`), which fixes the daemon's class cycles; and `discover_local_warm_with` clears `github`'s diagnostics whenever the forge slice is not fresh, because the freshness gate never names `github` for eviction after a run where every query failed. Other diagnostic kinds and other providers' reports are untouched, and a fresh slice keeps its own report. CHANGELOG entry added; no ADR, per the operator.
Verified with 12 new tests (model, warm discovery, daemon refresh and poll, History view): 11 fail with the fix disabled and the twelfth is a guard that a fresh slice keeps its report; disabling only the warm-path call fails 5 discovery tests, so both changes are needed. `just check` passes. A throwaway daemon warm-started from a copy of the operator's stale `graph.bin` went from 42 diagnostics (installed binary) to 0 (fixed build) after its first forge poll. Work is on branch `fix/csp-904-stale-forge-diagnostics`, uncommitted.
<!-- SECTION:FINAL_SUMMARY:END -->
