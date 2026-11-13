# ADR 0207: Live-State Sources Beyond Hooks Need Their Own Decision

## Status

Accepted. Part of the live agent state plan with ADRs 0203 to 0206.
Applies the carve-out in ADR 0028 (no generic terminal scraping without a
separate decision) and the tiers of ADR 0086. Its first further source,
a Codex pane's title, is
[ADR 0217](0217-a-codex-panes-title-fills-in-its-live-state-where-hooks-cannot.md),
which follows decision 4 and makes one exception to it, for an approval
answered while the hook record is fresh. Its second, `claude agents
--json` after the check decision 5 asked for, is
[ADR 0218](0218-claude-codes-agent-list-repairs-a-live-state-its-hooks-left-stuck.md),
which runs only to repair a Claude Code hook record silent for ten
seconds in `working` or `waiting`, the meaning of "stale" in decision 4
for that source.

## Context

Hooks and plugins (ADR 0206) cover most states but have gaps: an
interrupted Claude turn fires no `Stop`, an approval granted fires no
event, a killed process fires no exit, and a session with no hook
installed has no evidence at all. Other orchestrators close those gaps
by reading the harness's own state surface, the screen, or another
orchestrator's database
(`docs/plans/live-state/research/05-orchestrators-and-ux.md` §4). Their
issue trackers show where the cost is: screen scraping regresses with
every harness UI change, and transcript formats change between
versions.

Each candidate widens what Conspectus reads or runs:

| Source | What it adds | What it costs |
|---|---|---|
| tmux pane title (`#{pane_title}`) | Codex's spinner and `[ ! ] Action Required` clear the instant an approval is answered; Claude prefixes a glyph | Reads text a harness chose to display; glyph vocabulary undocumented; ADR 0028 carve-out |
| `claude agents --json` | An official, supported read of session state with `waitingFor` | A new subprocess and capability entry (ADR 0117); starts a supervisor? unknown; cost per poll unknown |
| Transcript or rollout tail | `ready` versus `working` repair; interrupt detection | Reads payload-bearing files (ADR 0086 Tier 1: structural fields only); formats change between versions |
| Another orchestrator's state (agent-deck's `state.db`, `herdr`, `amux`) | Free, accurate state for sessions it manages | Third-party schemas; covers only its own sessions |
| Pane capture and regexes | Works with no hook | Highest maintenance in every tool surveyed; Tier 2 content |

## Decision

1. **Phase 1 reads no screen, no transcript, and runs no new program for
   live state.** The only inputs are hook and plugin events (ADR 0206) and
   process liveness the discovery code already has.
2. **A session with no events has no live state.** It is not shown as idle
   (ADR 0203).
3. **A missing signal is made visible, not guessed.** `hook status`
   reports a harness that is active without events, so a missing install
   or an untrusted Codex hook is found by the operator rather than hidden
   by a guess.
4. **Each further source needs its own ADR** that states its ADR 0086
   tier, adds its capability entry (ADR 0117), names the states it may
   assert, and says how it yields to hook evidence: a fresh hook record
   wins over a poll; a poll overrides only a stale or absent hook record.
5. **Ordering of candidates when the operator wants them**, by cost to
   benefit: the Codex pane title (cheapest, the only prompt exit from
   `waiting`); `claude agents --json` after a check that it does not
   start a supervisor; structural transcript tails for the interrupt and
   `end_turn` repair; ingesting another orchestrator's state; pane
   capture last, opt-in only.
6. **Terminal input is out of scope for every source** (ADR 0028, absolute).

## Consequences

- The first release answers the operator's three questions from events
  alone and is honest where it cannot: `unknown` and `quiet` exist so a
  gap shows as a gap.
- The known inaccuracies are listed in ADR 0206 and in the operator guide,
  so they are not rediscovered as bugs.
- Closing a gap is a small, separately reviewable change, and a source can
  be turned down without touching the model or the store.

## Alternatives Considered

- **Ship a pane-title reader now.** It is the best single hook-free signal
  and the cheapest, but it reads displayed text and carries the carve-out
  in ADR 0028, so it is the operator's call.
- **Scrape for everything.** Rejected by the evidence in the survey.
- **Run `claude agents --json` every refresh.** Unknown cost and a
  subprocess; needs the supervisor check first.

## Open Questions Answered

- *Is this ADR a refusal of the fallbacks?* No. It orders them and puts a
  gate in front of each, so adding one is a decision with a record.
