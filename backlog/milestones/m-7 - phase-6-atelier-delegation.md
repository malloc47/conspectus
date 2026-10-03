---
id: m-7
title: "Phase 6: Atelier Delegation"
---

## Description

Source plan: `docs/implementation/phase-06-atelier-delegation.md`.
Design framing: the "Migration Plan" section in `docs/design.md`.

Conspectus has stabilized its graph, discovery, JSON, table, and
declared-link surfaces (Phases 1–5). Phase 6 turns that surface into
something Atelier (and any other future consumer) can rely on, and
coordinates the Atelier-side deprecation/delegation work.

The Conspectus crate has been library-first since ADR 0007, so this
phase is mostly about stabilizing the *contract* (what's stable,
where it lives, how to depend on it), refreshing user-facing docs to
position Conspectus as the cross-workspace observability surface, and
filing the cross-repo work in Atelier. No Atelier-side code lands in
this repo.

- **CSP-073** Record an ADR for the Conspectus library API surface

- **CSP-074** Record an ADR for Conspectus distribution

- **CSP-075** Audit pure vs impure modules and produce a library API inventory

- **CSP-076** Add a curated public re-export facade

- **CSP-077** Write the Atelier migration guide

- **CSP-078** Add a representative comparison fixture

- **CSP-079** File the Atelier-side delegation work in the Atelier repo

- **CSP-080** Refresh top-level docs to position Conspectus as the cross-workspace observability surface

- **CSP-081** Decide whether to extract Conspectus into its own repository

- **CSP-082** Verify the Phase 6 end state
