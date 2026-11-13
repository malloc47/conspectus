# ADR 0144: Pinned And History Views

## Status

Accepted. Amends ADR 0073 (the kind glyph is shown for every kind in
views that mix kinds) and applies ADR 0131's alignment rule to rows of
mixed kinds. Removes the Pins groups, placeholder rows, and Pins modal
of ADR 0057's TUI surface. Part of the history, rebuild, and pin slots
plan with ADRs 0140, 0141, 0142, and 0143. Amended by ADR 0198: a pin's
menu offers to reveal its entity only where that entity's view lists it.

Amended (2026-10): a pin with no target yet wears a hollow pin glyph
(`◎`) in the ghost color, and `gone` describes only an entity that
existed; the agent or program is the same filled chip other views draw,
without a dot delimiter; the History view lists unpinned records by
default, with an **Unpinned only** control to list pinned ones too; the
interface calls a pin a pin and never by its storage word.

Amended by ADR 0145: status words are `stopped`, `removed`, `missing`,
and `planned` (never `gone`); the pin marker is filled (`◉`) for a pin on
a running entity and hollow (`◎`) otherwise, for live pins and History
records too; the age is the same axis as the native views; the chip
column is one width so the content aligns.

## Context

Pinned muxes and sessions sit in a Pins group at the top of the
Sessions and Mux views, with placeholder rows for pins that aren't
running and a `p` modal plus `N`, `B`, `b`, `A`, and `L` shortcuts to
manage them. With pin slots (ADR 0141), a pin can point at any
rebuildable kind and at things that are gone, and history (ADR 0140)
holds gone entities that were never pinned. The operator wants pinned
entities in their own view, a peer of Sessions, Mux, and PRs, and wants
history browsable without pinning anything.

Both new views list different kinds together. No current view does:
each builds rows of one kind under group headers. The row layout has to
be settled before the renderer is built, so columns line up across
kinds.

## Decision

### Two new views

**Pinned** (`4`) and **History** (`5`) join the view switcher after
Sessions (`1`), Mux (`2`), and PRs (`3`), in the tab strip, Controls,
and `[` / `]`. Each has a `[tui.views.pinned]` / `[tui.views.history]`
config table and a `--view pinned|history` flag, like the other views.

### One row layout for mixed kinds

```
<indent><disclosure?> <kind> <label>  <status>  <age>  <content>
```

| Column | Width | Source |
|---|---|---|
| kind | 1 cell | The ADR 0073 kind glyph, for **every** kind, sessions included |
| label | padded, at most 28 | The slot id in Pinned; the record's display label in History |
| status | padded, at most 9 | Per kind, below |
| age | right-aligned, padded | Live: last activity. Gone: last seen |
| content | the rest, cut with an ellipsis | Per kind, below |

Fixed columns pad to the widest value in the **whole tree**, not the
visible rows, so folding a group never shifts them, and the content is
laid out against the width the row really gets, after the scrollbar
gutter (ADR 0131).

ADR 0073 suppresses the session glyph where every row is a session,
because the glyph would only repeat what the view already says. In a
mixed view the glyph is the only kind signal, so it is always shown.

| Kind | Status | Content |
|---|---|---|
| Mux | `live`, `exited` (no harness running), `gone`, `unknown`, `planned` | `harness · ~/cwd`, or the command when there is no harness |
| Session | `in mux`, `resumable`, `missing` | `harness · title` |
| Checkout | `live`, `gone`, `unknown`, `planned` | `repo · branch` |
| Repo | `live`, `gone`, `unknown` | the first remote |
| Branch | `live`, `gone` | `repo · upstream` |
| PR | `open`, `draft`, `merged`, `closed` | `owner/repo · title` |
| Workspace, fork | `live`, `gone` | `provider · root` |

Gone, unknown, missing, and planned rows are dimmed. The status column
says what they are, so no ghost glyph is added.

### Pinned view

```
Pinned · by project                                            4/9 pinned live
▼ ~/src/conspectus
  ▶ ▣ conspectus-main    live       2m  claude · ~/src/conspectus
    ▣ conspectus-serve   live      40s  conspectus serve
    ● e44d01cb           resumable 17m  claude · Fix the ledger merge
    ◇ ledger-worktree    gone       2d  conspectus · feature/ledger
    ⇄ ledger-pr          merged     5h  malloc47/conspectus · Ledger store and merge
▼ ~/src/config
    ▣ config-agent3      gone       3d  claude · ~/src/config
    ▣ scratch            planned     —  codex · ~/src/config
▼ Other
    ◆ dotfiles           live       1d  origin git@github.com:me/dotfiles.git
```

- Every slot is a row. Selecting one selects its `PinNode`, and the
  detail pane shows the slot, its effective recipe with the source of
  each field (override, live, history, snapshot, default), and the
  target.
- **Grouping**: `project` (default; a slot's repo comes from its
  target or its cwd, and slots with none go in Other), `kind`, or
  `none`. **Sort**: slot order (default) or recency. Reordering from
  the TUI is backlog `CSP-687`; `pin move` reorders from the CLI.
- A mux slot expands to its sessions (the live mux's, or the ghost's
  hosted sessions). A checkout slot expands to the muxes and sessions
  rooted in it.
- `Enter` runs the kind's primary action: attach to a live mux,
  rebuild a gone mux or a planned slot (showing the plan first, ADR
  0142), resume a session, open a mux in a live worktree or rebuild a
  gone one, open a PR in the browser.

### History view

