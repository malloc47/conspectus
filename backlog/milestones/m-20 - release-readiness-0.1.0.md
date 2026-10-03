---
id: m-20
title: "Release Readiness: 0.1.0"
---

## Description

Catalog assembled 2026-09-30 while rewriting `README.md` for a public
release and an upcoming talk. Baseline at the time: `cargo fmt`,
`cargo clippy -D warnings`, and 2,070 nextest tests all green; no tags;
GitHub repo private. Tiers: **P0** blocks going public or would embarrass
a live demo; **P1** should land with the 0.1.0 release; **P2** can follow
it. P0 is organized as a workstream with its own decisions, dependency
shape, and sub-stories. Work P1 and P2 top-to-bottom.

### P0 Workstream: Public Release And Talk Readiness (CSP-532..CSP-540)

Goal: make Conspectus safe to publish and ready to demo. The repository
flips from private to public with a license, accurate docs, and truthful
`--help`, and the talk demo runs on the showcase fixture without visible
defects. Sources: the 2026-09-30 README rewrite (which re-derived the
feature set from the binary) plus a sweep of every `--help`, the
user-facing docs, the TUI (rendered with `conspectus tui --snapshot`
against `tests/fixtures/showcase.json`), git history, and this backlog.

Scope bounds:

- Fix what is wrong or misleading; add no features. The ambiguous-mux
  picker (`CSP-175`), resume-into-mux (`CSP-170`), and release, packaging,
  and platform work (`CSP-541`..`CSP-544`) stay out.
- No new dependencies and no new checked-in workflow tools. A
  demo-recording tool enters the repo only through an ADR (`AGENTS.md`).
- Behavior changes are limited to the defects named below. Validate every
  TUI story with `conspectus tui --snapshot` (ADR 0067) against the
  showcase fixture; code changes pass `just check`, docs-only changes pass
  `git diff --check`.

Definition of done:

- `LICENSE` exists and the README links it; the vendored-asset `NOTICE`
  is accurate.
- `--help` for every subcommand is free of backlog IDs, rustdoc link
  syntax, and retired semantics, and a test keeps it that way.
- `README.md`, `docs/operations.md`, `docs/design.md`, and the guides
  describe only commands and config keys that exist.
- On the refreshed showcase fixture the TUI shows relative ages, accurate
  hints, a readable help overlay, visible related-row labels, and sane
  header counts.
- Backlog checkboxes match landed work, and `CHANGELOG.md` carries the
  0.1.0 content.
- Talk captures exist; the README hero frame and numbers are refreshed.
- `CSP-533.02` (flip to public) lands last on the publication track.

Decisions to collect in one sitting (recommendation in parentheses):

1. `CSP-532.01`: the copyright line (`Copyright (c) 2026 Jarrell Waggoner`).
2. `CSP-533.01`: personal home paths in five tracked files (accept them in
   ADR and backlog prose; normalize `examples/pantry.rs` and the test
   string to `/home/user`), and `Claude-Session:` URLs in four commit
   trailers (accept; don't rewrite `main`).
3. `CSP-535.02`: ADR numbers in `--help` (drop them from command summaries
   and point at docs; the dev-only snapshot/fixture flags may keep them).
4. `CSP-535.02`: `tui --view union|prs|forks` (hide the values from help to
   match `CSP-501`, but keep accepting them so scripts don't break).
5. `CSP-537`: `docs/feature-summary.md` (delete; the README supersedes it).
6. `CSP-539`: the Phase 11 notes in `CHANGELOG.md` (fold them into a short
   "Upgrading from development builds" subsection).
7. `CSP-540.02`: capture tooling (keep it operator-local; putting it in the
   repo needs an ADR).

Dependency shape inside the workstream:

```
CSP-532.01 ───┐
CSP-532.02 ───┤
CSP-533.01 ───┤
CSP-535.01 ───┤
CSP-535.02 ───┤
CSP-536.01 ───┼──→ CSP-533.02  (flip the repo public)
CSP-536.02 ───┤
CSP-536.03 ───┤
CSP-536.04 ───┤
CSP-537  ─────┤
CSP-539  ─────┘

CSP-534.01..05 ───────┐
CSP-540.01 ───┬───────┴──→ CSP-540.02  (captures) ────┐
              │                                       ├──→ CSP-540.04  (README refresh)
              └──→ CSP-540.03  (just demo)            │
CSP-538 ──────────────────────────────────────────────┘
```

Every story without an incoming arrow can start now and land in any order.
The publication track ends at `CSP-533.02`; the demo track ends at
`CSP-540.04`. `CSP-534` and `CSP-538` don't block going public, but landing
them first means the public repo opens with a clean demo and an accurate
backlog. Suggested order: collect the decisions, then `CSP-534`,
`CSP-535`, `CSP-536` and `CSP-537`, `CSP-538`, `CSP-539`, `CSP-540`, and
finally `CSP-533.02`.

- **CSP-532** License and third-party notices
- **CSP-533** Pre-publication review, then flip the repository public
- **CSP-534** Fix demo-visible TUI defects
- **CSP-535** True up CLI `--help`
- **CSP-536** True up the docs
- **CSP-537** Retire `docs/feature-summary.md`
- **CSP-538** True up backlog checkboxes
- **CSP-539** Write the 0.1.0 CHANGELOG entry
- **CSP-540** Talk and demo assets

### P1: With The 0.1.0 Release

- **CSP-541** Define the release process and cut `v0.1.0`
- **CSP-542** Add installation paths beyond `--path`
- **CSP-543** Settle platform posture and CI shape
- **CSP-544** Clean up dependency and tooling leftovers
- **CSP-545** Make the on-disk footprint consistent with ADR 0087
- **CSP-546** Consolidate harness-hook docs
- **CSP-547** Complete the documentation index and add an ADR index
- **CSP-548** Decide how public docs refer to Atelier

### P2: After 0.1.0

- **CSP-549** Fix or document known functional gaps
- **CSP-550** Close the ADR-alignment items that shape the public story
- **CSP-551** Add contributor onboarding
- **CSP-552** Decide the backlog's long-term shape
- **CSP-553** Decide the library API posture
