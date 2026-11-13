---
id: CSP-818
title: Fix the hook attribution spool defects the live-state research found
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 12:52'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/research/06-internal-audit.md
priority: medium
type: bug
ordinal: 904000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The research found defects in the attribution path that live state does not need fixed to work, but that are wrong on their own: the spool key ignores the pane so split-pane agents overwrite each other's attribution, `ACTIVE_TTL_SECONDS` is dead code while the docs say stale records are ignored, an ingest leaves two active links for one pane until the next cycle, and the traced capability test does not cover `hook write`.

Scope: Key the spool by pane (falling back to pid, then cwd); apply the stated TTL or correct the docs and delete the constant, whichever the code supports after a decision recorded in an ADR note; supersede older hook links for the same pane during ingest; update ADR 0028 and `docs/operations.md`, which still describe `hooks.sqlite3` and a TTL; add the omitted `hook_ingest` to the socket command list in `docs/design.md`. The per-pane unit test must go through the spool rather than the legacy JSON reader.

TUI changes: none expected; attribution of split-pane sessions becomes correct where it was wrong. List any change in the final summary.

Tests and checks: Spool tests with two panes in one tmux session, ingest followed by cycle, TTL behavior, and the existing hook-sidecar suites.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Two agents in split panes of one tmux session keep separate attribution records
- [x] #2 The TTL claim in the docs matches the code
- [x] #3 An ingest never leaves two active hook links for one pane
- [x] #4 ADR 0028, `docs/operations.md`, and `docs/design.md` match the code
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Spool slot per pane (src/hook.rs): derive each record's slot from the record (tmux socket + session + pane; a pane-less mux record keeps one slot per mux session, as the attribution pass dedupes it; outside a mux: pid, then cwd, then session). Readers re-slot every stored record newest-wins, so records under the old per-session key migrate on read and are rewritten under their pane key by the next accepted write. SCHEMA_VERSION stays 1, so an older binary reads the spool and adds its record without dropping any; a spool with a newer schema version is never overwritten. Bound the spool at 256 records (oldest dropped), since pane keys lose the per-session bound.
2. TTL: delete the dead ACTIVE_TTL_SECONDS; record in an ADR 0146 amendment that no time gate applies (CSP-249) and what makes a record stale; correct ADR 0028, ADR 0029's reference, and docs/operations.md.
3. Ingest supersede (src/discovery/hook_sidecar.rs): seed the per-pane winners from the graph's Active hook links, so an ingested record that wins its pane overrides the older link and drops its process observation; an older one is overridden by the graph's winner. Cycles are unchanged (the pass is evicted first).
4. Docs: docs/design.md socket list gains hook_ingest and the spool line says per pane; operations.md; plan README section 8; CHANGELOG Fixed entry.
5. Tests: store-level pane slots, old-key compat (old-key spool read, old binary write emulation, newer-schema untouched), bound; per-pane sidecar test through the spool; ingest supersede unit tests; server ingest double-link and ingest-then-cycle tests. Then checks.sh, run-tests.sh, frames before/after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions:
- TTL: no time gate (CSP-249 removed it on purpose); ACTIVE_TTL_SECONDS deleted; ADR 0146 amendment item 5 defines stale as pid gone / mux newer than the record / active pane runs another harness. ADR 0028, ADR 0029 (cited the constant), operations.md, design.md corrected.
- Slot key derived from the record: tmux:{socket}:{session}:pane:{pane}; pane-less mux record keeps tmux:{socket}:{session}; outside a mux pid:{pid}, then cwd:{cwd}, then session:{harness}:{session}. Readers re-slot stored records newest-wins (tie: the record already under its slot), so old per-session keys migrate on read and are rewritten by the next accepted write. SCHEMA_VERSION stays 1; a spool with a higher version reads as empty and refuses writes.
- Bound: 256 records, oldest observed dropped (pane slots have no natural bound). Considered pruning dead pids instead; it cannot bound pid-less records (all records on hosts without /proc).
- Ingest: apply_hook_records seeds per-pane winners from the graph's Active hook links; a winning ingested record overrides the older link and removes its RuntimeProcess node and links; an older one is overridden. Rejected replaying the whole spool on ingest: evicting the pass drops live-state bindings of hook-synthesized sessions until the next cycle.
- The description's traced-capability gap was already closed: tests/capabilities.rs traces hook write (added with CSP-810).
Validation: run-tests.sh 5362 passed, 0 failed; checks.sh pass; 9 tui --snapshot frames byte-identical to the base build. Follow-up CSP-852 (non-active pane gated against the active pane's harness).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Agents in split panes of one tmux session now keep separate attribution records, and a daemon ingest never leaves two Active hook links for one pane.

- Spool (`src/hook.rs`): each record's slot is derived from the record, `tmux:{socket}:{session}:pane:{pane}` (pane-less mux records keep one slot per session; outside a mux the pid, then the cwd). Readers re-slot every stored record newest-wins, so a spool under the old per-session key reads correctly and the next accepted write stores pane keys. `SCHEMA_VERSION` stays 1, so an older binary adds its record without dropping any; a spool with a newer schema version reads as empty and is never overwritten. The spool keeps at most 256 records (oldest dropped).
- TTL: the dead `ACTIVE_TTL_SECONDS` is deleted rather than applied, since CSP-249 removed the 15-minute gate on purpose. The ADR 0146 amendment records this and defines a stale record (pid gone, mux newer than the record, active pane runs another harness).
- Ingest (`src/discovery/hook_sidecar.rs`): the pass seeds its per-pane winners from the graph's Active hook links. A fresher ingested record overrides the older link and drops its process observation; an older one is overridden. Cycles are unchanged.
- Docs: ADR 0146 amendment, ADR 0028 corrected (SQLite, TTL) with an amendment, ADR 0029 no longer cites the constant, index marks both amended, `docs/design.md` lists `hook_ingest` and per-pane records, `docs/operations.md` says what stops a record counting, plan section 8, CHANGELOG.
- The traced capability test already covered `hook write` (CSP-810), so nothing was needed there.

Verified: new tests in `src/hook_tests.rs` cover pane slots, pid/cwd fallback, an old-key spool, an old-key and pane-key record for one pane, an older binary's write, a newer-schema spool, a corrupt spool, and the bound. `hook_sidecar_tests.rs` covers the per-pane test through the spool, ingest supersede, ingest matching a cycle, late arrivals, re-ingest, and a graph that already held a double link. `server_tests.rs` covers ingest then refresh, one pane and two panes. `run-tests.sh`: 5362 passed, 0 failed. `checks.sh` passes. The nine `tui --snapshot` frames are byte-identical: the showcase world has one hook record, in one pane. nextest and `just tuikit-features` were not rerun because tuikit is untouched.

Follow-up: CSP-852 (a non-active pane's record is gated against the active pane's harness).
<!-- SECTION:FINAL_SUMMARY:END -->
