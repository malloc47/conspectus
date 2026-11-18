# ADR 0248: The Showcase Demo Pins The Clock And Keeps Its Reads In Memory

## Status

Accepted. Amends
[ADR 0069](0069-interactive-fixture-mode-for-tui.md)
(fixture mode) and
[ADR 0113](0113-readme-screenshots.md)
(the pinned clock), and
[ADR 0245](0245-tui-an-agent-that-stops-reads-unread-until-you-read-it-and-reads-persist.md)
(which TUIs keep read marks in the state file). The demo's copy of the
fixture is a build artifact under `target/`; no write path outside
[ADR 0143](0143-mutation-envelope-with-history-pins-and-clone.md)'s
envelope is added.

## Context

The showcase's live states carry absolute times near its anchor
(2026-09-30). `tui --snapshot` pins the clock to that time
(`--snapshot-now`), so the screenshots show them. The interactive demos
(`just demo`, which is `tui --fixture`, and `dev scenario tui showcase`)
read them against today's clock, where every state is long stale and reads
`?`, and their ages read months. The operator, who asked how to see the
unread behavior (2026-11-17), could not: nothing in the demo was fresh, so
nothing could be unread, and there was no way to stage the transition from
working to waiting that the behavior is about.

Both demo TUIs also write the operator's real state file (`resume_view`),
read marks included. A mark made in one demo run would then hide the unread
stops of the next, and the fixture's session ids would join the 2,000
marks of the operator's own sessions.

## Decision

1. **`--fixture` takes the pinned clock.** `--snapshot-now EPOCH` applies
   to `tui --fixture` as it does to `--snapshot`: ages and live states are
   judged against that time for the whole run. `just demo` passes the
   showcase's anchor time (the justfile's `showcase_now`), so the demo
   shows what the screenshots show. The clock does not tick; an age in the
   demo stays what it was at the anchor.
2. **Fixture and scenario TUIs keep read marks in memory.** Their marks are
   not loaded from the state file and not offered to it; resting the cursor
   on a row, `c`, and `C` still read, for that run. The rest of the state
   file (sort, filters, expanded rows) behaves as before. Starting the demo
   again finds every stop unread again.
3. **`just demo` edits a copy; `just demo-stop` stages a stop.** The demo
   copies `tests/fixtures/showcase.json` to `target/demo/showcase.json` and
   runs on the copy, which `r` re-reads. `just demo-stop`, run from another
   terminal, makes the showcase's working hook session wait a few seconds
   before the anchor time; pressing `r` in the demo shows the stop as one
   more waiting agent, unread. The checked-in fixture is never edited.
4. **The scenario TUI keeps today's clock.** Its sessions' ages come from
   transcript files written when it starts, so pinning the clock to the
   anchor would date them in the future. Its comment says so; the fixture
   demo is the way to see the live states interactively.

## Consequences

- `just demo` is the interactive version of the screenshots: waiting,
  ready, and error marks, bold unread rows, `c`, `C`, `}`, `{`, and a staged
  stop. It needs `jq`, as `just regen-showcase-fixture` already does.
- The flag is named for snapshots and used by the demo; the help text says
  both. A fixture exported from a real world is not run with it.
- Operators who run `tui --fixture` or `dev scenario tui` no longer
  leave marks of the fixture's sessions in their state file.

## Alternatives Considered

- **A new flag (`--fixture-now`).** Two flags for one clock, with the same
  value in every use; the existing one gains a second place to apply.
- **Read the anchor from the fixture** (its newest `observed_ms`). Magic: a
  fixture exported from a real world would pin to a past moment.
- **Rewrite the fixture's times relative to today** at demo start. Moves
  every age and the `ambiguous`/attach evidence together, which only the
  anchor time keeps consistent, and would not match the screenshots.
- **Pin the scenario TUI too.** Its file-derived ages would be negative.
- **`--no-resume-view` in the demo recipe.** Stops the write but not the
  read of the operator's marks, and changes the demo's view state.
