---
id: m-14
title: "Phase 9: Embedded Query Engine"
---

## Description

Source plan: `plans/stage-3-sqlite-query-engine.md` (the activation of
ADR 0035 stage 3). Phase goal: deliver `conspectus query <sql>` — a
user-facing SQL surface over the resolved graph — backed by an
embedded SQLite engine. The phase introduced a typed SQL schema
mirroring the Rust model, a loader, a query runner, a small library of
saved views, and a fixture-driven regression suite. The future
file-backed `graph.sqlite` warm-start lifecycle is tracked under
Phase 7; the current one-shot CLI can materialize a resolved snapshot
into an in-memory SQLite database when no persisted graph exists.

This phase was gated on an ADR cluster (engine selection, persistence
model, server transport, library API gating, distribution amendment,
resolver-stays-in-Rust, and vector search). The engine, query feature,
distribution amendment, resolver-boundary, vector-search, and
consumer-surface ADRs have landed. The persistence and transport
pieces remain the SQLite-aware re-scopes of `CSP-137` and `CSP-140`.

Dependency shape inside the phase:

```
ADR cluster (engine/persistence/transport/lib-api/dist/resolver) ────┐
                                                                     │
CSP-271 (spike) ──→ CSP-272 (schema) ──┬──→ CSP-273 (loader) ──→ CSP-274 (query MVP) ──┬──→ CSP-276 (saved views)
                                       │                                               ├──→ CSP-275 (result fmts)
                                       │                                               └──→ CSP-277 (fixture corpus)
                                       │
                                       └──→ CSP-277 in parallel after CSP-272

CSP-278 (vector search via sqlite-vec) ───→ CSP-293 (embedding import)
```

The TUI workstream (Phase 8 open stories) began independently, but
Phase 10 later made SQLite the sole consumer-side read surface. The
remaining producer-side `GraphSnapshot` shape stays scoped to
discovery, resolution, JSON dump, and fixture setup.

- **CSP-271** SQLite integration spike (bundled build, WAL, lifecycle)

- **CSP-272** Define the SQL schema for the resolved graph

- **CSP-273** Implement the GraphSnapshot → SQLite loader

- **CSP-274** Implement `conspectus query <sql>` MVP

- **CSP-275** Result formatters for query output

- **CSP-276** Saved views: a library of common queries

- **CSP-277** Fixture corpus and query regression suite

- **CSP-278** Vector search via `sqlite-vec` (deferred)

- **CSP-293** Add an embedding import command
