---
id: m-23
title: "History, Rebuild, And Pin Slots"
---

## Description

Session pins (ADR 0057) do four jobs at once: they record a mux so it can be relaunched (with its argv and last session), stand in for work not yet started, float work to the top, and name sessions. Relaunching is only available through a pin, so in practice every mux gets pinned and "pinned" stops meaning anything. This milestone splits the jobs into three layers, per `docs/plans/history-rebuild-and-pin-slots.md` (operator review 2026-10-19 and 2026-10-21):

- **History**: everything rebuildable that Conspectus observes (muxes with their command and hosted sessions, worktrees, repos, PR snapshots), recorded automatically under `$XDG_STATE_HOME`, including entities that are gone. Gone needs a targeted check; absence from a scan never counts.
- **Pin slots**: editable, ordered slots in `$XDG_DATA_HOME/conspectus/pins.toml` that point at anything rebuildable (live, gone, or planned) and carry operator overrides.
- **Rebuild**: plans with an executor per kind (mux plus session resume, worktree through worktrunk, repo clone under a superseding envelope ADR), chained when one rebuild needs another.

The TUI gains Pinned and History views that list mixed kinds in aligned columns, and ghost muxes in the Mux view. The Pins groups and modal go away.

Order of work: the decision tickets first (history, pin slots, rebuild, the superseding envelope ADR, the views, then `docs/design.md`). Then the history spine (store, mux probe status, mux recording, checkout/repo and PR recording, graph projection with the live-assumption audit). Then the slot store and the rebuild engine, after which executors, the CLIs, and migration run in parallel. Then the shared row renderer and the views. The milestone ships as 0.2.0 after 0.1.0, with the legacy pin loader retired one release later. Follow-ups: bulk rebuild after reboot, TUI slot reordering, the worktree-location investigation, and pinned-PR refresh.

All tasks carry the `history-pins` label.
