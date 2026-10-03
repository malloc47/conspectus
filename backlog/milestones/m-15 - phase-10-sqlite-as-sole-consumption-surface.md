---
id: m-15
title: "Phase 10: SQLite As Sole Consumption Surface"
---

## Description

Gated on ADR 0043. Phase goal: collapse the dual read path (in-memory
`SnapshotIndex` + SQLite) into one. All view, render, and inspection
code reads from a `rusqlite::Connection`; `GraphSnapshot` is demoted
to a producer-side intermediate scoped to the discovery → resolver →
loader pipeline.

The spike on `phase-9-sqlite-spike` proved out the shape:
`src/query/reader.rs` rounds-trips the loader fixtures losslessly,
and `src/output/agent_sqlite.rs` reimplements 9 of 17 agent-projection
cells against `v_sessions_with_repo` + a `LEFT JOIN`, reusing the
existing `RenderOptions` / `render_rows` substrate unchanged. The
spike code is a reference, not the production landing — Phase 10
stories formalize it.

Dependency shape inside the phase:

```
ADR 0043 ──→ CSP-279 (round-trip + read exhaustiveness) ──┬──→ CSP-280 (structured-id columns)
                                                          │
                                                          └──→ CSP-281 (shared render substrate)

CSP-280 + CSP-281 ──→ CSP-282 (agent) ──┬──→ CSP-283..287 (per-renderer migration, parallel after agent)
                                        │
                                        └──→ CSP-288..289 (TUI migration, parallel after agent)

(all migrations) ──→ CSP-291 (retire indexes) ──→ CSP-292 (demote GraphSnapshot)
```

The Phase 8 ADR 0031 TUI views (Mux/Union/Prs/Forks) are independent;
this phase migrates whichever ones exist when each story lands.

- **CSP-279** Promote reader + add compile-time read exhaustiveness

- **CSP-280** JSON-encoded NodeId foreign references (ADR 0044)

- **CSP-281** Extract the shared rendering substrate

- **CSP-282** Migrate the CLI agent projection to SQLite

- **CSP-283** Migrate the CLI mux projection to SQLite

- **CSP-284** Migrate the CLI union projection to SQLite

- **CSP-285** Migrate the CLI PRs projection to SQLite

- **CSP-286** Migrate the CLI forks projection to SQLite

- **CSP-287** Migrate `node show` to SQLite

- **CSP-288** Migrate the TUI detail pane to SQLite

- **CSP-289** Migrate the TUI sessions row builder to SQLite

- **CSP-290** Decide whether TUI Mux/Union/Prs/Forks builders need SQLite-specific migration work

- **CSP-291** Retire `SnapshotIndex`, `SnapshotView`, `SessionsIndex`

- **CSP-292** Demote `GraphSnapshot` to producer-only

- **CSP-294** Retire snapshot bridge APIs from consumer modules

- **CSP-401** Audit GraphSnapshot round-trip and force coverage on future fields
