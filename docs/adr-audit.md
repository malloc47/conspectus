# ADR Corpus Audit

Status: draft audit, 2026-07-01. Third companion audit alongside
`docs/extensibility-assessment.md` (provider seams) and
`docs/code-hygiene-audit.md` (code-level hygiene). Scope: all 84 ADRs in
`docs/adr/`, checked for internal contradictions, stale acceptances against
the current codebase, and alignment with the product direction in
`docs/design.md`.

Method: status extraction for all 84, decision-section reads for all 84,
full reads of the pivotal arcs (identity/evidence core, the SQLite arc,
viewer arc, workspace-view arc, pins arc), and code verification of specific
claims (`SnapshotIndex` existence, opencode SQL columns, sqlite-vec
leftovers, config alias, release state).

## Overall Health

The corpus is in unusually good shape for its size:

- **Status hygiene is real.** The entire SQLite arc (0036, 0037, 0039, 0040,
  0042, 0044 superseded; 0038, 0043 partially superseded) is correctly
  cross-marked to ADR 0082. 0008→0009 likewise. Very few projects maintain
  this.
- **Retirements are clean.** Zero `sqlite-vec` / `vec0` references survive
  in `src/`; the `query` surface is gone end-to-end; the CHANGELOG records
  the breaking removal.
- **The "boring decision" culture is consistent and valuable.** 0059 (no
  rules engine), 0056 (no `arboard`), 0046 (no `sysinfo`), 0076 (no
  scrollbar crate), 0020 (no table crate for the CLI), 0025/0030/0051
  (minimal single-purpose deps) all follow the same tenet: prefer in-tree
  or stdlib until a measured need appears. This tenet is *aligned* and
  should be named explicitly somewhere citable.

The problems are: two accepted ADRs the codebase no longer satisfies, one
tenet stated more absolutely than the code honors, one guardrail the product
outgrew, an un-superseded UI arc, and a corpus-density problem where ~30
load-bearing decisions share a namespace with ~25 changelog-grade UI notes.

## Contradictions And Stale Acceptances

### C1. ADR 0035 Stage 1 is accepted but absent from the tree

ADR 0035 (graph-to-view query layer) commits to "Stage 1 (now): introduce an
in-Rust `SnapshotIndex` selector layer at `src/model/index.rs`, factor the
duplicated indexing out." No `SnapshotIndex` and no `src/model/index.rs`
exist today; consumers linear-scan `GraphSnapshot` (35 node-scan sites) and
re-derive ad-hoc maps per view — the exact duplication Stage 1 was accepted
to remove. The likely history: Stage 2/3 escalation went through the SQLite
arc (0036/0043), which made the index moot, and the 0082 retirement restored
direct snapshot iteration without restoring the index. The hygiene audit
independently re-derived this need as `H-HYG-006`.

**Recommendation:** amend ADR 0035 — Stage 1 re-lands via `H-HYG-006`
(cite it), Stage 2 (Ascent) and Stage 3 (`conspectus query`) gates are
superseded by ADR 0082's decision that typed-snapshot consumption is the
consumer surface. Without the amendment, 0035 reads as an unimplemented
commitment and its escalation gates point at a retired path.

### C2. The payload-privacy tenet is stated absolutely but honored selectively

ADR 0048 (Codex state/log readers) states the rule as a principle — readers
"never select privacy-sensitive payload columns" — and `docs/design.md`
repeats it as a general property of harness-DB reads. But the opencode
adapter's preview query (ADR 0013 + ADR 0023 lineage) selects
`json_extract(data, '$.text')` from opencode's message table — message
payload from a harness DB. Meanwhile `last_message_preview` (0023) and the
native viewer (0052) intentionally put transcript content on screen and into
the graph snapshot (which persists to `graph.bin` and serves over the
socket and JSON export).

This is not a product bug — previews and the viewer are core features — but
the *tenet as written* is contradicted, and the real rule is implicit.

