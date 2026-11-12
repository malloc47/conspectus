# Live State In The TUI: Options And Recommended Defaults

Status: waiting for the operator. Phase 1 puts live state on the row models
(CSP-814) and renders nothing. Every item below changes what the operator
sees or widens what Conspectus may read, so each has options and a
recommendation. The mockups are generated from the documented frames in
`docs/tui-columns.md` (same column grid and spacing); the glyphs are
candidates and must pass the vetting in ADR 0154 before use.

What the data layer provides to every option: for a session, an effective
state (`working`, `waiting`, `ready`, `compacting`, `error`, `starting`,
`exited`, `unknown`) with a detail, `since`, a quiet/stale flag, and a
subagent count; for a mux, counts over the sessions it hosts. Absence is "no
evidence", never "idle".

## Constraints that shape every option

- `docs/tui-columns.md` principle 1: single-kind views (Sessions, Mux, PRs)
  show state as a one-cell glyph; Pinned and History, which mix kinds, use
  words. Principle 2: every value has its own shape, no state hides behind
  the absence of a glyph, and nothing depends on color alone.
- The common `●`, `◐`, `○` set collides with the session kind, draft PR, and
  open PR glyphs. The braille spinner means "refresh pending" and cannot mean
  "working". `?` already means "can't be checked", `×` is the shared failure
  mark, `⊘` a stopped mux, `▲` an ambiguous link.
- The kind glyph `●` is the same on every session row in every view (it says
  what the row is). The state cell (`⊸`/`∘`) is the mux link.
- Colors are theme roles (ADR 0153): success, warning, error, info, muted.

## 1. Where the state mark goes

### Sessions today

```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ●   showcase   claude       1m  ∘  ambiguous mux candidate A  project          │
│    ●   showcase   claude       1m  ∘  ambiguous mux candidate B  project          │
│    ●   showcase   claude       1m  ⊸  agent-deck composite launcher  showcase-deck│
│    ●   showcase   claude       1m  ∘  hook-supersession current session  project  │
│    ● ◉ showcase   claude       1h  ∘  claude-code in project repo  project        │
│    ●   showcase   pi           2h  ∘  repo-a            Showcase: pi in atelier m…│
│    ●   showcase   opencode     3h  ∘  repo-b            Showcase: opencode in ate…│
│    ●   showcase   codex       12h  ⊸  bare-work  bare-project      Showcase: codex│
```

### Option A (recommended): a new one-cell column right after the age

`!` waiting on you, `»` working, `›` ready for a prompt, `×` error, `·` no
evidence. The age beside it then reads as "waiting 1m" or "ready 1h". The
cell is always filled, so no state hides behind a missing glyph.

```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ●   showcase   claude       1m  !  ∘  ambiguous mux candidate A  project       │
│    ●   showcase   claude       1m  »  ∘  ambiguous mux candidate B  project       │
│    ●   showcase   claude       1m  ›  ⊸  agent-deck composite launcher  showcase-…│
│    ●   showcase   claude       1m  »  ∘  hook-supersession current session  proje…│
│    ● ◉ showcase   claude       1h  ›  ∘  claude-code in project repo  project     │
│    ●   showcase   pi           2h  ·  ∘  repo-a            Showcase: pi in atelie…│
│    ●   showcase   opencode     3h  ×  ∘  repo-b            Showcase: opencode in …│
│    ●   showcase   codex       12h  ›  ⊸  bare-work  bare-project      Showcase: c…│
```

Pro: keeps `●` as the kind glyph and `⊸`/`∘` as the link; one cell; fits
`compact` density by dropping the cell last or first, as decided. Con: three
more cells (the label is cut earlier), and one more glyph to learn.

### Option B: the live glyph replaces the session kind glyph

```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   !   showcase   claude       1m  ∘  ambiguous mux candidate A  project          │
│    »   showcase   claude       1m  ∘  ambiguous mux candidate B  project          │
│    ›   showcase   claude       1m  ⊸  agent-deck composite launcher  showcase-deck│
│    »   showcase   claude       1m  ∘  hook-supersession current session  project  │
│    › ◉ showcase   claude       1h  ∘  claude-code in project repo  project        │
│    ·   showcase   pi           2h  ∘  repo-a            Showcase: pi in atelier m…│
│    ×   showcase   opencode     3h  ∘  repo-b            Showcase: opencode in ate…│
│    ›   showcase   codex       12h  ⊸  bare-work  bare-project      Showcase: codex│
```

Pro: no extra width; the mark sits where the eye starts. Con: breaks the
rule that a session row carries `●` in every view (a session's identity
glyph would change with its state), and nested and Pinned rows would need
the same treatment.

### Option C: a state word after the link cell

```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ●   showcase   claude       1m  ∘  needs you  ambiguous mux candidate A  proje…│
│    ●   showcase   claude       1m  ∘  working    ambiguous mux candidate B  proje…│
│    ●   showcase   claude       1m  ⊸  ready      agent-deck composite launcher  s…│
│    ●   showcase   claude       1m  ∘  working    hook-supersession current sessio…│
│    ● ◉ showcase   claude       1h  ∘  ready      claude-code in project repo  pro…│
│    ●   showcase   pi           2h  ∘             repo-a            Showcase: pi i…│
│    ●   showcase   opencode     3h  ∘  error      repo-b            Showcase: open…│
│    ●   showcase   codex       12h  ⊸  ready      bare-work  bare-project      Sho…│
```

Pro: self-explaining. Con: nine cells of width and it breaks the "single-kind
views use glyphs" principle; best kept for Pinned and History, which already
use words.

**Pinned and History** take words regardless: `waiting`, `working`, `ready`,
`error`. Open choice: replace `in mux` when live state exists (recommended;
`in mux` stays in the detail pane), or compose them (`in mux · waiting`).