```
History · by project · 6 gone                                   Clean up: 2 past threshold
▼ ~/src/conspectus
  ▶ ▣ conspectus-feature gone       2d  claude · ~/wt/conspectus.feature-ledger
    ◇ feature-ledger     gone       2d  conspectus · feature/ledger
    ▣ spike              gone      16d  codex · ~/src/conspectus
▼ ~/src/config
    ▣ config-agent3      gone       3d  claude · ~/src/config
    ▣ scratch-shell      unknown    9d  zsh
▼ Other
    ◆ old-notes          gone      41d  origin git@github.com:me/notes.git
```

- Rows are history records that are gone or unknown, in the same
  layout, grouped by `project` (default), `kind`, or `none`, and sorted
  by last seen, newest first. Live entities are in their native views,
  so they aren't listed.
- A mux row expands to its hosted sessions, each offering "rebuild with
  this session".
- `Enter` rebuilds, showing the plan first.

### The row action menu

Today the only menus are the `w` worktree menu and the `m` mux menu.
A row action menu, `.`, opens on any row in any view and lists what
applies to it, menu-first as the TUI's other surfaces are:

- every pinnable row: **Pin** or **Unpin**;
- Pinned rows: Rebuild or Attach, Edit, Rename, Retarget, Refresh
  snapshot, Unpin, Reveal in its native view, Show rebuild plan;
- History rows: Rebuild, Rebuild with this session (on a hosted
  session), Pin, Forget, Show rebuild plan, Clean up…;
- gone rows in native views: Rebuild, Show rebuild plan.

`p` is the accelerator for Pin / Unpin on the selected row. `Delete`
unpins a pinned row (pressed twice, as removing a pin is today). `R`
renames a slot in the Pinned view.

**Edit** opens the slot edit form, the launch-spec form (ADR 0097)
filled from the effective recipe, with only the overridable fields for
the slot's kind (ADR 0141). **Retarget** opens a picker of same-kind
entities, live first, then from history.

### Clean up

"Clean up…" in the History view opens a modal listing the records
`history reap` would remove (past `[history.reap_after]`, not pinned,
not unknown), each with its age and selected. The operator deselects
any, then confirms. It reaps only what stays selected.

### Ghosts in the Mux view

```
Mux · by repo · 11 mux · 4 gone
▼ ~/src/conspectus
    ▣ conspectus-main         claude   2m
    ▣ conspectus-serve        serve   40s
    ▣ conspectus-feature      gone     2d
    ▣ spike                   gone    16d
```

Ghost muxes sort below the live muxes of their group, dimmed, with the
`gone` status and their last-seen age. A Controls toggle, "Show gone
muxes" (on by default), hides them. The header reads `11 mux · 4 gone`.
`Enter` on a ghost rebuilds it, plan first. A session whose last mux is
a ghost carries a `was in <mux>` chip in the Sessions view instead of a
placeholder row.

### Native views

- Native views stop floating pinned entities. A pinned row gets a
  one-cell pin marker, `◉` (the pin kind glyph), and stays where it
  sorts.
- Controls gains a `pinned only` filter for every native view.
- The Pins groups and the unbound-pin placeholder rows go away from
  the Sessions and Mux views.
- The Pins modal (`p`) and the `N`, `B`, `b`, `A`, and `L` pin keys go
  away. `N` opens the mux launch form with the worktree toggle on (ADR
  0096), and the mux launch form gains "pin" and "pin, don't launch".

## Consequences

- Pinning a row no longer moves it, and the native views show what
  exists, in their usual order.
- The pin-specific keys shrink to `p` and the row action menu, which
  also carries every other row action the operator can't remember a
  key for.
- The detail pane, search, and Reveal work on gone rows, because they
  are graph nodes (ADR 0140).
- Screenshots and the showcase scenario change to show the new views
  (ADR 0070, ADR 0113).
- Two more views to keep consistent with the column rules; the shared
  renderer keeps them from drifting apart.

## Alternatives Considered

- **Keep the Pins group at the top of each view.** The operator asked
  for a peer view instead; a group that floats above every view also
  pushes the view's real content down.
- **A ghost glyph for gone rows.** The first revision of the plan had
  one. The status column already says `gone`, and dimming carries the
  rest.
- **Show live records in History.** They are already in their native
  views; History is for what isn't there anymore.
- **One combined Pinned-and-History view.** Pinned is a short,
  ordered list the operator curates; History is long and grows by
  itself. Mixing them buries the slots.
- **A modal per action instead of a row menu.** That is the Pins
  modal's shape, and it took six keys. A menu built from the selected
  row lists only what applies.

## Open Questions Answered

- *Where did the pin keys go?* `p` pins and unpins, `.` opens the row
  menu, `Delete` unpins, and `Enter` rebuilds or attaches.
- *How does the operator see a gone mux they never pinned?* In the Mux
  view, dimmed below the live ones, and in History.
- *Do the views work in fixture and snapshot modes?* Yes. Fixtures
  carry history and gone nodes (ADR 0140).

## Related ADRs

- ADR 0031 — the Controls overlay.
- ADR 0057 — the TUI surface this replaces.
- ADR 0070, ADR 0113 — the showcase scenario and screenshots.
- ADR 0073 — node-kind visual identity; amended.
- ADR 0096, ADR 0097 — mux launch and the launch-spec form.
- ADR 0131 — the aligned-column rule.
- ADR 0132 — rows awaiting a refresh refuse actions.
- ADR 0140 — history and presence.
- ADR 0141 — pin slots.
- ADR 0142 — rebuild plans.
