---
id: m-12
title: "Phase 7: Continuous Operation And Snapshot Persistence"
---

## Description

Source plan: pending; this section is the workstream skeleton. See
`docs/design.md` sections "Continuous Operation Mode" and "Graph
Snapshot Persistence" for the high-level model. Phase goal: take
Conspectus from a pure one-shot CLI to a tool that can persist its
graph between invocations and optionally maintain it live in a
long-running server.

Dependency shape inside the phase:

```
Phase 9 ADR-A (engine selection) ────→ CSP-137 (snapshot ADR) ──┐
                                                                ├──→ CSP-139 (warm-start save/load) ─┐
CSP-138 (provenance/freshness model) ───────────────────────────┤──→ CSP-141 (partial eviction) ─────┤
                                                                │                                    │
CSP-137 ──→ CSP-140 (server ADR) ───────────────────────────────┴────────────────────────────────────┴──→ CSP-142 (serve) ──→ CSP-143 (CLI ↔ server)
                                                                                                                         ├──→ CSP-144 (status/inspection)
                                                                                                                         └──→ CSP-145 (event-driven, stretch)
```

Under the Stage 3 SQLite pivot (Phase 9), `CSP-137` (persistence
ADR) and `CSP-140` (server transport ADR) are no longer parallel:
the engine selection ADR (Phase 9 ADR-A) settles the storage
choice; `CSP-137` then absorbs the SQLite persistence model; and
`CSP-140` builds on the persistence shape to settle the WAL-based
read path plus Unix-socket write path. `CSP-138` is still
foundational and should land before any persistence or eviction
code.

- **CSP-137** ADR: graph snapshot persistence format and lifecycle

- **CSP-138** Add provider provenance and freshness metadata to graph nodes and candidate links

- **CSP-139** Implement snapshot save/load for the one-shot CLI

- **CSP-140** ADR: continuous server mode architecture and transport

- **CSP-141** Implement partial graph eviction at provider granularity

- **CSP-142** Implement `conspectus serve`

- **CSP-143** Implement CLI ↔ server snapshot read path

- **CSP-144** Add server status and inspection subcommands

- **CSP-145** Event-driven refresh via filesystem watchers (stretch)