**Animation.** Recommended: none. A static mark is cheaper to redraw, works
with reduced motion, and leaves the braille spinner meaning "refresh
pending". If you want motion for `working`, the refresh spinner needs a
different mark first.

## 2. A mux with several agents

A mux has no state of its own; the model gives counts. Option A for muxes
shows one rolled-up mark in the same column:

```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ▣   ambiguous                  claude      15m  !  ∘                           │
│    ▣   bare-work                  codex        2h  ›  ∘  Showcase: codex resume v…│
│    ▣ ◉ project                    claude       5m  »  ⊸                           │
│    ▣   showcase-deck              claude      30m  ·  ∘  Showcase: agent-deck com…│
│    ▣ ◎ nightly-refactor           claude       2d  ·  ⊘  ~/repos/project          │
```

Which state dominates when agents differ:

| Rule | Reads as | Notes |
|---|---|---|
| Recommended: waiting, then error, then ready, then working, then the rest | "does anything need me, then does anything wait for a prompt" | Matches the three buckets; a mux with one waiting and five working shows `!` |
| Waiting, then error, then working, then ready | "something is still busy" | Hides a finished agent behind a busy one |
| Counts only (`! 1 › 2 » 3`) | exact | Wide; fits a group header or the detail pane better than a row |

Nested agent rows (a mux with more than one agent) always carry their own
mark. Group headers could show a dim attention count next to `(N)`
(recommended: waiting count only, shown when above zero).

## 3. Header

Approximate current header: `updated 5s ago · 11/11 sessions · 5 mux · 1 stopped · ▲ 2`.

| Option | Header gains | Notes |
|---|---|---|
| Recommended | `! 2` (waiting count, whole graph, hidden at 0), ranked after freshness and counts, before `▲` | One glance answers "does anything need me?"; follows the same give-way rule as other chips |
| Fuller | `! 2 › 3 » 4` | Useful on wide terminals; costs width everywhere |
| None | nothing | Sorting alone carries it |

Open sub-choice: the chip counts the whole graph (recommended) or the
filtered view.

## 4. Ordering, grouping, and a way to jump

- **Jump to the next session that needs attention** (a key in the scoped
  keymap, works in every view, then the existing attach gesture). Recommended
  order: longest-waiting first, `waiting` only. The key is picked from the
  free keys when the story is built.
- **Float waiting sessions to the top**: a control, off by default, using the
  same mechanism as "float sessions with a mux". Rows never re-sort under the
  cursor between operator actions; the change applies on the next
  operator-initiated refresh or fold. (Claude's own agent view and Herdr
  re-sort live; Conspectus keeps rows still on fold, so this is the
  consistent choice.)
- **Group by state** as one more entry in the `Ctrl-G` cycle: optional, not
  recommended for v1 (group churn).
- **A dedicated Attention view** (needs-you and ready-unseen across sessions
  and muxes, plus ambiguous links and gone pins): a seventh tab. Recommended
  as a later step, after the cell and the header chip show whether it is
  needed.

## 5. Seen and unseen

A finished turn the operator has not looked at is the strongest "(b) ready
for a new prompt" signal, and six surveyed tools split it from "idle".
Where "seen" lives:

| Option | How | Cost |
|---|---|---|
| Recommended: derived read-only | seen when a tmux client was attached to the session's mux after the turn ended (`last_attached_epoch`), plus in-memory for sessions with no mux | no write; resets when the TUI restarts for sessions with no mux |
| In memory only | per TUI instance | forgets on restart |
| Persisted | a rebuildable observation under the state directory | a write the operator did not ask for (ADR 0143); flicker risk seen in agent-deck |

Shown as: `›` bright for unseen, dim for seen (shape the same).

## 6. Subagents and background work

Recommended: a dim `+N` chip after the mark's row label while any subagent is
active, and the detail pane lists them; no nested rows (a parallel burst
would multiply rows). A subagent that is waiting on you raises `!` on the
root session (already in the model).

## 7. Notifications

Recommended: none in v1. Every harness already notifies; doubling is noise,
and a desktop notification is a new subprocess and capability. Later: a
terminal bell while the TUI is open and unfocused, with per-state toggles.

## 8. Detail pane and Controls

Recommended: the explorer's Node gets `State`, `Since`, `Source`, and
`Evidence` (fresh, quiet, stale) fields; Controls gets a `State` filter row
beside the existing filters; `conspectus table --state waiting` follows.

## 9. Sources beyond hooks (detection policy)

Phase 1 reads no screen and no transcript (ADR 0206). Each source below needs
your go-ahead, then its own ADR.

| Source | Recommend | Why |
|---|---|---|
| Codex pane title (spinner, `[ ! ] Action Required`) | yes, first | cheapest; the only prompt exit from `waiting`; works even when the operator has not trusted Codex hooks |
| Another orchestrator's state (agent-deck's `state.db`) | yes | the reader already exists; no detection cost |
| `claude agents --json` | after the supervisor check | closes the Esc and approval gaps; unknown cost per poll |
| Transcript and rollout tails | no | formats change between versions; content-adjacent |
| Pane capture and regexes | no | the highest-maintenance path in every tool surveyed |

## Questions for the operator

1. Option A, B, or C for the session mark; glyph set as shown or different?
2. Mux rollup rule, and group-header attention count?
3. Header chip: waiting only, fuller, or none?
4. Which of jump key, float-to-top control, Attention view to build first?
5. Seen/unseen: derived, in memory, or persisted?
6. Subagents: `+N` chip, nested rows, or detail only?
7. Notifications: none, bell, or desktop?
8. Which hook-free sources may be built?