**Recommendation:** one short amendment (to 0048 or a new tenet ADR) stating
the actual invariant: *attribution and identity readers never read payload;
operator-facing content features (preview, viewer) may read payload but only
normalized/truncated for display, and hook-sidecar/state readers stay
payload-free.* Then the codex rule, the opencode preview, and 0023's
normalization all sit under one stated principle instead of appearing to
conflict.

### C3. The workspace-view arc is not cross-superseded

ADR 0062 polishes the dedicated Workspaces view; ADR 0065 then deletes that
view entirely, replacing it with `SessionsGrouping::Workspace`. 0062 still
reads "Accepted" with no forward pointer, and 0063 (drop the workspace
cross-reference chip) partially describes code 0065 removed. Anyone reading
0061→0065 in order gets whiplash: grouping dropped from four views (0061),
view polished (0062), chip dropped (0063), grouping hybridized (0064), view
deleted (0065).

**Recommendation:** mark 0062 "Superseded by ADR 0065"; annotate 0063 as
absorbed. Cheap, and it makes the UI arc legible.

### C4. "Read-only first" has been outgrown without being restated

The guardrail (CLAUDE.md, design.md: "Start read-only unless a task
explicitly calls for persistence or link CRUD") predates a now-substantial
sanctioned mutation surface: lockstep tmux renames (0029), pin launch /
send-keys / new-session (0057), resume splicing (0058), TOML store writes
(0014/0029/0057). Each mutation was individually ADR'd, but the tenet text
still says "read-only," which makes every new mutation look like an
exception rather than part of a defined envelope.

**Recommendation:** a short tenet amendment defining the mutation envelope:
Conspectus may (a) write its own TOML/user-intent stores, (b) manage mux
lifecycle (create/rename/attach/send-keys) when operator-initiated, and
(c) never mutates harness-native state or injects terminal input into
agents (0028's line stays absolute). This is a *description of decisions
already made*, not a new decision.

### C5. Config schema sprawl with an expired deprecation promise

0021 established `[table.<rows>]`; 0031 established `[tui.views.<name>]`
and promised the legacy `[tui].sessions_grouping` alias survives "until a
follow-on ADR retires it" — that follow-on never happened and the alias
code path is still live (`config.rs:624`). Three generations of view/table
config keys coexist.

**Recommendation:** file the small retire-the-alias ADR (or explicitly
re-bless the alias as permanent). Consider whether `[table.<rows>]` and
`[tui.views.<name>]` should converge when `H-EXT` config work touches this
area anyway.

### C6. Distribution policy is aspirational and drives ceremony

0015 (library API surface), 0016 (crates.io-first distribution), 0017 (stay
in-repo) assume an external-consumer future. Reality: version 0.1.0, no tags
cut, no crates.io publish, the curated `api.rs` facade is 59 lines, and
`H-REF-010` already questions whether its re-exports are right. The tenets
aren't wrong — Atelier migration is a stated goal — but they currently buy
ceremony (facade curation, stable-surface discipline across `model`,
`output`, `resolve`, `config`, `declared`) without a consumer exercising the
contract.

**Recommendation:** keep 0015/0016/0017 but downgrade enforcement to "don't
break gratuitously" until the first real external consumer lands; fold the
facade question into `H-REF-010` rather than treating the current `api.rs`
as contractual.

## Tenets That Are Aligned (Keep And Defend)

- **Evidence-preserving candidates + resolver separation** (0002, 0006,
  0077, 0041). The single most load-bearing decision in the codebase. It
  survived the SQLite round-trip untouched — 0041 ("resolver stays in
  Rust") is the reason 0082's retirement was a storage swap rather than a
  rewrite. Ambiguity-as-data (0071, 0072, 0077) flows directly from it.
- **Provider-neutral graph, provider reality at the edges** (0001, 0003,
  0004, 0026). Verified in the extensibility audit: consumers dispatch on
  node fields, not provider names. This tenet is what makes the `H-EXT`
  plan tractable.
- **Conservative discovery** (0027 explicit-roots-only inference, 0028's
  refusal to inject terminal input, read-only DB opens in 0013/0048). These
  are trust decisions, not just technical ones; they define the product's
  safety character.
- **User intent in federated TOML; caches never authoritative** (0012,
  0014, 0029, 0057, 0058, 0083's cache-not-store framing, 0084's
  "TOML remains source of truth, graph node is derived"). Applied with
  striking consistency across four generations of features — this is the
  tenet the codebase honors best.
- **Injectable runner seams** (0011, and the tmux equivalent). The offline
  test story and the fixture culture depend on it.
- **Agent-oriented development loop** (0067 snapshot mode, 0068/0069
  fixture modes, 0070 showcase scenario, 0007's snapshot-test posture).
  Unusually forward-looking: the TUI is verifiable by an agent without a
  human screenshot in the loop. This is a differentiating engineering
  tenet; name it and keep investing.
- **Named escalation gates** (0035's Stage 2/3 pattern, 0010's "revisit if
  the justfile grows," 0056's re-evaluate-on-feedback). Even where the
  gates led somewhere that got retired (SQLite), having *named* triggers is
  why the corpus could unwind cleanly. Keep writing gates into decisions.

## Perfunctory Or Detrimental

- **The SQLite arc as a cautionary record (0036–0044).** Nine ADRs
  designed, built, and retired a persistence + query + vector-search stack
  inside one phase. Two tenet violations drove the cost: 0043 inverted the
  data-model-first tenet by making storage the consumer surface
  (`GraphSnapshot` "demoted to a producer-side intermediate"), and 0042
  shipped vector-search schema with no embedding pipeline — capability
  ahead of any user need, against the project's own "design for, don't
  implement until needed" rule. The unwind (0082/0083) was exemplary, but
  the corpus never states the lesson. **Recommendation:** add a
  consequences note to 0082 (or a one-page tenet doc) recording the two
  generalizable rules: *consumers read the typed model — storage is an
  implementation detail*, and *no capability lands before its first
  consumer.* The `H-EXT`/`H-HYG` plans should be held to the second rule
  explicitly.
- **Changelog-grade UI ADRs dilute the corpus.** 0034 (repo row path
  preference), 0061, 0062, 0063, 0071, 0072, 0074, 0075 are pixel-level
  decisions with file/line-specific scopes — several already describe
  deleted code (C3). They follow the letter of the "memorialize decisions"
  guardrail but bury the ~30 load-bearing ADRs under UI churn, and their
  maintenance (supersession marking) demonstrably lags. 0078 is the
  exception that proves the rule: it extracted the *rubric* from the churn
  and is genuinely reusable. **Recommendation:** adopt a two-tier
  convention — full ADRs for model/persistence/dependency/workflow
  decisions; a lighter `docs/design-notes/` (or a `Tier: UI` header with
  relaxed supersession expectations) for view polish. Migrating old ones is
  optional; stopping the dilution is the point.
- **Mux attribution has no single canonical description.** The rules now
  span 0006 (candidates), 0028 (hook sidecars), 0046 (process tree), 0047
  (process nodes), 0048 (log readers), 0071/0072 (ambiguity UX), 0077
  (slot preservation), plus evidence-string weights in the resolver. Each
  ADR is individually sound and the subsystem is core product value — this
  is *aligned complexity* — but onboarding requires reading eight documents
  and `cross_link.rs` (3.7k lines) to know what wins over what.
  **Recommendation:** one consolidating architecture note (no new
  decisions) that states the evidence hierarchy end-to-end and links the
  ADRs; `H-EXT-004`'s runtime-signature work is the natural moment to
  write it.
- **0047 (runtime process nodes) is the costliest active model decision.**
  First-class graph nodes for rebuildable process facts ripple into every
  consumer (default-hide filters in projections, rkyv schema, exports,
  detail rendering). The diagnostic value is real and the ADR anticipated
  it, but it's the one active decision where "evidence as attributes"
  (the alternative it rejected) would have been materially simpler. Not
  worth reversing now; worth naming as the complexity ceiling — decline
  future proposals to promote other operational facts (watcher events,
  scheduler state) to node kinds without 0047-level justification.
- **Dual table renderers are justified but should stay bounded.** 0020
  (hand-rolled width-aware CLI tables) and 0054 (comfy-table for Markdown
  tables inside transcripts) solve different problems and each ADR is
  sound, but the crate now carries two table layout systems. Fine as-is;
  flag only so a third table surface reuses one of the two.

## Recommended Actions

Small, mostly documentation-shaped; none block feature work. Disposition
as of 2026-07-01: the pure bookkeeping items were applied directly; the
items needing a real decision or real writing are filed in
`docs/backlog.md` § ADR And Tenet Alignment as `H-ADR-001` … `H-ADR-005`.

1. **ADR-A1 (done 2026-07-01):** amended ADR 0035 (Stage 1 → `H-HYG-006`;
   Stages 2/3 → superseded by 0082). Status-accuracy fix for C1.
2. **ADR-A2 (done 2026-07-01):** marked 0062 superseded by 0065; annotated
   0063. Fix for C3.
3. **ADR-A3 (filed as `H-ADR-001`):** privacy-tenet amendment stating the
   attribution-vs-content payload rule (C2).
4. **ADR-A4 (filed as `H-ADR-002`):** mutation-envelope amendment replacing
   the outgrown "read-only first" phrasing in design.md/CLAUDE.md
   guardrails (C4).
5. **ADR-A5 (filed as `H-ADR-003`):** retire (or permanently bless) the
   `[tui].sessions_grouping` legacy alias (C5).
6. **ADR-A6 (done 2026-07-01):** lessons addendum on 0082 recording the two
   SQLite-arc lessons (consumers read the typed model; no capability before
   its first consumer).
7. **DOC-A7 (filed as `H-ADR-004`):** consolidated mux-attribution
   architecture note linking 0006/0028/0046/0047/0048/0071/0072/0077;
   write alongside `H-EXT-004`.
8. **CONV-A8 (filed as `H-ADR-005`):** two-tier decision-record convention
   (full ADR vs UI design note) so the load-bearing corpus stays legible.

Alignment scorecard, one line each:

| Tenet | Verdict |
| --- | --- |
| Candidates + resolver separation (0002/0006/0041/0077) | Aligned — the architecture's spine |
| Provider-neutral graph (0001/0003/0004/0026) | Aligned — enables H-EXT |
| Federated TOML user intent; caches rebuildable (0012/0014/0029/0057/0083/0084) | Aligned — most consistently honored |
| Conservative, non-invasive discovery (0027/0028) | Aligned — product-defining |
| Agent-oriented dev loop (0067–0070) | Aligned — differentiating; invest more |
| Boring-dependency bias (0020/0025/0030/0046/0056/0059/0076) | Aligned — keep |
| Named escalation gates (0035 et al.) | Aligned as practice — but keep gate targets current (C1) |
| "Read-only first" as phrased | Outgrown — restate as mutation envelope (C4) |
| Payload privacy as phrased in 0048 | Overclaimed — restate precisely (C2) |
| Library/distribution posture (0015/0016/0017) | Aspirational — keep, downgrade enforcement until a consumer exists (C6) |
| Storage as consumer surface (0043, retired) | Detrimental — correctly reversed; record the lesson |
| Capability before consumer (0042, retired) | Detrimental — correctly reversed; record the lesson |
| UI micro-decisions as full ADRs (0034/0061–0063/0074/0075) | Perfunctory — tier them (CONV-A8) |
| Runtime process nodes (0047) | Aligned but at the complexity ceiling — hold the line |
